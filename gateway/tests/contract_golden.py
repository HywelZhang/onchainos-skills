#!/usr/bin/env python3
"""P0 契约测试：gateway verb 输出 vs golden（CLI 录制基准）逐字段对照。

口径（tests/cli-golden/README.md）:
    * **方向**: golden 的业务字段必须全部在 gateway 输出中存在且相等（单向包含）；
      gateway 多出的字段只记录、不算失败（新增能力）。
    * **忽略**: CLI 自己加工的展示层字段（card/cells/statusLabel/…）与渲染文本。
    * **断言**: 任一 golden 业务字段缺失或不等 → 该 verb FAIL，并给出差异明细。

用法:
    python tests/contract_golden.py                     # 自动取最新 golden
    python tests/contract_golden.py --golden <dir> --report <file.json>
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from okxai import home, session, verbs  # noqa: E402

IGNORE_KEYS = {
    # CLI 本地加工的展示层：不是服务端业务字段
    "card", "cells", "statusLabel", "statusDescription", "approvalLabel",
    "ratingStars", "tip", "roleLabel", "displayStatus", "summaryLine",
    # CLI 本地派生
    "shortId", "source", "thisDeviceName",
    # 非确定性/CLI 自有诊断措辞：分页游标每次检索都可能不同；hint 是 CLI 自检器的说明文案
    "searchAfter", "hint",
}


def newest_golden() -> Path | None:
    root = home.path("golden", "cli")
    if not root.exists():
        return None
    dirs = sorted([p for p in root.iterdir() if p.is_dir()])
    return dirs[-1] if dirs else None


def norm(value: Any) -> Any:
    """数值/字符串归一（服务端可能给 int 或 str）。"""
    if isinstance(value, bool):
        return value
    if isinstance(value, (int, float)):
        return f"{value:g}" if isinstance(value, float) else str(value)
    if isinstance(value, str):
        v = value.strip()
        try:
            return f"{float(v):g}"
        except ValueError:
            return v
    return value


def deep_compare(golden: Any, actual: Any, path: str, diffs: list, matches: list) -> None:
    if isinstance(golden, dict):
        if not isinstance(actual, dict):
            diffs.append({"path": path, "kind": "type", "golden": type(golden).__name__, "actual": type(actual).__name__})
            return
        for key, gval in golden.items():
            if key in IGNORE_KEYS:
                continue
            if key not in actual:
                diffs.append({"path": f"{path}.{key}", "kind": "missing", "golden": _short(gval)})
                continue
            deep_compare(gval, actual[key], f"{path}.{key}", diffs, matches)
        return

    if isinstance(golden, list):
        if not isinstance(actual, list):
            diffs.append({"path": path, "kind": "type", "golden": "list", "actual": type(actual).__name__})
            return
        if len(golden) != len(actual):
            diffs.append({"path": path, "kind": "length", "golden": len(golden), "actual": len(actual)})
        for idx, gval in enumerate(golden):
            if idx < len(actual):
                deep_compare(gval, actual[idx], f"{path}[{idx}]", diffs, matches)
        return

    if norm(golden) == norm(actual):
        matches.append(path)
    else:
        diffs.append({"path": path, "kind": "value", "golden": _short(golden), "actual": _short(actual)})


def _short(value: Any, limit: int = 80) -> Any:
    if isinstance(value, str) and len(value) > limit:
        return value[:limit] + "…"
    return value


def golden_payload(entry: dict) -> tuple[Any, str]:
    """返回 (payload, kind)。kind ∈ json|text。"""
    raw = entry.get("stdout") or ""
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError:
        return raw, "text"
    if isinstance(parsed, dict) and "data" in parsed:
        return parsed["data"], "json"
    return parsed, "json"


def argv_value(entry: dict, prefix: str) -> str | None:
    """从 argv 里取 `--flag value` 形式的值（返回紧跟其后的 token）。"""
    argv = [str(a) for a in entry.get("argv", [])]
    for idx, item in enumerate(argv):
        if item == prefix and idx + 1 < len(argv):
            return argv[idx + 1]
        if item.startswith(prefix + "="):
            return item.split("=", 1)[1]
    # 兜底：裸位置参数（如 `task status <jobId>`）
    if prefix.startswith("0x"):
        for item in argv:
            if item.startswith("0x"):
                return item
    return None


def plan(golden_name: str, entry: dict) -> tuple[str, dict[str, Any]] | None:
    """golden 条目 → (gateway verb, kwargs)。None = 本轮不做对照（已在报告里说明）。"""
    if golden_name == "session.wallet_status":
        return "wallet.status", {}
    if golden_name == "session.device_list":
        return "session.devices", {}
    if golden_name == "agent.mine":
        return "agent.mine", {"page_size": 5}
    if golden_name == "agent.mine_role_user":
        return "agent.mine", {"role": "user", "page_size": 5}
    if golden_name == "agent.flat":
        return "agent.flat", {}
    if golden_name == "agent.get":
        ids = argv_value(entry, "--agent-ids")
        return ("agent.get", {"agent_ids": ids}) if ids else None
    if golden_name == "gate.check":
        return "gate.check", {}
    if golden_name == "task.active":
        return "task.active", {}
    if golden_name == "task.deliverables":
        job = argv_value(entry, "--job-id")
        return ("task.deliverables", {"job_id": job}) if job else None
    if golden_name == "sub.list":
        return "sub.list", {}
    if golden_name == "sub.detail":
        sub = argv_value(entry, "0x")
        return ("sub.detail", {"sub_id": sub}) if sub else None
    if golden_name == "sub.cost":
        return "sub.cost", {}
    if golden_name == "service.match":
        return "service.match", {"keywords": "signal"}
    if golden_name == "flow.pending_decisions":
        return "flow.pending_decisions", {}
    if golden_name == "task.mine":
        return "task.mine", {}
    if golden_name == "task.status":
        job = argv_value(entry, "0x")
        return ("task.status", {"job_id": job}) if job else None
    return None


TEXT_ASSERTIONS: dict[str, list[tuple[str, str]]] = {
    "task.mine": [(r"Task list \((\d+) total", "total")],
    # golden 的文本渲染 → 用正则抓关键事实，与 gateway 的 JSON 字段比对
    "task.status": [(r"Task status:\s*(\w+)", "statusName")],
    "sub.detail": [(r"subId:\s*(0x[0-9a-f]+)", "jobId")],
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--golden", default=None)
    ap.add_argument("--report", default=None)
    ap.add_argument("--only", default=None, help="只跑某个 golden 条目")
    args = ap.parse_args()

    gdir = Path(args.golden) if args.golden else newest_golden()
    if not gdir or not (gdir / "manifest.json").exists():
        print(f"no golden manifest found (looked in {gdir})")
        return 2

    manifest = json.loads((gdir / "manifest.json").read_text(encoding="utf-8"))

    # 逐条 golden 的**完整**输出在 `NN_<name>.json` 里（manifest 只存 sha256/大小）——
    # 一开始只读 manifest 导致 stdout 为空、对照退化成"0 字段全 PASS"的假通过。
    records: dict[str, dict] = {}
    for path in sorted(gdir.glob("[0-9][0-9]_*.json")):
        try:
            rec = json.loads(path.read_text(encoding="utf-8"))
        except Exception:  # noqa: BLE001
            continue
        records[str(rec.get("name"))] = rec
    if not records:
        print(f"no per-command recordings found in {gdir} (expected NN_<name>.json)")
        return 2

    s = session.Session.load()
    results = []
    skipped = []

    for meta in manifest["entries"]:
        name = meta["name"]
        entry = records.get(name, meta)
        if args.only and name != args.only:
            continue
        if entry.get("exit") != 0:
            skipped.append({"golden": name, "why": f"golden exit={entry['exit']}（CLI 参数面差异）"})
            continue
        mapped = plan(name, entry)
        if not mapped:
            skipped.append({"golden": name, "why": "无对应 gateway verb（本轮不覆盖）"})
            continue
        verb_name, kwargs = mapped
        payload, kind = golden_payload(entry)
        t0 = time.time()
        try:
            actual = verbs.run(verb_name, session=s, **kwargs)
        except Exception as exc:  # noqa: BLE001
            results.append({"golden": name, "verb": verb_name, "status": "ERROR",
                            "error": f"{type(exc).__name__}: {exc}"})
            continue
        elapsed = int((time.time() - t0) * 1000)

        if kind == "text":
            checks = TEXT_ASSERTIONS.get(name, [])
            text = payload if isinstance(payload, str) else ""
            ok_fields, bad_fields = [], []
            for pattern, key in checks:
                m = re.search(pattern, text)
                if not m:
                    continue
                want = m.group(1)
                got = actual.get(key) if isinstance(actual, dict) else None
                if norm(want) == norm(got):
                    ok_fields.append(key)
                else:
                    bad_fields.append({"field": key, "golden": want, "actual": got})
            status = "PASS" if not bad_fields else "FAIL"
            results.append({"golden": name, "verb": verb_name, "status": status, "kind": "text",
                            "matched": len(ok_fields), "diffs": bad_fields, "elapsed_ms": elapsed,
                            "note": "golden 是文本渲染；只比对可提取的关键事实"})
            continue

        diffs: list = []
        matches: list = []
        deep_compare(payload, actual, "$", diffs, matches)
        results.append({
            "golden": name, "verb": verb_name,
            "status": "PASS" if not diffs else "FAIL",
            "kind": "json", "matched_fields": len(matches), "diffs": diffs, "elapsed_ms": elapsed,
        })

    passed = [r for r in results if r["status"] == "PASS"]
    report = {
        "kind": "gateway-contract-report",
        "golden_dir": str(gdir),
        "cli_version": manifest.get("cli_version"),
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "summary": {
            "compared": len(results),
            "pass": len(passed),
            "fail": len([r for r in results if r["status"] == "FAIL"]),
            "error": len([r for r in results if r["status"] == "ERROR"]),
            "skipped": len(skipped),
        },
        "results": results,
        "skipped": skipped,
    }

    out = Path(args.report) if args.report else home.path("golden", "reports", f"contract-{time.strftime('%Y%m%d-%H%M%S')}.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    for r in results:
        if r["status"] == "PASS":
            extra = f"fields={r.get('matched_fields', r.get('matched'))}"
            print(f"PASS {r['golden']:24s} → {r['verb']:22s} {extra} {r['elapsed_ms']}ms")
        else:
            print(f"{r['status']} {r['golden']:24s} → {r['verb']:22s} diffs={len(r.get('diffs', []))} {r.get('error', '')}")
            for d in r.get("diffs", [])[:6]:
                print(f"      {d}")
    for sk in skipped:
        print(f"SKIP {sk['golden']:24s} {sk['why']}")
    print(f"\nsummary: {report['summary']}\nreport: {out}")
    return 0 if report["summary"]["fail"] == 0 and report["summary"]["error"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
