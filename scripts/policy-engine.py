#!/usr/bin/env python3
"""policy-engine.py — validate + decide per docs/design/03 & 07.

Reads JSON policy configs (YAML too if pyyaml importable), merges by scope
(sub < job < role < global), classifies watch-host events, resolves a decision
per events.<wire> > nodes.<id> > events."*" > default(ask), with 03's fail-safe
rule: auto/script without limits present degrades to ask.

Usage:
  python scripts/policy-engine.py --validate examples/policy/global.json
  python scripts/policy-engine.py --decide --event '<json>' --dir <configs-dir> --scope sub-XXX
  python scripts/policy-engine.py --selftest
"""
import argparse, json, os, re, sys

MODES = {"direct", "llm", "ask", "hybrid", "auto", "notify"}
KNOWN_SCOPE = {"global", "role-buyer", "role-asp", "role-evaluator"}
VENUES = {"dex", "defi", "polymarket", "trade_kit", "hyperliquid"}
WIRE_RE = re.compile(r"\b(job_[a-z_]+|sub_[a-z_]+|decision_request[a-z_]*)\b")

def parse_amount(s):
    m = re.match(r"^\s*([0-9]+(?:\.[0-9]+)?)\s*([A-Za-z0-9]+)\s*$", str(s))
    return (float(m.group(1)), m.group(2).upper()) if m else None

def _read(p):
    with open(p, encoding="utf-8") as f:
        txt = f.read()
    if p.endswith((".yaml", ".yml")):
        try:
            import yaml
            return yaml.safe_load(txt)
        except Exception:
            raise SystemExit(f"yaml unavailable or invalid: {p}")
    return json.loads(txt)

def load_cfg(path):
    cfg = _read(path)
    if not isinstance(cfg, dict):
        raise ValueError(f"config must be an object: {path}")
    cfg.setdefault("schemaVersion", 1)
    return cfg

def mode_of(entry):
    """entry: {'mode': X} or a plain string mode"""
    if isinstance(entry, str):
        return entry
    if isinstance(entry, dict):
        return entry.get("mode")
    return None

def validate(cfg, base_dir="."):
    errs, warns = [], []
    if cfg.get("schemaVersion") != 1:
        errs.append("schemaVersion must be 1")
    scope = cfg.get("scope", "global")
    if scope not in KNOWN_SCOPE and not re.match(r"^(sub|job)-", scope):
        errs.append(f"unknown scope '{scope}'")
    sig = cfg.get("signal", {})
    pm = sig.get("parseMode")
    if pm and pm not in {"strict", "loose", "notify"} and not str(pm).startswith("custom:"):
        errs.append(f"signal.parseMode invalid: {pm}")
    for ct in cfg.get("signal", {}).get("contentTags", []):
        if not isinstance(ct, dict) or not ct.get("kind") or not ct.get("match"):
            errs.append(f"signal.contentTags entry needs 'kind' + 'match': {ct}")
    lim = cfg.get("limits", {})
    for venue, caps in lim.get("venueGrants", {}).items():
        if venue not in VENUES:
            errs.append(f"limits.venueGrants unknown venue '{venue}'")
        for side in ("maxBuy", "maxSell"):
            if side in caps and parse_amount(caps[side]) is None:
                errs.append(f"limits.venueGrants.{venue}.{side} unparsable: {caps[side]}")
    if "dailyCap" in lim and parse_amount(lim["dailyCap"]) is None:
        errs.append(f"limits.dailyCap unparsable: {lim['dailyCap']}")
    for table, kind in (("events", "events.<wire>"), ("nodes", "nodes.<id>")):
        for key, entry in cfg.get(table, {}).items():
            m = mode_of(entry)
            if m is None:
                errs.append(f"{kind} '{key}': missing mode")
                continue
            if m not in MODES and not str(m).startswith("script:"):
                errs.append(f"{kind} '{key}': invalid mode '{m}'")
            if isinstance(entry, dict):
                if entry.get("confirm") not in (None, "always", "once", "never"):
                    errs.append(f"{kind} '{key}': confirm invalid")
                if entry.get("vetoFallback") not in (None, "ask", "abort", "notify"):
                    errs.append(f"{kind} '{key}': vetoFallback invalid")
    for node, hk in cfg.get("hooks", {}).items():
        for pos in ("pre", "post"):
            for cmd in hk.get(pos, []):
                if str(cmd).startswith("scripts/"):
                    p = os.path.join(base_dir, str(cmd))
                    if not os.path.exists(p):
                        warns.append(f"hooks.{node}.{pos}: whitelisted script missing: {cmd}")
                else:
                    warns.append(f"hooks.{node}.{pos}: '{cmd}' outside scripts/ whitelist (ignored)")
    if lim or cfg.get("signal", {}).get("rules"):
        pass
    return errs, warns

