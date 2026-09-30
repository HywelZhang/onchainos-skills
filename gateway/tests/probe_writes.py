#!/usr/bin/env python3
"""P1 写路径验证（**可逆、不花钱**）。

覆盖 v1.2 边界下两种写执行体：
    1. 原生 HTTP 写（无签名）：`sub.offline`（离线接收标记）往返、`sub.device`（设备列表清空幂等）
    2. 委托写（CLI 执行器）：`cli.raw agent get-my-agents`（只读命令，验证委托管道本身）

设计原则：**先读现状 → 写回同一语义 → 核对**，绝不留下状态漂移。
（踩过的坑：`deviceList: []` ≠ `null`；前者会让该设备彻底收不到订阅信号。）

用法: python tests/probe_writes.py [--sub-id 0x...]
退出码: 0 全通。
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from okxai import delegate, verbs, verbs_write, session  # noqa: E402,F401

failures: list[str] = []


def check(label: str, ok: bool, detail: str = "") -> None:
    print(f"[{'ok ' if ok else 'ERR'}] {label}{(' — ' + detail) if detail else ''}")
    if not ok:
        failures.append(label)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sub-id", default=None, help="默认取订阅列表里第一条")
    args = ap.parse_args()

    s = session.Session.load()

    subs = verbs.run("sub.list", session=s)
    rows = subs.get("list") if isinstance(subs, dict) else []
    if not rows:
        print("没有订阅可测（先创建一个订阅）")
        return 2
    sub_id = args.sub_id or rows[0]["jobId"]
    print(f"目标订阅: {sub_id}\n")

    # ── 1) 离线接收标记往返（0 → 1 → 回到原值） ──────────────────────
    before = verbs.run("sub.detail", session=s, sub_id=sub_id).get("offlineReceiveFlag")
    print(f"离线标记现值 = {before}")
    for target in (1 - int(before), int(before)):
        verbs.run("sub.offline", session=s, sub_id=sub_id, flag=target)
        got = verbs.run("sub.detail", session=s, sub_id=sub_id).get("offlineReceiveFlag")
        check(f"sub.offline 写入 {target} 后服务端读回", int(got) == int(target), f"read={got}")

    # ── 2) 设备列表：清空语义幂等（省略字段 = 所有设备收） ────────────
    row = next((r for r in verbs.run("sub.list", session=s).get("list", []) if r.get("jobId") == sub_id), {})
    check("清空前 thisDeviceReceives 为 true（所有设备收）",
          row.get("thisDeviceReceives") is True, f"deviceList={row.get('deviceList')}")
    verbs.run("sub.device", session=s, job_id=sub_id, device_list=None)
    row2 = next((r for r in verbs.run("sub.list", session=s).get("list", []) if r.get("jobId") == sub_id), {})
    check("sub.device 清空后仍为 null/所有设备收",
          row2.get("deviceList") is None and row2.get("thisDeviceReceives") is True,
          f"deviceList={row2.get('deviceList')} thisDeviceReceives={row2.get('thisDeviceReceives')}")

    # ── 3) 委托执行器管道（只读命令） ────────────────────────────────
    res = verbs.run("cli.raw", session=s, command="agent get-my-agents")
    check("委托执行 agent get-my-agents 成功", bool(res.get("ok")),
          f"exit={res.get('exit')} {res.get('duration_ms')}ms executor={res.get('executor')}")
    check("越权命令被拒绝", _rejects())

    print("\nPASS" if not failures else f"\nFAIL: {failures}")
    return 0 if not failures else 1


def _rejects() -> bool:
    try:
        delegate.run(["wallet", "transfer", "--to", "x"])
        return False
    except delegate.DelegateError:
        return True


if __name__ == "__main__":
    raise SystemExit(main())
