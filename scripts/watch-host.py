#!/usr/bin/env python3
"""watch-host.py — headless event listener wrapping `okx-a2a user watch --json`.

Reuses the closed-source okx-a2a CLI as transport (auth/queue/device routing),
consumes its stdout batches deterministically (no LLM in the loop), normalizes
events to JSONL, decides re-enter vs stop by RULE (watch-core semantics), and
notifies via console (v1). Future: policy-engine intake + telegram adapter.

Usage:
  python scripts/watch-host.py                 # loop (supervisor keeps alive)
  python scripts/watch-host.py --once          # single batch (cron heartbeat / debug)
  python scripts/watch-host.py --once --job-id JOB --cmd 'okx-a2a' --event-dir ./state
  python scripts/watch-host.py --selftest      # offline fixture test, no network

Exit: 0 = stopped by rule (scoped terminal / user stop); 2 = fatal (backoff exhausted).
"""
import argparse, hashlib, json, os, subprocess, sys, time

# ── terminal markers (watch-core stop-condition list — the ONLY stops) ──
TERMINAL_MARKERS = ["[Job Completed]", "[Job Auto-Completed]", "[x402 Job Completed]",
                    "[Job Expired]", "[Job Closed]", "[Refund Settled]", "[Auto-Refund Settled]"]
# markers that LOOK terminal but are NOT (mid-flow → keep watching)
MID_FLOW_MARKERS = ["[Deliverable Received]", "[x402 Deliverable Received]", "[Job Accepted]",
                    "[Payment Mode Set]", "[Connecting ASP]", "[Job Created]",
                    "[x402 Replay Failed]", "[Rejection Confirmed]", "[📝 Rating Submitted]"]
WAKE_PROMPT = "Re-enter watch now: okx-a2a user watch --json"

def now(): return int(time.time())

def normalize(raw_text: str, job_id: str | None, sticky: bool) -> dict:
    """Classify one watch payload into a normalized event (schema per doc 06 §4)."""
    text = raw_text if isinstance(raw_text, str) else json.dumps(raw_text, ensure_ascii=False)
    markers = [m for m in TERMINAL_MARKERS + MID_FLOW_MARKERS if m in text]
    kind = "raw"
    low = text.lower()
    if "decision_request" in low or "pending decision" in low or "awaiting_decision" in low:
        kind = "decision_request"
    elif any(m in text for m in ["[Deliverable Received]", "deliverable", "signal"]) or "subscription" in low:
        kind = "signal" if ("signal" in low or "active_subscription" in low) else "task_event"
    elif any(m in text for m in TERMINAL_MARKERS + MID_FLOW_MARKERS) or "job" in low:
        kind = "task_event"
    elif text.strip():
        kind = "notification"
    terminal = any(m in text for m in TERMINAL_MARKERS)
    return {"ts": now(), "kind": kind, "jobId": job_id or "", "sticky": sticky,
            "terminal": terminal, "markers": markers, "raw": text, "source": "watch"}

def dedupe_key(ev: dict) -> str:
    return hashlib.sha1(f"{ev['jobId']}|{ev['kind']}|{ev['raw']}".encode()).hexdigest()

class EventStore:
    def __init__(self, event_dir: str):
        os.makedirs(event_dir, exist_ok=True)
        self.path = os.path.join(event_dir, "events.jsonl")
        self.seen = self._load_seen()
    def _load_seen(self):
        seen, p = set(), self.path
        if os.path.exists(p):
            for line in open(p, encoding="utf-8", errors="ignore"):
                try:
                    seen.add(json.loads(line)["_k"])
                except Exception:
                    pass
        return seen
    def push(self, ev: dict) -> bool:
        k = dedupe_key(ev)
        if k in self.seen:
            return False
        self.seen.add(k)
        with open(self.path, "a", encoding="utf-8") as f:
            f.write(json.dumps({**ev, "_k": k}, ensure_ascii=False) + "\n")
        return True

def notify_console(ev: dict):
    tag = {"decision_request": "⚠️ 需决策", "signal": "📥 订阅信号", "task_event": "📋 任务事件",
           "notification": "🔔 通知", "raw": "❓ 未识别"}[ev["kind"]]
    line = f"[{now()}] {tag}" + (f" job={ev['jobId']}" if ev["jobId"] else "") + \
           (f" terminal={ev['markers']}" if ev["terminal"] else "")
    print(line, flush=True)

