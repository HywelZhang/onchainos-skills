#!/usr/bin/env python3
"""sub-collect.py — per-subscription scoped collector (real-data acquisition loop).

Polls okx-a2a watch scoped to ONE subscription job, normalizes each payload with
watch-host's normalize(), routes it through the decision loop under the sub's
policy scope, and appends a routed record to collected.jsonl (for calibration
stats / OQ-10). Use one process per tracked subscription, or run periodically.

Usage:
  python scripts/sub-collect.py --job-id 0x4467... --scope sub-40209 --policy-dir examples/policy --out collected.jsonl [--once] [--timeout 300]
"""
import argparse, importlib.util, json, os, subprocess, sys, time

_SCRIPTS = os.path.dirname(os.path.abspath(__file__))
wh = importlib.util.spec_from_file_location("watch_host", os.path.join(_SCRIPTS, "watch-host.py"))
_wm = importlib.util.module_from_spec(wh)
wh.loader.exec_module(_wm)  # noqa
dl_spec = importlib.util.spec_from_file_location("decision_loop", os.path.join(_SCRIPTS, "decision-loop.py"))
_dl = importlib.util.module_from_spec(dl_spec)
dl_spec.loader.exec_module(_dl)  # noqa

def poll_once(job_id, timeout_s=45):
    cmd = _wm.resolve_a2a("okx-a2a")
    r = subprocess.run([cmd, "user", "watch", "--json", "--job-id", job_id,
                        "--timeout", str(timeout_s), "--once"],
                       capture_output=True, text=True, timeout=timeout_s + 20,
                       encoding="utf-8", errors="replace")
    return r.returncode, (r.stdout or "")

def collect(job_id, scope, policy_dir, out_path, once, deadline_s, report_every=10):
    out = open(out_path, "a", encoding="utf-8")
    n_events = n_routerr = 0
    t0 = time.monotonic()
    while time.monotonic() - t0 < deadline_s:
        rc, text = poll_once(job_id, 45)
        if rc != 0 or not text.strip():
            continue
        payloads = _wm.parse_batch(text)
        for p in payloads:
            try:
                if isinstance(p, dict) and p.get("userContent"):
                    raw_text = p["userContent"]
                    ev_job = p.get("jobId") or job_id
                else:
                    raw_text = p if isinstance(p, str) else json.dumps(p, ensure_ascii=False)
                    ev_job = job_id
                ev = _wm.normalize(raw_text, ev_job or "", sticky=False)
            except Exception:
                continue
            if ev.get("kind") in (None, ""):
                continue
            try:
                d, notes, _ = _dl.process_event(ev, policy_dir, scope, policy_dir)
                rec = {"ts": ev.get("ts"), "scope": scope, "kind": ev.get("kind"),
                       "mode": d["mode"], "node": d["node"],
                       "src": d["decision_src"], "hash": ev.get("_k"),
                       "raw": (ev.get("raw") or "")[:400]}
                out.write(json.dumps(rec, ensure_ascii=False) + "\n")
                out.flush()
                n_events += 1
                if n_events % report_every == 1 or n_events <= 3:
                    print(f"  [sub-collect] +{n_events} {scope} kind={ev.get('kind')} -> {d['mode']} @ {d['node']}", flush=True)
            except Exception as e:
                n_routerr += 1
                print("  [sub-collect] route err:", e, flush=True)
        if once:
            break
    out.close()
    print(f"[sub-collect] done: {n_events} events routed, {n_routerr} route errors, scope={scope}")
    return n_events

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--job-id", required=True)
    ap.add_argument("--scope", required=True)
    ap.add_argument("--policy-dir", default=os.path.join(_SCRIPTS, "..", "examples", "policy"))
    ap.add_argument("--out", default="collected.jsonl")
    ap.add_argument("--once", action="store_true")
    ap.add_argument("--timeout", type=int, default=280)
    a = ap.parse_args()
    sys.exit(0 if collect(a.job_id, a.scope, a.policy_dir, a.out, a.once, a.timeout) >= 0 else 1)

if __name__ == "__main__":
    main()
