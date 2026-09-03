# P0-10 买家订阅流 · 一阶段手册（能用起来）

> 状态: v0.1（2026-09-03）。第一阶段目标 = 让买家订阅"能跑通、可配置、可验证"。真实订阅流上链验证待 OQ-14 确认。
> 配套: examples/policy/sub-36563.json（真 ASP 模板）、scripts/sub-sim.py（离线场景模拟 7/7 PASS）、decision-loop contentTags 打标、watch-host。

## 1. 买家订阅日常动作（官方 CLI 包装）

| 动作 | 命令 | 说明 |
|---|---|---|
| 找可订阅服务 | `agent service-match --keywords "跟单 信号"` | 过滤 subscription 非空 + freeTrial |
| 建订阅 | `agent create-subscribe --service-id <id> --service-token-amount N --service-token-address <usdt> --auto-renew false --use-trial true --title ... --description ...` | 试用先行, auto-renew 先关 |
| 看订阅 | `agent my-subscriptions --role buyer` | 当前 0 条 |
| 详情 | `agent subscribe-detail <subId>` | 状态/周期/价格 |
| 月成本 | `agent subscribe-cost` | 汇总 |
| 拒收本期 | `agent subscribe-reject --reason ... <subId>` | 质量差时 |
| 取消 | `agent subscribe-cancel <subId>` | 关闭续费/退订 |
| 接收设备 | `agent subscribe-device-update --job-id <subId> --device-list <id>` | 默认本机 deviceId 即可 |

资金/授权动作（需用户显式确认, 与 policy 的 ask 对应）:
- `agent autotrade-consent-set --job-id <subJobId> --mode auto|manual|decline [--cap N]` — 官方"自动跟单授权"(auto 需 Trade Kit + 资金上限)
- 我们 policy 的 order→ask 默认不改动它: 买家在 agent 会话里看到 order 信号卡 → 回 A(跟)/B(不跟) 即可, 无需开全局 auto。

## 2. 信号分流设计（本阶段核心, 已实现）

ASP 8136 真实信号两类:
- analysis(每~3min 全市场扫描) → 确定性打标 signal_analysis → nodes.buyer.sub.signal_analysis = **notify**（console, 不打扰、不占 ask 队列）
- order(真实下单/调仓, 低频) → 打标 signal_order → nodes.buyer.sub.signal_order = **ask**（含 action/sz/杠杆/价; 买家回 A 跟单或 B 忽略）
- 无 signal_type/无信封的未知 signal → nodes.buyer.sub.signal_received = **ask**（安全兜底）
- 其他通知(sub_renew/机械提醒) → notify; decision_request → ask

机制: watch-host 归一化事件 → decision-loop process_event → policy(sub-<sid> 链) → contentTags 子串匹配(确定性, 0 LLM) → decide → hook/console。策略全部可自定义(改 YAML/JSON 即可, agent 可代写)。

## 3. 验证状态

- [x] 离线: sub-sim 7 场景 7/7（analysis 静默/order ask/未知 ask/信封门/决策 ask/通知 notify）
- [x] 组件: policy-engine(新 kind 支持+contentTags 校验)、decision-loop 打标、watch-host 已实装
- [ ] 真实流: 订阅 36563 后 watch 捕获真实 analysis/order → 确认分流正确性（OQ-14 待批: 72h 免费试用, auto-renew 关闭, 试用结束自动停, 0 额外成本）

## 4. 风险与边界（如实）

- contentTags 是子串匹配: ASP 改文案格式(如 signal_type= ORDER)会失配 → 落入 ask 兜底(安全侧), 需校准(OQ-10 已有计划)
- order 信号跟单若开 auto 走官方 autotrade(闭源 Trade Kit), 我们只做策略裁决不碰资金执行; 自动执行红线不变
- 试用订阅是链上真实状态变更(需要你确认后我才执行 create-subscribe)
- 评审/quality: 本阶段分析信号只 notify 不评判质量, 拒收靠 subscribe-reject(买家动作)
