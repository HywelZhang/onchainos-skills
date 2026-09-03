#!/usr/bin/env python3
"""executor-lite.py — deterministic buyer-side driver for an A2A single task.

Implements docs/design/01 buyer.task flow with LLM-free mechanical nodes:
  publish (create-task) -> watch (accept/deliverable) -> decrypt download
  -> rule-review (content heuristics; uncertain escalates to human)
  -> complete (release escrow)  [+ pending-decisions resolve fallback]

Everything shells out to the official CLIs (onchainos / okx-a2a). No LLM in the
loop. Chain-touching steps require --live (default is dry-run).

Usage:
  python scripts/executor-lite.py --dryrun \
      --service-id <UUID> --provider-agent <aspAgentId> --agent-id <myAgentId> \
      --title T --description D --budget 0.1
  python scripts/executor-lite.py --live ...          # real run (money!)
  python scripts/executor-lite.py --selftest          # offline tests
"""
import argparse, json, os, re, subprocess, sys, time

TERMINAL = ["[任务已完成]", "[Job Completed]", "[Job Auto-Completed]", "[任务已关闭]"]
ACCEPT_MARK = ["[任务已接受]", "[Job Accepted]"]
DELIVER_MARK = ["[Received]", "[Deliverable Received]", "fileKey"]

def run(cmd, dryrun=False, timeout=120):
    if dryrun:
        print("  DRYRUN>", " ".join(cmd)); return {"dryrun": True}
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return {"rc": r.returncode, "out": r.stdout, "err": r.stderr}
    except subprocess.TimeoutExpired:
        return {"rc": -1, "out": "", "err": "timeout"}

def find_key(obj, key, depth=0):
    if depth > 6: return None
    if isinstance(obj, dict):
        if key in obj: return obj[key]
        for v in obj.values():
            r = find_key(v, key, depth+1)
            if r is not None: return r
    elif isinstance(obj, list):
        for v in obj:
            r = find_key(v, key, depth+1)
            if r is not None: return r
    return None

def parse_json(text):
    try:
        return json.loads(text)
    except Exception:
        return None

# ── publish ─────────────────────────────────────────────────────────────
def publish(a, dryrun):
    cmd = ["onchainos", "agent", "create-task",
           "--description", a.description, "--budget", str(a.budget),
           "--max-budget", str(a.budget), "--currency", a.currency,
           "--provider", a.provider_agent, "--payment-mode", a.payment_mode,
           "--service-id", a.service_id]
    if a.title: cmd += ["--title", a.title]
    r = run(cmd, dryrun)
    if dryrun: return None, r
    d = parse_json(r["out"])
    job_id = find_key(d, "jobId") or find_key(d, "job_id")
    if not job_id:
        print("  [executor] create-task: no jobId in output; rc=", r.get("rc"))
        print("  raw:", (r.get("out") or r.get("err") or "")[:500])
        sys.exit(3)
    print(f"  [executor] published jobId={job_id}")
    return job_id, r

# ── watch loop ──────────────────────────────────────────────────────────
def watch_until(job_id, a, dryrun):
    """Poll scoped watch until accept+deliverable or terminal/decision/timeout."""
    got_accept, dl = False, None
    deadline = time.time() + a.watch_timeout
    while time.time() < deadline:
        cmd = ["okx-a2a", "user", "watch", "--json", "--job-id", job_id,
               "--timeout", "50", "--once"]
        r = run(cmd, dryrun, timeout=70)
        if dryrun: return True, None
        text = r.get("out", "")
        if not text.strip():
            continue
        if any(m in text for m in TERMINAL):
            print("  [executor] terminal event seen:", [m for m in TERMINAL if m in text])
            return True, dl
        if any(m in text for m in ACCEPT_MARK):
            got_accept = True
            print("  [executor] accepted by provider")
        if any(m in text for m in DELIVER_MARK) and ("fileKey" in text or "digest" in text):
            dl = extract_deliverable(text)
            print("  [executor] deliverable params captured")
            return True, dl
    print("  [executor] watch timeout without full lifecycle (accept=%s dl=%s)" % (got_accept, bool(dl)))
    return got_accept and bool(dl), dl

