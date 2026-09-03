#!/usr/bin/env python3
"""decision-loop.py — wire watch-host events -> policy-engine decide -> hooks/notify.

Closes the loop of docs/design/06+07: consumes normalized events (watch-host
JSONL or --event stdin), resolves a decision via policy-engine (events.<wire> >
nodes.<id> > * > ask), then executes the node's pre/post hooks (whitelisted
scripts/ dir, env-var context, pre non-zero exit = veto -> vetoFallback) and
prints a console notification (OQ-4 console).

Hooks runtime (03): pre hooks may veto; post hooks observer-only. Commands must
start with scripts/ (whitelist) and live under the repo's scripts dir or the
policy config dir.

Usage:
  python scripts/decision-loop.py --event-dir <watch events dir> --policy-dir examples/policy --scope sub-EXAMPLE [--loop]
  echo '{"kind":"signal","raw":"..."}' | python scripts/decision-loop.py --stdin ...
  python scripts/decision-loop.py --selftest
"""
import argparse, importlib.util, json, os, subprocess, sys, time

_SCRIPTS = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("policy_engine", os.path.join(_SCRIPTS, "policy-engine.py"))
pe = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pe)  # noqa
_se2 = importlib.util.spec_from_file_location("signal_envelope", os.path.join(_SCRIPTS, "signal-envelope.py"))
se = importlib.util.module_from_spec(_se2)
_se2.loader.exec_module(se)  # noqa

def run_hook(cmd, env, base_dir):
    """Execute one hook command. Returns (ok, vetoed, output)."""
    parts = cmd.split()
    if not parts or not parts[0].startswith("scripts/"):
        return True, False, f"skip non-whitelisted hook: {cmd}"
    path = os.path.join(base_dir, *parts[0].split("/"))
    if not os.path.exists(path):
        return True, False, f"missing hook script: {cmd}"
    argv, rest = [path], parts[1:]
    if path.endswith(".py"):
        argv = [sys.executable, path]
    elif path.endswith(".sh"):
        argv = ["bash", path]
    argv += rest
    try:
        r = subprocess.run(argv, capture_output=True, text=True, timeout=30,
                           env={**os.environ, **env}, encoding="utf-8", errors="replace")
        if r.returncode != 0:
            return False, True, (r.stderr or r.stdout or "").strip()[:300]
        return True, False, (r.stdout or "").strip()[:200]
    except subprocess.TimeoutExpired:
        return False, True, "hook timeout(30s)"

def signal_check(ev, merged):
    """Parse+validate envelope for signal events (02). Returns dict or None."""
    if ev.get("kind") != "signal":
        return None
    env, raw = se.parse_envelope(ev.get("raw", "") or "")
    if not env:
        return {"envelope": False, "valid": False,
                "issues": ["no envelope (loose raw)"],
                "parseMode": merged.get("signal", {}).get("parseMode", "loose")}
    mode = merged.get("signal", {}).get("parseMode", "loose")
    trust = merged.get("signal", {}).get("trustAsps", [])
    issues = se.validate(env, mode=mode, trust_asps=tuple(trust))
    return {"envelope": True, "valid": not issues, "issues": issues,
            "parseMode": mode, "cls": env["header"].get("signalClass")}

def content_tag(ev, merged):
    """Signal content tagging (per-sub policy signal.contentTags): raw signals are
    classified deterministically by substring match (e.g. signal_type=order/analysis)
    into sub-kinds so a 3-min analysis stream never spams a buyer's ask queue."""
    tags = (merged.get("signal") or {}).get("contentTags") or []
    if not tags or ev.get("kind") != "signal":
        return ev
    raw = ev.get("raw", "") or ""
    for ct in tags:
        if ct.get("match") and ct["match"] in raw:
            return dict(ev, kind=ct["kind"])
    return ev