def load_chain(cfg_dir, scope):
    """sub-<id> > job-<id> > role-* > global (shallow merge, later wins)."""
    order = []
    if scope.startswith("job-"):
        order = ["global", "role-buyer", scope]  # role by scope param
    elif scope.startswith("sub-"):
        order = ["global", "role-buyer", scope]
    elif scope in KNOWN_SCOPE:
        order = ["global"] if scope == "global" else ["global", scope]
    merged, sources = {}, []
    for name in order:
        for ext in (".json", ".yaml", ".yml"):
            p = os.path.join(cfg_dir, name + ext)
            if os.path.exists(p):
                cfg = load_cfg(p)
                for k, v in cfg.items():
                    merged[k] = v if k != "events" else {**merged.get("events", {}), **v}
                sources.append(p)
                break
    return merged, sources

def classify(ev):
    raw = ev.get("raw", "")
    kind = ev.get("kind", "raw")
    node, wire = None, None
    m = WIRE_RE.search(raw)
    if m:
        wire = m.group(1)
    if kind == "decision_request":
        node = "buyer.sub.decide"
    elif kind == "signal":
        node = "buyer.sub.signal_received"
    elif kind == "signal_order":
        node = "buyer.sub.signal_order"
    elif kind == "signal_analysis":
        node = "buyer.sub.signal_analysis"
    elif kind == "task_event":
        node = "buyer.task.terminal" if ev.get("terminal") else "buyer.task.event"
    elif kind == "notification":
        node = "notify"
    return {"node": node, "wire": wire, "kind": kind}

def decide(ev, cfg, base_dir="."):
    cl = classify(ev)
    events = cfg.get("events", {})
    nodes = cfg.get("nodes", {})
    hooks = cfg.get("hooks", {})
    entry = None
    if cl["wire"] and cl["wire"] in events:
        entry, src = events[cl["wire"]], f"events.{cl['wire']}"
    elif cl["node"] and cl["node"] in nodes:
        entry, src = nodes[cl["node"]], f"nodes.{cl['node']}"
    elif "*" in events:
        entry, src = events["*"], "events.*"
    else:
        entry, src = {"mode": "ask"}, "default(ask)"
    mode = mode_of(entry) or "ask"
    limits = cfg.get("limits", {})
    has_limits = bool(limits.get("venueGrants")) and bool(limits.get("requireConsentSnapshot", True)) is not False or bool(limits)
    fail_safe = mode in ("auto",) or str(mode).startswith("script:")
    if fail_safe and not limits:
        mode, src = "ask", f"{src}->ask(fail-safe: no limits for {mode})"
    conf = entry if isinstance(entry, dict) else {}
    node_hooks = hooks.get(cl["node"], {}) if cl["node"] else {}
    return {
        "scope_cfg": cfg.get("scope", "?"),
        "kind": cl["kind"], "node": cl["node"], "wire": cl["wire"],
        "decision_src": src, "mode": mode,
        "confirm": conf.get("confirm"), "vetoFallback": conf.get("vetoFallback", "ask"),
        "hooks": {"pre": node_hooks.get("pre", []), "post": node_hooks.get("post", [])},
        "limits_present": bool(limits), "reason": conf.get("reason"),
    }