def extract_deliverable(text):
    def grab(pat):
        m = re.search(pat, text)
        return m.group(1) if m else None
    return {"fileKey": grab(r'fileKey["\']?\s*[:=]\s*["\']?([A-Za-z0-9_\-\.]+)'),
            "digest": grab(r'digest["\']?\s*[:=]\s*["\']?([A-Za-z0-9]+)'),
            "salt": grab(r'salt["\']?\s*[:=]\s*["\']?([A-Za-z0-9+\/=]+)'),
            "nonce": grab(r'nonce["\']?\s*[:=]\s*["\']?([A-Za-z0-9+\/=]+)'),
            "secret": grab(r'secret["\']?\s*[:=]\s*["\']?([A-Za-z0-9+\/=]+)')}

# ── download + decrypt ──────────────────────────────────────────────────
def download(dl, agent_id, out_dir, dryrun):
    cmd = ["okx-a2a", "file", "download", "--file-key", dl["fileKey"],
           "--agent-id", agent_id, "--digest", dl["digest"], "--salt", dl["salt"],
           "--nonce", dl["nonce"], "--secret", dl["secret"]]
    r = run(cmd, dryrun, timeout=120)
    if dryrun: return None
    d = parse_json(r["out"])
    path = None
    if isinstance(d, dict):
        path = find_key(d, "path") or find_key(d, "savedPath") or find_key(d, "filePath")
    if not path:
        m = re.search(r'([A-Za-z]:[\\\/][^\s"\']+\.md)', r.get("out", ""))
        path = m.group(1) if m else None
    if path and os.path.exists(path):
        print(f"  [executor] deliverable saved: {path}")
        return path
    print("  [executor] download: no saved path resolved. raw:", (r.get("out") or "")[:300])
    return None

# ── rule review ─────────────────────────────────────────────────────────
def rule_review(path):
    if not path or not os.path.exists(path):
        return "UNCERTAIN", "no deliverable file"
    t = open(path, encoding="utf-8", errors="ignore").read()
    checks = {
        "length>200": len(t) > 200,
        "weekday coverage": sum(1 for d in ["周一","周二","周三","周四","周五","周六","周日","星期一","星期天"] if d in t) >= 4,
        "nutrition terms": any(k in t for k in ["蛋白质", "热量", "kcal", "卡路里"]),
    }
    ok = all(checks.values())
    print("  [executor] rule-review:", checks)
    return ("PASS" if ok else "UNCERTAIN"), json.dumps(checks, ensure_ascii=False)

# ── complete ────────────────────────────────────────────────────────────
def complete(job_id, dryrun):
    r = run(["onchainos", "agent", "complete", job_id], dryrun)
    if dryrun: return
    text = (r.get("out", "") + r.get("err", ""))
    print("  [executor] complete rc=%s" % r.get("rc"))
    if r.get("rc") == 0 and "ok" in text.lower():
        print("  [executor] completed (escrow released)")
        return True
    print("  [executor] complete blocked/delayed; review gate likely pending.")
    print("  fallback hint: onchainos agent pending-decisions-v2 resolve-with-sessionkey --job-id %s" % job_id)
    return False

