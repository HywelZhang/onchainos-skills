#!/usr/bin/env python3
"""sub-sim.py — buyer subscription flow simulator (offline, no chain/money).

Feeds realistic per-subscription events through the decision loop (watch-host
normalized JSONL shape -> policy decide -> hooks) and asserts the routed mode.
Phase-1 proof that the subscription pipeline is wired headless; same harness
replays recorded live events later (calibration, OQ-10).

Usage:
  python scripts/sub-sim.py                    # default: examples/policy, scope sub-36563
  python scripts/sub-sim.py --policy-dir <dir> --scope sub-XXXX --scenario f.json
"""
import argparse, importlib.util, json, os, sys

_SCRIPTS = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("decision_loop", os.path.join(_SCRIPTS, "decision-loop.py"))
dl = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(dl)  # noqa

DEFAULT_SCENARIO = [
    {"ev": {"kind": "signal", "jobId": "0x36563", "raw": "signal_type=analysis 方向参考 BTC 多周期共振, 入场区参考 60000-61500, 止损 59000, 仅供参考未下单"},
     "want": "notify", "node": "buyer.sub.signal_analysis", "note": "analysis 流每~3min 一轮 → notify, 不打扰"},
    {"ev": {"kind": "signal", "jobId": "0x36563", "raw": "signal_type=order action=open side=buy sz=0.01 杠杆=5x 价格=61200 已成交 id=9f2a"},
     "want": "ask", "node": "buyer.sub.signal_order", "note": "order 实单事件 → ask 人工确认(默认无 auto 授权)"},
    {"ev": {"kind": "signal", "jobId": "0x36563", "raw": "signal_type=analysis 扫盘完成 无高胜率机会 静默"},
     "want": "notify", "note": "analysis 静默期正常"},
    {"ev": {"kind": "signal", "jobId": "0x36563", "raw": "no recognizable type here, random text"},
     "want": "ask", "node": "buyer.sub.signal_received", "note": "未打标/无信封 signal → nodes.signal_received ask(未知内容安全处理, 节点优先于通配符)"},
    {"ev": {"kind": "decision_request", "jobId": "0x36563", "raw": "sub_user_reject 本期信号质量差, 请选择: A 接受 B 拒收"},
     "want": "ask", "node": "buyer.sub.decide", "note": "订阅期决策 → buyer.sub.decide ask"},
    {"ev": {"kind": "notification", "jobId": "0x36563", "raw": "sub_renew 续费窗口开启, 上期收益可提取"},
     "want": "notify", "node": "notify", "note": "机械提醒 → notify"},
    {"ev": {"kind": "signal", "jobId": "0x36563",
            "raw": '```json\n{"header":{"schemaVersion":1,"signalClass":"trade","signalId":"x1","sender":{"agentId":"Agent#999"}},"body":{"side":"buy","asset":{"chain":"bsc"}},"raw":""}\n```\ntrade call'},
     "want": "ask", "node": "buyer.sub.signal_received", "note": "信封缺必需字段(strict 语义)且非白名单 sender → ask(安全门)"},
]

def run(policy_dir, scope, scenario):
    fails = []
    for i, s in enumerate(scenario, 1):
        d, notes, _ = dl.process_event(s["ev"], policy_dir, scope, policy_dir)
        ok = d["mode"] == s.get("want") and (not s.get("node") or d["node"] == s["node"])
        print(("%s %2d kind=%-16s node=%-24s mode=%-7s (want %s)"
               % ("PASS" if ok else "FAIL", i, d["kind"], d["node"], d["mode"], s.get("want"))),
              "|", s.get("note", ""))
        print("        src:", d["decision_src"][:120])
        if not ok:
            fails.append(i)
    print("sub-sim:", "OK" if not fails else f"{len(fails)} FAIL (scenario {fails})")
    return 0 if not fails else 1

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--policy-dir", default=os.path.join(_SCRIPTS, "..", "examples", "policy"))
    ap.add_argument("--scope", default="sub-36563")
    ap.add_argument("--scenario", help="optional JSON file [{ev,want,node?,note?}]")
    a = ap.parse_args()
    scenario = DEFAULT_SCENARIO
    if a.scenario:
        scenario = json.load(open(a.scenario, encoding="utf-8"))
    sys.exit(run(a.policy_dir, a.scope, scenario))

if __name__ == "__main__":
    main()