def process_event(ev, policy_dir, scope, base_dir):
    merged, srcs = pe.load_chain(policy_dir, scope)
    # live subscription deliveries arrive as task_event w/ [Received] payload;
    # under a sub-* scope they ARE signals (02) -> normalize so envelope/contentTags apply
    if scope.startswith("sub-") and ev.get("kind") == "task_event":
        raw = ev.get("raw", "") or ""
        if "[Received]" in raw and "Job:" in raw:
            ev = dict(ev, kind="signal")
    sig = signal_check(ev, merged)
    ev2 = content_tag(ev, merged)
    d = pe.decide(ev2, merged, base_dir)
    if sig and not sig["valid"] and d["mode"] in ("auto", "direct"):
        d = dict(d, mode="ask",
                 decision_src=d["decision_src"] + "->ask(signal invalid: %s)" % "; ".join(sig["issues"][:2]))
    d["signal"] = sig
    env = {"EVENT_KIND": ev.get("kind", ""), "JOB_ID": ev.get("jobId", ""),
           "EVENT_RAW": (ev.get("raw", "") or "")[:2000],
           "EVENT_SIGNAL_VALID": str(bool(sig and sig["valid"]))}
    notes = []
    # pre hooks (control)
    for h in d["hooks"].get("pre", []):
        ok, vetoed, out = run_hook(h, env, base_dir)
        notes.append(f"pre {h}: {'OK' if ok else 'VETO'} {out[:120]}")
        if vetoed:
            fb = d.get("vetoFallback", "ask")
            notes.append(f"veto -> fallback {fb}")
            return d, notes, env
    # post hooks (observer)
    for h in d["hooks"].get("post", []):
        ok, _, out = run_hook(h, env, base_dir)
        notes.append(f"post {h}: {'OK' if ok else 'ERR'} {out[:120]}")
    return d, notes, env

def consume(events, policy_dir, scope, base_dir, out):
    for ev in events:
        d, notes, env = process_event(ev, policy_dir, scope, base_dir)
        line = {"ts": int(time.time()), "kind": ev.get("kind"), "node": d["node"],
                "decision_src": d["decision_src"], "mode": d["mode"],
                "hooks": notes, "raw": (ev.get("raw") or "")[:120]}
        out.append(line)
        tag = {"direct": "⚙️ 直触发", "ask": "❓ 需人工", "llm": "🤖 交LLM",
               "auto": "⚙️ auto", "notify": "🔔 通知"}.get(d["mode"], d["mode"])
        print(f"[decision-loop] {tag} | {d['node']} | src={d['decision_src']}", flush=True)
        for n in notes:
            if n: print(f"    {n}", flush=True)
    return out