# ── main ────────────────────────────────────────────────────────────────
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--service-id")
    ap.add_argument("--provider-agent")
    ap.add_argument("--agent-id", help="receiver agentId for file download")
    ap.add_argument("--title", default="")
    ap.add_argument("--description")
    ap.add_argument("--budget", type=float, default=0.1)
    ap.add_argument("--currency", default="USDT")
    ap.add_argument("--payment-mode", default="escrow")
    ap.add_argument("--watch-timeout", type=int, default=240)
    ap.add_argument("--out-dir", default="./state/executor")
    ap.add_argument("--dryrun", action="store_true")
    ap.add_argument("--live", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(selftest())
    for req in ("service_id", "provider_agent", "agent_id", "description"):
        if not getattr(a, req):
            print(f"missing required: --{req.replace('_', '-')}")
            sys.exit(2)
    if not a.live and not a.dryrun:
        print("safety: pass --dryrun to preview or --live to actually run (money/chain).")
        sys.exit(2)
    os.makedirs(a.out_dir, exist_ok=True)
    print("== executor-lite ==")
    job_id, _ = publish(a, a.dryrun)
    if not a.dryrun:
        ok, dl = watch_until(job_id, a, False)
        path = download(dl, a.agent_id, a.out_dir, False) if dl else None
        verdict, detail = rule_review(path)
        print(f"  [executor] review verdict: {verdict} {detail}")
        if verdict == "PASS":
            complete(job_id, False)
        else:
            print("  [executor] human review required (rule UNCERTAIN); not completing.")
    else:
        print("  [executor] dryrun plan above; job NOT published.")

def selftest():
    fails = []
    def check(c, m):
        print(("PASS" if c else "FAIL"), m)
        if not c: fails.append(m)
    # publish arg builder
    class A: pass
    a = A(); a.description="d"; a.budget=0.1; a.currency="USDT"; a.provider_agent="11198"; a.payment_mode="escrow"; a.service_id="205aa54c-20db-4a9d-acfb-36bb0d843c6d"; a.title="t"; a.watch_timeout=10
    # extract deliverable from realistic event text
    txt = 'deliverable received fileKey=abc123 digest=deadbeef salt=SALT nonce=NONCE secret=SECRET [Received]'
    dl = extract_deliverable(txt)
    check(dl["fileKey"] == "abc123" and dl["digest"] == "deadbeef", "deliverable params extracted")
    # review heuristics
    import tempfile, os
    good = ("周一 晚餐: 清蒸鲈鱼150g + 糙米饭100g + 蒜蓉西兰花200g, 蛋白质约38g, 热量约380kcal\n"
            "周二 晚餐: 鸡胸肉炒时蔬200g + 玉米半根, 蛋白质约42g, 热量约360kcal\n"
            "周三 晚餐: 番茄豆腐鱼片汤 + 全麦馒头半个, 蛋白质约35g, 热量约350kcal\n"
            "周四 晚餐: 虾仁蒸蛋 + 凉拌黄瓜 + 杂粮饭80g, 蛋白质约36g, 热量约370kcal\n"
            "周五 晚餐: 香煎三文鱼120g + 烤蔬菜, 蛋白质约34g, 热量约400kcal\n"
            "周六 晚餐: 牛肉炒芹菜 + 紫薯一个, 蛋白质约40g, 热量约390kcal\n"
            "周日 晚餐: 白灼虾200g + 青菜豆腐汤 + 少量荞麦面, 蛋白质约45g, 热量约420kcal\n"
            "做法均以蒸煮炖为主, 每餐用油不超过5ml, 全天控糖。\n"
            "减脂期饮食建议: 1) 每餐先吃蔬菜再吃蛋白质最后主食; 2) 每天饮水2000ml以上; 3) 晚餐尽量在睡前3小时完成。")
    with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False, encoding="utf-8") as f:
        f.write(good); p = f.name
    v, _ = rule_review(p)
    check(v == "PASS", "review passes on complete 7-day plan")
    os.unlink(p)
    with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False, encoding="utf-8") as f:
        f.write("hello"); p = f.name
    v, _ = rule_review(p)
    check(v == "UNCERTAIN", "review uncertain on junk")
    os.unlink(p)
    print("selftest:", "OK" if not fails else f"{len(fails)} FAIL")
    return 0 if not fails else 1

if __name__ == "__main__":
    main()
