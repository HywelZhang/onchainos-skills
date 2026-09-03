#!/usr/bin/env python3
"""signal-envelope.py — two-layer signal payload (docs/design/02).

ASP side:  build_envelope(...) -> deliverable text = JSON fence (header+body) + raw
Buyer side: parse_envelope(text) -> (envelope|None, raw_text)
            validate(envelope, mode) -> issues (strict requires side/asset/venue/amount;
              caps: slippageBps<=500, ttl<=86400, expired rejected, idempotency key)
            loose classify seam stays deterministic by default (llm_fn injectable later)

Usage:
  python scripts/signal-envelope.py --build --class trade --raw "...long text..." \
      --side buy --asset-address 0x... --chain solana --venue dex \
      --amount 5 --amount-mode percent --sender-agent Agent#12 --service-id svc_9
  python scripts/signal-envelope.py --parse <file-or-text> [--mode strict] [--trust-asp Agent#12]
  python scripts/signal-envelope.py --selftest
"""
import argparse, json, re, sys, time

FENCE_START = "```json"
FENCE_END = "```"
MAX_SLIPPAGE_BPS = 500
MAX_TTL_SEC = 86400
TRADE_REQUIRED_STRICT = ("side", "asset", "venue", "amount")

def now_ms():
    return int(time.time() * 1000)

def build_envelope(signal_class, raw, sender_agent, service_id, template,
                   lang="zh-CN", signal_id=None, expires_at=None, ttl_sec=None,
                   body=None, extra_header=None):
    """Compose envelope dict; caller supplies class-specific body fields."""
    header = {
        "schemaVersion": 1,
        "signalClass": signal_class,
        "signalId": signal_id or ("sig_%x" % now_ms()),
        "issuedAt": now_ms(),
        "sender": {"agentId": sender_agent, "serviceId": service_id},
        "template": template,
        "lang": lang,
    }
    if expires_at:
        header["expiresAt"] = expires_at
    if ttl_sec:
        header["ttlSec"] = ttl_sec
    if extra_header:
        header.update(extra_header)
    env = {"header": header, "body": body or {}, "raw": raw}
    return env

def render_deliverable(env):
    """deliverable .txt = JSON fence (envelope) + blank line + raw text (02 §6.1)."""
    return FENCE_START + "\n" + json.dumps(env, ensure_ascii=False, indent=1) + "\n" + FENCE_END + "\n\n" + (env.get("raw") or "")

def parse_envelope(text):
    """Detect fenced envelope in deliverable text. Returns (envelope|None, raw)."""
    if not text:
        return None, ""
    m = re.search(FENCE_START + r"\s*(\{.*?\})\s*" + FENCE_END, text, re.S)
    if not m:
        return None, text
    try:
        env = json.loads(m.group(1))
    except json.JSONDecodeError:
        return None, text
    if not isinstance(env, dict) or not isinstance(env.get("header"), dict):
        return None, text
    return env, text[m.end():].strip()

def check_expiry(env):
    h = env["header"]
    if h.get("expiresAt") and now_ms() > h["expiresAt"]:
        return "signal expired"
    if h.get("ttlSec") and h["ttlSec"] > MAX_TTL_SEC:
        return "ttl exceeds 86400s"
    if h.get("ttlSec") and now_ms() > h["issuedAt"] + h["ttlSec"] * 1000:
        return "signal ttl elapsed"
    return None

