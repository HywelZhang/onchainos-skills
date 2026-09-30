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
- [x] **真机首链验证(2026-09-03)**: 试用订阅 ASP 3895 Janus Cross-Market Basis Monitor(5 USDT/月, 72h, jobId 0x44673735…, ACTIVE) → 首份自动监测投递(BTC/ETH 资金费率表) → watch 捕获归一化 → decision-loop sub-40209 裁决 → notify。真实投递原为 task_event, 已加 sub 作用域 [Received]→signal 归一化, 现走 nodes.buyer.sub.signal_received 显式路径 ✓
- [ ] ❌ **试用到期保护失败(2026-09-06 实测, 2026-09-30 复盘)**: 两条一次性 cancel cron(007facb80e7f Janus 18:05 / ebd39aea092d meme 18:20 CST)均未触发——调度器/host 越过 120s 宽限, 09-07 15:52 被判定"不会触发"并移除(证据: hermes cron/output/<jobid>/2026-09-07_15-52-17.md)。两笔试用已自动转付费扣款: **5 USDT**(Janus 3895, tx 0x60c0dc98a501…fab3, 09-06 18:08 CST) + **1 USDT**(Meme Signal Auto Trader 10724, tx 0x8d98fcbc38e5…f8f6, 09-06 18:17 CST), 合计 6 USDT 真实成本。当前两订阅均 ACTIVE, 付费期 2026-09-06 → 10-06 19:07/19:16 CST, autoRenew=0(不再续费)。教训: 一次性本地 cron 不构成时限保护(需补跑/常驻/后端侧机制, 见 OQ-19)。
- [x] 配置回环闭环(2026-09-03): 会话直投 → ASP 配置问询(4 问) → 回复默认范围(OKX BTC/ETH-SWAP; 阈值=年化费率 Δ≥2pp / 溢价 Δ≥0.05pp / 翻号) → 基线快照回显同一阈值, 已采纳 ✓
- [ ] 周期级事件校准: 拒收(subscribe-reject)/续费决策等 sub 域事件待周期出现后补验（QO-10 记录）

## 4. 风险与边界（如实）

- contentTags 是子串匹配: ASP 改文案格式(如 signal_type= ORDER)会失配 → 落入 ask 兜底(安全侧), 需校准(OQ-10 已有计划)
- **ASP 数值口径实测不可靠(2026-09-03; 2026-09-30 复核实证漏报)**: Janus 正常模板年化自洽(0.006430%/8h → 7.04%/yr ✓; 0.0100%/8h → 10.95%/yr ✓), 但其"高亮"分支换算差 100×: 基线 0.0100%/8h 报 0.0375%/yr(应 10.95%), 09-03 11:15 与 09-04 13:32 的 Δ 快照均报 0.0215 / -0.0227%/yr(实为 2.15 / -2.27%/yr)。**关键**: 09-04T13:32Z 快照 ETH 年化 Δ = **-2.27pp/年**, 已越过订阅阈值(年化 Δ≥2pp)本应告警, 却被错误口径判为"无告警"——该 bug 不是展示问题, 是**漏报**(missed alert)。→ 买家端必须独立复算数值并自建告警判定, 不能依赖 ASP 计算(fork 产品设计输入; 与 §3 试用保护失败同属"买方端不能托管信任")。**漏报清单(ASP 告警分支,全部伴随 100× 换算错误)**: 09-04T13:32Z ETH -2.27pp; 09-04T19:16Z BTC -4.95pp + 翻号; 09-05T03:16Z ETH +3.56pp + BTC 翻号; 09-05T11:17Z BTC -2.08pp + ETH -8.55pp(且翻号); 09-05T19:17Z BTC +4.00pp + 翻号; 09-06T03:17Z ETH +4.41pp —— **6/6 份告警快照全部漏报**。完整模板快照另计(不自带告警声明): 09-04T23:17Z BTC -2.52pp+翻号、09-05T05:19Z ETH +5.60pp、09-05T11:30Z BTC -2.27pp + ETH -12.27pp+翻号、09-05T17:33Z BTC +2.03pp、09-05T23:35Z BTC +2.71pp 与 ETH +4.48pp(双翻号)、09-06T05:35Z BTC +4.37pp 与 ETH +6.03pp 亦越阈。
- **试用保护机制实测失效 + 信号陈旧度(2026-09-30 复盘)**: 见 §3——一次性本地 cron 未触发, 造成 6 USDT 实际扣款; 且第二笔 cron(18:20)本身就晚于扣款时刻(18:17), 时限保护须按"截止前 ≥30min"设计并支持补跑。另: 2026-09-03T17:10Z 的快照直到 09-30 才补投(滞后 27 天), 客户端必须按 content 时间戳打"陈旧"标记, 否则买家会按失效行情决策。
- order 信号跟单若开 auto 走官方 autotrade(闭源 Trade Kit), 我们只做策略裁决不碰资金执行; 自动执行红线不变
- 试用订阅是链上真实状态变更(需要你确认后我才执行 create-subscribe)
- 评审/quality: 本阶段分析信号只 notify 不评判质量, 拒收靠 subscribe-reject(买家动作)