def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            pass
    ap = argparse.ArgumentParser()
    ap.add_argument("--event-dir", help="watch-host events dir (JSONL, tail new)")
    ap.add_argument("--stdin", action="store_true", help="read one event JSON from stdin")
    ap.add_argument("--event", default=None, help="inline event JSON")
    ap.add_argument("--policy-dir", default="examples/policy")
    ap.add_argument("--scope", default="global")
    ap.add_argument("--base-dir", default=".")
    ap.add_argument("--loop", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(selftest())
    seen = set()
    path = None
    if a.event_dir:
        path = os.path.join(a.event_dir, "events.jsonl")
    while True:
        batch = []
        if a.stdin or a.event:
            raw = a.event or sys.stdin.read()
            try:
                batch = [json.loads(raw)]
            except Exception as e:
                print("bad event:", e); sys.exit(2)
        elif path and os.path.exists(path):
            for line in open(path, encoding="utf-8"):
                line = line.strip()
                if not line: continue
                try:
                    obj = json.loads(line)
                except Exception:
                    continue
                k = obj.get("_k") or obj.get("ts")
                if k in seen: continue
                seen.add(k)
                batch.append(obj)
        if batch:
            consume(batch, a.policy_dir, a.scope, a.base_dir, [])
        if not a.loop:
            break
        time.sleep(10)

def selftest():
    import tempfile
    fails = []
    def check(c, m):
        print(("PASS" if c else "FAIL"), m)
        if not c: fails.append(m)
    # decision on signal with example policy dir (sub-EXAMPLE over global)
    ev = {"kind": "signal", "jobId": "", "raw": "active_subscription_signal buy XXX 5%"}
    with tempfile.TemporaryDirectory() as td:
        # hook scripts: audit-log (ok), veto (nonzero)
        os.makedirs(f"{td}/scripts")
        open(f"{td}/scripts/audit-log.py", "w").write("print('audited')\n")
        open(f"{td}/scripts/veto.py", "w").write("import sys; sys.exit(3)\n")
        d, notes, _ = process_event(ev, "examples/policy", "sub-EXAMPLE", td)
        check(d["node"] == "buyer.sub.signal_received"
              and "nodes.buyer.sub.signal_received" in d["decision_src"]
              and d["mode"] == "ask" and "signal invalid" in d["decision_src"],
              "signal resolves via example policy (nodes.<id>) but raw-only under strict -> ask")
        check(any("audit-log" in n for n in notes), "post hook executed from sub cfg")
        ev2 = {"kind": "task_event", "jobId": "", "raw": "job_created new task"}
        # veto scenario: temp policy with pre-hook on the resolved node
        veto_dir = os.path.join(td, "veto-policy")
        os.makedirs(veto_dir)
        json.dump({"schemaVersion": 1, "scope": "global",
                   "events": {"*": {"mode": "ask"}},
                   "hooks": {"buyer.task.event": {"pre": ["scripts/veto.py"]}}},
                  open(f"{veto_dir}/global.json", "w"))
        d2, notes2, _ = process_event(ev2, veto_dir, "global", td)
        check(any("VETO" in n for n in notes2) and any("veto -> fallback ask" in n for n in notes2),
              "pre hook veto -> vetoFallback ask")
        # signal envelope gating: strict policy, wildcard auto with limits
        sig_dir = os.path.join(td, "sig-policy")
        os.makedirs(sig_dir)
        json.dump({"schemaVersion": 1, "scope": "global",
                   "signal": {"parseMode": "strict", "trustAsps": ["Agent#12"]},
                   "limits": {"venueGrants": {"dex": {"maxBuy": "200 USDC"}}},
                   "events": {"*": {"mode": "auto"}}},
                  open(f"{sig_dir}/global.json", "w"))
        good_env = se.build_envelope("trade", "buy XXX 5%", "Agent#12", "svc_9",
                                     "tpl", signal_id="s1",
                                     body={"side": "buy",
                                           "asset": {"chain": "solana", "address": "0xabc"},
                                           "venue": {"kind": "dex"},
                                           "amount": {"mode": "percent", "value": 5}})
        ev_ok = {"kind": "signal", "jobId": "", "raw": se.render_deliverable(good_env)}
        d3, _, _ = process_event(ev_ok, sig_dir, "global", td)
        check(d3["mode"] == "auto" and d3["signal"]["valid"], "valid strict envelope keeps auto")
        bad_env = se.build_envelope("trade", "buy XXX", "Agent#12", "svc_9", "tpl",
                                    signal_id="s2", body={"side": "buy"})
        ev_bad = {"kind": "signal", "jobId": "", "raw": se.render_deliverable(bad_env)}
        d4, _, _ = process_event(ev_bad, sig_dir, "global", td)
        check(d4["mode"] == "ask" and "signal invalid" in d4["decision_src"],
              "strict invalid envelope downgraded to ask")
        raw_ev = {"kind": "signal", "jobId": "", "raw": "no envelope plain text"}
        d5, _, _ = process_event(raw_ev, sig_dir, "global", td)
        check(d5["mode"] == "ask" and "no envelope" in d5["signal"]["issues"][0],
              "raw-only signal in strict policy -> ask")
    print("selftest:", "OK" if not fails else f"{len(fails)} FAIL")
    return 0 if not fails else 1

if __name__ == "__main__":
    main()