def validate(env, mode="strict", trust_asps=(), max_slippage_bps=MAX_SLIPPAGE_BPS):
    """Return list of issues. strict: missing trade-critical fields = reject."""
    issues = []
    h, b = env.get("header", {}), env.get("body") or {}
    if h.get("schemaVersion", 1) > 1:
        issues.append("schemaVersion too new; treat as raw")
    sender = (h.get("sender") or {}).get("agentId")
    cls = h.get("signalClass")
    if not h.get("signalId"):
        issues.append("missing signalId (idempotency key)")
    exp = check_expiry(env)
    if exp:
        issues.append(exp)
    if cls == "trade":
        if mode == "strict":
            for f in TRADE_REQUIRED_STRICT:
                if f not in b or b.get(f) in (None, ""):
                    issues.append(f"strict: missing trade field '{f}'")
        if isinstance(b.get("slippageBps"), int) and b["slippageBps"] > max_slippage_bps:
            issues.append("slippageBps over cap")
        amt = b.get("amount")
        if amt and not isinstance(amt, dict):
            issues.append("amount must be {mode, value}")
        elif amt:
            if amt.get("mode") not in ("percent", "fixed"):
                issues.append("amount.mode invalid")
            if not isinstance(amt.get("value"), (int, float)):
                issues.append("amount.value missing")
        venue = (b.get("venue") or {}).get("kind") if isinstance(b.get("venue"), dict) else None
        if venue not in ("dex", "trade_kit", "dapp", None):
            issues.append(f"unknown venue kind: {venue}")
    elif cls == "security_alert":
        if not b.get("alertType"):
            issues.append("security_alert missing alertType")
        if b.get("urgency") not in (None, 0, 1, 2, 3, 4):
            issues.append("urgency must be 0-4")
    elif cls is None:
        issues.append("missing signalClass")
    if sender and trust_asps and sender not in trust_asps and mode == "strict":
        issues.append(f"sender {sender} not in trustAsps (strict auto refused)")
    return issues

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--build", action="store_true")
    ap.add_argument("--parse", metavar="TEXT_OR_FILE")
    ap.add_argument("--mode", default="strict")
    ap.add_argument("--trust-asp", action="append", default=[])
    ap.add_argument("--class", dest="signal_class", default="trade")
    ap.add_argument("--raw", default="")
    ap.add_argument("--side", default=None)
    ap.add_argument("--asset-address", default=None)
    ap.add_argument("--chain", default=None)
    ap.add_argument("--venue", default=None)
    ap.add_argument("--amount", type=float, default=None)
    ap.add_argument("--amount-mode", default="percent")
    ap.add_argument("--sender-agent", default="Agent#TEST")
    ap.add_argument("--service-id", default="svc_test")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(selftest())
    if a.build:
        body = {}
        if a.signal_class == "trade":
            asset = {"chain": a.chain or "solana"}
            if a.asset_address:
                asset["address"] = a.asset_address
            body = {"side": a.side or "buy", "asset": asset,
                    "venue": {"kind": a.venue or "dex"}}
            if a.amount is not None:
                body["amount"] = {"mode": a.amount_mode, "value": a.amount}
        env = build_envelope(a.signal_class, a.raw, a.sender_agent, a.service_id,
                             template=f"okx-signal-{a.signal_class}-v1", body=body)
        print(render_deliverable(env))
    if a.parse:
        text = open(a.parse, encoding="utf-8").read() if a.parse != "-" and ("\n" not in a.parse and (a.parse.endswith(".txt") or a.parse.endswith(".md") or a.parse.endswith(".json"))) else a.parse
        env, raw = parse_envelope(text)
        if not env:
            print(json.dumps({"envelope": None, "rawLen": len(raw),
                              "issues": ["no envelope; treat as loose raw"]}, ensure_ascii=False))
            sys.exit(0)
        issues = validate(env, mode=a.mode, trust_asps=tuple(a.trust_asp))
        print(json.dumps({"envelope": env, "rawLen": len(raw), "issues": issues,
                          "valid": not issues}, ensure_ascii=False))
        sys.exit(1 if issues else 0)

def selftest():
    fails = []
    def check(c, m):
        print(("PASS" if c else "FAIL"), m)
        if not c: fails.append(m)
    body = {"side": "buy", "asset": {"chain": "solana", "address": "0xabc"},
            "venue": {"kind": "dex"}, "amount": {"mode": "percent", "value": 5},
            "slippageBps": 500}
    env = build_envelope("trade", "market text buy XXX 5%", "Agent#12", "svc_9",
                         "okx-signal-trade-v1", signal_id="dlv_1", body=body)
    txt = render_deliverable(env)
    check(env["header"]["signalClass"] == "trade" and env["body"]["side"] == "buy",
          "build: trade envelope fields present")
    check(txt.startswith("```json") and "market text buy XXX 5%" in txt,
          "render: fenced envelope + raw retained")
    e2, raw = parse_envelope(txt)
    check(e2 is not None and raw == "market text buy XXX 5%" and e2["body"]["amount"]["value"] == 5,
          "parse: envelope extracted, raw kept intact")
    check(validate(e2, "strict") == [], "strict validate passes complete trade")
    bad = dict(e2); bad["body"] = {"side": "buy"}
    issues = validate(bad, "strict")
    check(any("missing trade field 'asset'" in i for i in issues), "strict rejects missing asset")
    # raw-only (loose fallback)
    e3, r3 = parse_envelope("just some text, no envelope")
    check(e3 is None and r3 == "just some text, no envelope", "raw-only parses as None envelope")
    # security alert minimal
    ea = build_envelope("security_alert", "rug pull detected", "Agent#12", "svc_9",
                        "okx-signal-alert-v1", body={"alertType": "rug_pull", "urgency": 3,
                                                     "chain": "bsc", "targets": ["0x1"]})
    check(validate(ea, "loose") == [], "security_alert validates")
    # trust-asps gate in strict
    issues = validate(dict(e2), "strict", trust_asps=["Agent#99"])
    check(any("not in trustAsps" in i for i in issues), "strict refuses untrusted sender")
    # expiry
    envx = build_envelope("trade", "x", "Agent#12", "svc_9", "tpl",
                          expires_at=now_ms() - 1000, signal_id="dlv_2", body=body)
    check(any("expired" in i for i in validate(envx, "strict")), "expired signal rejected")
    print("selftest:", "OK" if not fails else f"{len(fails)} FAIL")
    return 0 if not fails else 1

if __name__ == "__main__":
    main()
