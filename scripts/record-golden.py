#!/usr/bin/env python3
"""Record golden CLI baselines (P0-11 §4.4 / §7 P0.0).

Purpose
-------
The okx-ai gateway (P0-11) replaces the official `onchainos` / `okx-a2a` CLIs with
direct HTTP calls. Once the CLIs stop being used, their exact output shape can no
longer be reproduced — so the read-path baseline must be recorded *beforehand*
and kept as the regression oracle for every later phase.

Privacy
-------
Recordings contain real account data (agentId, wallet addresses, jobId, amounts).
They are therefore written OUTSIDE the repository (default: $ONCHAINOS_HOME/golden)
and never committed. Only this script and a redacted manifest (command names, exit
codes, output sha256, sizes) are safe to commit.

Usage
-----
    python scripts/record-golden.py                     # default read-path set
    python scripts/record-golden.py --include-write     # + guarded write probes (asks first)
    python scripts/record-golden.py --job-id 0x... --sub-id 0x... --agent-id 2366
    python scripts/record-golden.py --list              # show the command set, record nothing
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

DEFAULT_TIMEOUT_S = 120

# Read-path verbs mirrored by gateway P0 verbs (P0-11 §4.2).
READ_COMMANDS: list[tuple[str, list[str]]] = [
    ("session.wallet_status", ["wallet", "status"]),
    ("session.device_list", ["agent", "device-list"]),
    ("agent.mine", ["agent", "get-my-agents"]),
    ("agent.mine_role_user", ["agent", "get-my-agents", "--role", "user"]),
    ("agent.flat", ["agent", "my-agents"]),
    ("agent.get", ["agent", "get-agents", "--agent-ids", "{agent_id}"]),
    ("agent.profile", ["agent", "profile", "--agent-id", "{agent_id}"]),
    ("gate.check", ["agent", "gate-check", "--role", "user"]),
    ("task.mine", ["agent", "tasks"]),
    ("task.active", ["agent", "active-tasks"]),
    ("task.status", ["agent", "status", "{job_id}"]),
    ("task.deliverables", ["agent", "task-deliverable-list", "--job-id", "{job_id}", "--role", "user"]),
    ("sub.list", ["agent", "my-subscriptions"]),
    ("sub.detail", ["agent", "subscribe-detail", "{sub_id}"]),
    ("sub.cost", ["agent", "subscribe-cost"]),
    ("flow.pending_decisions", ["agent", "pending-decisions-v2", "list"]),
]

# Write-path probes: recorded ONLY as "observed shape" for P1 comparison.
# They are never executed by default; each one must be explicitly enabled.
WRITE_COMMANDS: list[tuple[str, list[str]]] = [
    ("write.service_match_SKIPPED", []),  # placeholder: read-only search, enable via --enable-search
]

SEARCH_COMMANDS: list[tuple[str, list[str]]] = [
    ("service.match", ["agent", "service-match", "--keywords", "signal"]),
    ("agent.search", ["agent", "search", "--keywords", "signal"]),
]


def resolve_cli(explicit: str | None) -> str:
    if explicit:
        return explicit
    for name in ("onchainos.exe", "onchainos"):
        found = shutil.which(name)
        if found:
            return found
    sys.exit("onchainos CLI not found on PATH; pass --cli <path>")


def cli_version(cli: str) -> str:
    try:
        out = subprocess.run([cli, "--version"], capture_output=True, text=True, timeout=30)
        return (out.stdout or out.stderr).strip()
    except Exception as exc:  # noqa: BLE001
        return f"<version probe failed: {exc}>"


def record(cli: str, name: str, argv: list[str], outdir: Path, index: int, timeout: int) -> dict:
    started = time.time()
    try:
        proc = subprocess.run([cli, *argv], capture_output=True, timeout=timeout)
        stdout = proc.stdout.decode("utf-8", "replace")
        stderr = proc.stderr.decode("utf-8", "replace")
        exit_code = proc.returncode
    except subprocess.TimeoutExpired:
        stdout, stderr, exit_code = "", f"<timeout after {timeout}s>", -9
    duration_ms = int((time.time() - started) * 1000)

    entry = {
        "name": name,
        "argv": argv,
        "exit": exit_code,
        "duration_ms": duration_ms,
        "stdout_bytes": len(stdout.encode("utf-8")),
        "stderr_bytes": len(stderr.encode("utf-8")),
        "stdout_sha256": hashlib.sha256(stdout.encode("utf-8")).hexdigest(),
        "recorded_at": _dt.datetime.now().astimezone().isoformat(timespec="seconds"),
    }
    safe = name.replace("/", "_")
    payload = dict(entry, stdout=stdout, stderr=stderr)
    (outdir / f"{index:02d}_{safe}.json").write_text(
        json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return entry


def main() -> int:
    ap = argparse.ArgumentParser(description="Record golden CLI baselines for P0-11")
    ap.add_argument("--cli", default=None, help="path to the onchainos executable")
    ap.add_argument("--outdir", default=None, help="output dir (default $ONCHAINOS_HOME/golden/cli/<ts>)")
    ap.add_argument("--job-id", default=os.environ.get("GOLDEN_JOB_ID", ""))
    ap.add_argument("--sub-id", default=os.environ.get("GOLDEN_SUB_ID", ""))
    ap.add_argument("--agent-id", default=os.environ.get("GOLDEN_AGENT_ID", ""))
    ap.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_S)
    ap.add_argument("--enable-search", action="store_true", help="also record service-match / agent search")
    ap.add_argument("--list", action="store_true", help="print the command set and exit")
    args = ap.parse_args()

    commands = list(READ_COMMANDS)
    if args.enable_search:
        commands += SEARCH_COMMANDS

    if args.list:
        for name, argv in commands:
            print(f"{name}: onchainos {' '.join(argv)}")
        return 0

    missing = [n for n, a in commands if any(v in a for v in ("{job_id}", "{sub_id}", "{agent_id}")) and not
               {"{job_id}": args.job_id, "{sub_id}": args.sub_id, "{agent_id}": args.agent_id}[
                   next(v for v in ("{job_id}", "{sub_id}", "{agent_id}") if v in a)
               ]]
    if missing:
        print("refusing to record: missing ids for " + ", ".join(missing), file=sys.stderr)
        print("pass --job-id/--sub-id/--agent-id (or GOLDEN_JOB_ID/GOLDEN_SUB_ID/GOLDEN_AGENT_ID)", file=sys.stderr)
        return 2

    home = Path(os.environ.get("ONCHAINOS_HOME") or (Path.home() / ".onchainos"))
    ts = _dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    outdir = Path(args.outdir) if args.outdir else home / "golden" / "cli" / ts
    outdir.mkdir(parents=True, exist_ok=True)

    cli = resolve_cli(args.cli)
    version = cli_version(cli)
    print(f"cli      : {cli}")
    print(f"version  : {version}")
    print(f"outdir   : {outdir}\n")

    manifest = {
        "kind": "cli-golden-baseline",
        "plan": "docs/design/11-cli-to-api-refactor.md §4.4/§7 P0.0",
        "cli": cli,
        "cli_version": version,
        "recorded_at": _dt.datetime.now().astimezone().isoformat(timespec="seconds"),
        "entries": [],
    }

    for idx, (name, argv) in enumerate(commands, start=1):
        argv = [a.format(job_id=args.job_id, sub_id=args.sub_id, agent_id=args.agent_id) for a in argv]
        entry = record(cli, name, argv, outdir, idx, args.timeout)
        manifest["entries"].append(entry)
        flag = "ok " if entry["exit"] == 0 else "ERR"
        print(f"[{flag}] {name:26s} {entry['duration_ms']:>7d}ms {entry['stdout_bytes']:>7d}B  exit={entry['exit']}")

    ok = sum(1 for e in manifest["entries"] if e["exit"] == 0)
    manifest["summary"] = {"total": len(manifest["entries"]), "ok": ok, "failed": len(manifest["entries"]) - ok}
    (outdir / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\nrecorded {ok}/{len(manifest['entries'])} ok -> {outdir / 'manifest.json'}")
    print("NOTE: recordings contain account data — keep them out of git.")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