def parse_batch(stdout: str):
    """okx-a2a watch stdout → list of payloads. Tolerant of list / {data:...} / text."""
    out = stdout.strip()
    if not out:
        return []
    try:
        obj = json.loads(out)
    except json.JSONDecodeError:
        return [out]
    if isinstance(obj, list):
        return obj
    if isinstance(obj, dict):
        d = obj.get("data", obj)
        for key in ("items", "notifications", "batch", "messages"):
            if isinstance(d, dict) and isinstance(d.get(key), list):
                return d[key]
        if isinstance(d, list):
            return d
        return [d]
    return [out]

def run_once(args, store: EventStore) -> int:
    cmd = [args.cmd, "user", "watch", "--json"]
    if args.job_id:
        cmd += ["--job-id", args.job_id]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=args.timeout)
    except subprocess.TimeoutExpired:
        print(f"[{now()}] watch long-poll timeout (no events) → re-enter", flush=True)
        return 0
    if r.returncode != 0:
        err = (r.stderr or r.stdout or "")[:500]
        print(f"[{now()}] watch error rc={r.returncode}: {err}", flush=True)
        return 1
    batch = parse_batch(r.stdout)
    if not batch:
        return 0
    scoped_terminal = False
    for payload in batch:
        text = payload.get("userContent") if isinstance(payload, dict) else str(payload)
        if isinstance(text, list):
            text = " ".join(str(x) for x in text)
        ev = normalize(text, args.job_id, sticky=bool(args.job_id))
        if store.push(ev):
            notify_console(ev)
        if args.job_id and ev["terminal"]:
            scoped_terminal = True
    if scoped_terminal:
        print(f"[{now()}] scoped job reached terminal state → stop (rule)", flush=True)
        return 0  # stop; supervisor decides whether to relaunch
    return 0  # otherwise: re-enter (loop)

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--once", action="store_true")
    ap.add_argument("--job-id", default=None)
    ap.add_argument("--cmd", default=os.environ.get("OKX_A2A_CMD", "okx-a2a"))
    ap.add_argument("--event-dir", default=os.environ.get("WATCH_EVENT_DIR", "./state/watch"))
    ap.add_argument("--timeout", type=int, default=120)
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(selftest())
    store = EventStore(a.event_dir)
    if a.once:
        rc = run_once(a, store)
        sys.exit(rc)
    # loop with backoff on process-level errors (timeouts/empty are NOT errors)
    backoff, attempts = 1, 0
    while True:
        rc = run_once(a, store)
        if rc == 0:
            attempts = 0
            backoff = 1
        else:
            attempts += 1
            if attempts >= 5:
                print(f"[{now()}] fatal: {attempts} consecutive process errors → exit 2", flush=True)
                sys.exit(2)
            time.sleep(backoff)
            backoff = min(backoff * 2, 30)

def selftest() -> int:
    import tempfile
    cases = [
        ("[Job Completed] deliverable accepted", "task_event", True, True),
        ("scoped [Job Auto-Completed]", "task_event", True, True),
        ("[Job Created] new task awaiting provider", "task_event", False, False),
        ("[Deliverable Received] check your inbox", "task_event", False, False),
        ("Pending decision_request auto-timeout reached. " + WAKE_PROMPT,
         "decision_request", False, False),
        ("subscription signal: buy XXX 5%", "signal", False, False),
        ("plain notification text", "notification", False, False),
    ]
    fails = 0
    for text, want_kind, want_terminal, _ in cases:
        ev = normalize(text, None, sticky=False)
        ok = ev["kind"] == want_kind and ev["terminal"] == want_terminal
        fails += 0 if ok else 1
        print(("PASS" if ok else "FAIL"), ev["kind"], ev["terminal"], "|", text[:60])
    # dedupe + terminal-stop decision
    with tempfile.TemporaryDirectory() as td:
        st = EventStore(td)
        ev = normalize("[Job Completed] x", "J1", True)
        assert st.push(ev) is True and st.push(ev) is False, "dedupe failed"
        print("PASS dedupe")
    print("selftest:", "OK" if fails == 0 else f"{fails} FAILURES")
    return 0 if fails == 0 else 1

if __name__ == "__main__":
    main()