def selftest():
    import tempfile
    fails = []
    def check(cond, msg):
        print(("PASS" if cond else "FAIL"), msg)
        if not cond:
            fails.append(msg)
    # 1. validation catches bad mode + bad amount + missing whitelist script
    with tempfile.TemporaryDirectory() as td:
        bad = {"schemaVersion": 1, "scope": "global",
               "events": {"*": {"mode": "teleport"}},
               "limits": {"venueGrants": {"dex": {"maxBuy": "abc USDC"}}},
               "hooks": {"notify": {"post": ["scripts/nope.py"]}}}
        e, w = validate(bad, td)
        check(any("teleport" in x for x in e), "validate rejects bad mode")
        check(any("abc USDC" in x for x in e), "validate rejects unparsable amount")
        check(any("missing" in x for x in w), "validate warns on missing whitelist script")
    # 2. fail-safe: auto w/o limits -> ask
    d = decide({"kind": "signal", "raw": "buy XXX 5%"}, {"schemaVersion": 1, "scope": "role-buyer", "events": {"*": {"mode": "auto"}}})
    check(d["mode"] == "ask" and "fail-safe" in d["decision_src"], "auto without limits degrades to ask")
    # 3. wire beats node beats wildcard
    cfg = {"schemaVersion": 1, "scope": "role-buyer",
           "events": {"sub_user_reject": {"mode": "llm"}, "*": {"mode": "notify"}},
           "nodes": {"buyer.sub.signal_received": {"mode": "direct"}}}
    d = decide({"kind": "task_event", "raw": "sub_user_reject event text"}, cfg)
    check(d["wire"] == "sub_user_reject" and d["mode"] == "llm", "events.<wire> wins over wildcard")
    d = decide({"kind": "signal", "raw": "signal arrived"}, cfg)
    check(d["mode"] == "direct" and d["node"] == "buyer.sub.signal_received", "nodes.<id> wins over wildcard")
    # 4. scope merge sub over role over global
    with tempfile.TemporaryDirectory() as td:
        json.dump({"scope": "global", "events": {"*": {"mode": "ask"}}}, open(f"{td}/global.json", "w"))
        json.dump({"scope": "role-buyer", "events": {"*": {"mode": "notify"}}}, open(f"{td}/role-buyer.json", "w"))
        json.dump({"scope": "sub-1", "nodes": {"buyer.sub.decide": {"mode": "auto"}}}, open(f"{td}/sub-1.json", "w"))
        merged, _ = load_chain(td, "sub-1")
        check(merged["events"]["*"]["mode"] == "notify", "role overrides global on merge")
        check(merged["nodes"]["buyer.sub.decide"]["mode"] == "auto", "sub adds node entry")
    print("selftest:", "OK" if not fails else f"{len(fails)} FAILURES")
    return 0 if not fails else 1

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--validate", metavar="CFG")
    ap.add_argument("--decide", action="store_true")
    ap.add_argument("--event", default='{"kind":"notification","raw":""}')
    ap.add_argument("--dir", default=".")
    ap.add_argument("--scope", default="global")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(selftest())
    if a.validate:
        errs, warns = validate(load_cfg(a.validate), os.path.dirname(a.validate) or ".")
        print(json.dumps({"valid": not errs, "errors": errs, "warnings": warns}, ensure_ascii=False))
        sys.exit(1 if errs else 0)
    if a.decide:
        merged, srcs = load_chain(a.dir, a.scope)
        out = decide(json.loads(a.event), merged, a.dir)
        out["config_sources"] = srcs
        print(json.dumps(out, ensure_ascii=False))
        sys.exit(0)

if __name__ == "__main__":
    main()
