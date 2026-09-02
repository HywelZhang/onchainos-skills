# P0-01 角色 × 事件节点清单（Buyer / ASP）

> 状态: 草案 v0.2（2026-09-03）— 修订: 所有事件默认动作可自定义；按事件使用审计裁剪（附录 C）
> 依据: `cli/src/commands/agent_commerce/task/common/state_machine.rs` Event 枚举（55 变体）+ `task-state-machine.md`（11 状态）+ 各角色 playbook
> 用途: 节点是 per-node policy（03）与 hook（pre/post）的挂载点。节点粒度 = "一个事件/状态到达后需要做的一个决策或动作"。
> 阅读约定: 模式 = direct(确定性直触发) / llm(交大模型) / ask(人工确认) / hybrid(先 direct，边界/异常降级 llm|ask) / script:<file>(买家/ASP 自定义脚本)。安全级: L1 只读/ack / L2 链上写(有协议兜底) / L3 资金或不可逆 / L4 争议与证据。

## 0. 建模规则

1. 事件≠状态: 同一事件可能不改状态（pass-through），节点表按「事件到达时该角色要做什么」建模，不按状态图建模。
2. **默认动作只是默认——每个事件/节点的动作都允许自定义。** 配置层不存在"不可配置"的节点（mode 枚举对任何节点开放，含 script 自定义）；安全不靠禁止配置实现，靠执行桥闸门（grants/consent/幂等/journal，见 03 §2 limits）在运行时强制——auto/script 若闸门不齐备，执行被拒并降级 ask（fail-safe）。
3. 默认原则（只决定"没配置时"怎么走）:
   - 协议机械动作（ack/状态应答/收件/落盘/重进 watch）→ direct，不进 LLM；
   - 状态转移的 on-chain 写（accept/submit/complete/refund/claim）→ 默认 ask，可配 auto（必须过 grant/consent 闸，见 03）；
   - 内容生成（ASP 交付物、信号正文）→ llm（可换 script）；
   - 语义理解（用户自由文本意图、争议回复、信号分类）→ llm；
   - 附件/文件类 → direct（落盘）+ llm（按需理解内容）。
4. LLM 边界红线: LLM 只做分类/生成，不做资金参数决策。资金参数只允许来自: 结构化字段(strict) / 买家显式配置 / grant 上限内的人工确认。（红线是引擎级约束，不是节点配置约束——即便节点 mode=llm，执行桥仍拒绝"LLM 生成的资金参数"直接过闸。）
5. 基础设施节点（watch 重进、wakeup 恢复、设备路由、超时提醒的 re-enter）不进 policy，属于宿主守护层职责（另文），此处只列业务节点。
6. 事件裁剪（审计见附录 C）: 评审/staking 域 14 个事件 v1 不建节点（官方 CLI 兜底，我们只透传/记录）；机械提醒/纯通知类不单独建节点，走通用 notify 节点；其余事件建节点如下。

## 1. Buyer（用户侧，User Agent）

### 1.1 单次任务流（publish → negotiate → deliver → review → terminal）

| 节点ID | 触发 | 默认动作 | 默认mode | 说明/LLM seam |
|---|---|---|---|---|
| buyer.task.publish_intent | 用户自由文本"发任务/找人做X" | 收集需求→解析意图→建任务 | llm+ask | 意图/交付物描述抽取为结构化任务参数（目标/期限/预算/验收），确认后走 CLI |
| buyer.task.publish | 确认后 | `agent task create`(或 designated 路径) | ask | L3: 预算+escrow，默认卡片确认；可覆盖 auto/script(闸门: 预算上限+journal) |
| buyer.task.provider_reject | JobProviderReject | 提示→重新指定/转公开池 | llm | 语义选择: 重指谁/转公开 |
| buyer.task.negotiate | NegotiateReply / 对方消息 | 理解回复→回复/接受/还价 | llm | 纯语义节点 |
| buyer.task.accept_asp | JobAspSelected 后指定确认 / JobAccepted 前 | confirm-accept | ask | L3: 触发 escrow；可覆盖 auto/script(闸门: consent) |
| buyer.task.review_deliverable | JobSubmitted / DeliverableReceived | 下载→检查交付物→complete/reject/dispute | hybrid | 内容检查=llm；结果=ask(资金释放) |
| buyer.task.complete | 确认通过 | complete on-chain | ask | L3 资金释放；可覆盖 auto/script(闸门: consent+grant) |
| buyer.task.reject | 不合格 | reject on-chain | ask | L3: 进入 24h 窗口 |
| buyer.task.claim_refund | SubmitExpired / RejectExpired / JobAutoRefunded | claimAutoRefund | direct+notify | L3 但协议兜底明确；可覆盖 ask/script |
| buyer.task.close_expired | JobExpired | close + reclaim | direct+notify | L2 |
| buyer.task.dispute | 争议期 | 证据上传/回复 | llm+ask | L4: 证据内容 llm 起草，提交 ask |

### 1.2 订阅任务 · 信号执行流（重点: copy-trade 驱动，配合 02 信号 schema）

| 节点ID | 触发 | 默认动作 | 默认mode | 说明/LLM seam |
|---|---|---|---|---|
| buyer.sub.signal_received | 本期交付物到达（文本/文件） | 落盘 + 信封解析 + 校验 Active | direct | 持久化；校验订阅状态；过期/非 Active → 走异常节点 |
| buyer.sub.signal_parse | 信号载荷 | strict: 字段校验→typed signal；loose: 交 LLM 分类补全并标 inferred+confidence | strict→direct / loose→llm | **LLM 只补分类与语义字段，不生成资金参数**（缺 side/asset/amount 且无法确认 → 降级 notify，不执行） |
| buyer.sub.risk_grade | typed signal | 规则引擎: 白名单/黑名单/流动性/年龄/与 grants 上限比对 | direct | 规则表 buyer 可配（03） |
| buyer.sub.decide | risk_grade 结果 | 按订阅策略: auto / script / ask / llm 复评 | 按策略（默认 ask） | 模式=策略配置; llm 复评仅当策略允许且输出仍是建议 |
| buyer.sub.pre_execute | 决定=执行 | pre-hook → 执行桥 | direct | 桥负责幂等 latch + grant/consent 校验（复用官方 executor.rs 模式） |
| buyer.sub.execute | 桥通过 | venue 适配器下单/赎回/调仓 | direct | 只跑固定可执行文件，不跑 shell；一次一单；receipt 为准 |
| buyer.sub.post_execute | 桥返回 | post-hook → 记账/通知/审计 | direct | |
| buyer.sub.exec_exception | 余额不足/未登录/缺 consent/grant 超限/插件缺失 | 分派: deposit QR / 登录流 / 设置流 / ask / abort | hybrid | 每类异常有固定 UI 流（官方已定义 deposit-QR MANDATORY 等） |
| buyer.sub.signal_manual | 买家选择人工确认 | 渲染卡片 → 等确认 token | ask | 默认人工；可覆盖 auto/script(闸门: grant+consent 齐备) |
| buyer.sub.security_alert | 语义告警信号(自定义场景) | alertType 分类 → 买家规则/脚本（如紧急赎回） | script/llm | alertType 由 LLM/规则分类；金额/路径由买家脚本自决——买家对脚本负责 |

### 1.3 订阅 · 管理流（buyer）

| 节点ID | 触发 | 默认动作 | 默认mode |
|---|---|---|---|
| buyer.sub.subscribe_request | 用户想订阅某 ASP 服务 | 读取该 ASP service/signalProfile → 展示字段覆盖与建议模式 → 引导创建 | hybrid |
| buyer.sub.subscribe_create | 确认 | 创建订阅(含 consentSnapshot 采集) | ask（可覆盖 auto，闸门: 首次人工 once） |
| buyer.sub.renew_decision | SubExpireWarn / SubRenew | 展示续订 → 确认续/不续 | ask（可覆盖 auto: 须 grant+cap） |
| buyer.sub.trial_promote | SubTrialIntoActive | 展示转正式 → 确认 | ask |
| buyer.sub.reject_period | 本期交付不满意 | SubUserReject 前: 理由+要求退款/换期 | llm+ask |
| buyer.sub.refund_claim | SubRejectRefundNotify | claimAutoRefund | direct+notify |
| buyer.sub.cancel | SubCancel 前后 | 取消/确认取消 | ask |
| buyer.sub.dispute_open | 服务纠纷 | 证据/描述 | llm+ask |

### 1.4 机械提醒 / 纯通知（不单独建节点，通用处理）

SubmitDeadlineWarn / ReviewDeadlineWarn / SubExpireWarn / SubCompleteNotify / SubCloseNotify / SubFailedNotify 等 → 通用 notify 节点（direct）：渲染给用户+按需提醒，可覆盖为 script/llm。JobExpired / JobClosed / JobAutoRefunded 等终局事件并入对应流程节点处理。

## 2. ASP（Agent Service Provider）

| 节点ID | 触发 | 默认动作 | 默认mode | 说明 |
|---|---|---|---|---|
| asp.sub.selected | SubAspSelected | 校验自身能力/配额 → 接受/拒绝 | hybrid | 拒绝要给理由（协议语义） |
| asp.sub.on_request | 订阅创建通知 | pre-hook 出口: 触发自有脚本(容量检查/预置/通知) | direct(hook) | 目标3的 ASP 操作空间 |
| asp.task.apply | JobCreated(公开池匹配) | 展示任务 → 是否 apply | llm+ask | 语义: 评估是否接 |
| asp.task.accept_negotiate | JobAspSelected / JobAccepted | 协商/确认接单 | llm+ask | |
| asp.task.deliver | job_accepted 后 | 生成交付物 → 上传/链上提交 | llm(生成)+ask(提交) | 内容缝=ASP 自己的 LLM/脚本；signalProfile 声明输出模板 |
| asp.sub.signal_generate | 本期到期 | 生成信号正文/结构化载荷 → 交付 | llm/script | 若走 TRADE 模板，尽量填全字段（02）→ 让买家可开 strict |
| asp.task.rejected_respond | JobRejected | 选: 仲裁 / 同意退款 | llm+ask | 24h 窗口; 争议=证据 llm+ask |
| asp.sub.period_rejected | SubUserReject | 选: 同意退款 / 开争议 | llm+ask | |
| asp.task.dispute_phase | DisputeApproved/JobDisputed | 证据准备+提交 | llm+ask | L4 |
| asp.task.claim_complete | ReviewExpired | claimAutoComplete | direct+notify | 协议兜底 |
| asp.sub.renew_payout | SubRenew/SubCompleteNotify | 记账/通知（钱由协议结算） | direct | |
| asp.sub.dispute | SubAspDispute | 证据/协商 | llm+ask | |

ASP 全部节点同样可自定义（03 §2 nodes/hooks 对 role=asp 开放相同机制）。

## 3. 默认 mode 汇总原则（细节默认值表见 03 §3）

- direct 族: 收件/落盘/校验/ack/记账/通知/审计/协议兜底领取
- ask 族: 所有 L3 首次动作（除非显式 auto+grant）
- llm 族: 内容生成、自由文本语义、信号分类（loose）
- hybrid 族: 内容有歧义但动作可回退、异常分派
- 全部可覆盖（含 script 自定义），覆盖后的安全性由执行桥闸门保证

## 附录 A. Evaluator/评审事件（v1 暂缓 — 不建节点）

EvaluatorSelected / RevealStarted / VoteCommitted / VoteRevealed / RoundFailed / VoteCommitDeadlineWarn / VoteRevealDeadlineWarn / Staked / UnstakeRequested / UnstakeClaimed / UnstakeCancelled / StakeStopped / CooldownEntered / RewardClaimed
官方 CLI 完整兜底处理这些事件；v1 客户端只透传/记录。若产品不做仲裁托管，这 14 个事件不需要任何处理逻辑。

## 附录 B. 事件全名单（state_machine.rs, 55 变体, 按域）

主任务: JobCreated ProviderApplied JobProviderReject JobUserReject JobAspSelected JobAccepted JobSubmitted JobCompleted JobRejected DisputeApproved JobDisputed JobRefunded DisputeResolved JobExpired JobClosed JobPaymentModeChanged
超时/提醒: SubmitExpired RejectExpired ReviewExpired JobAutoRefunded SubmitDeadlineWarn ReviewDeadlineWarn
附件/协商/唤醒: AttachmentAdded UserAttachmentReceived DeliverableReceived NegotiateReply WakeupNotify
订阅: SubCreated SubAspSelected SubCancel SubUserReject SubAspAgree SubAspDispute SubTrialIntoActive SubRenew SubExpireWarn SubCompleteNotify SubCloseNotify SubFailedNotify SubRejectRefundNotify
评审/staking: （见附录 A）
Other: 后端未识别事件 + 用户指令伪事件（dispute_raise/agree_refund/close/decision 类）

## 附录 C. Event 使用审计（2026-09-03, 基线 v4.5.3 @ 17daea5）

方法: 55 个变体全仓库引用计数（cli/src 除 state_machine.rs 外的 wire/pascal 引用 + skills/workflows/README/AGENTS/CLAUDE 文档引用，程序扫描）。

结论:
1. 没有 0 引用的死事件——全部事件都有 CLI 分发/处理代码（含解析/派发/playbook 分支），与后端协议对账，兼容层不能删。
2. 但按"是否需要在我们的客户端建处理节点"分三类:
   - 核心流程事件（建节点，见 §1/§2）: 主任务流 + 订阅流 + 交付/协商/附件/唤醒 ≈ 34 个
   - 机械提醒/纯通知（通用 notify 节点）: SubmitDeadlineWarn / ReviewDeadlineWarn / SubExpireWarn / SubCompleteNotify / SubCloseNotify / SubFailedNotify / JobExpired / JobClosed / JobAutoRefunded（并入流程节点）等
   - 评审/staking 域（v1 不建节点，附录 A）: 14 个
3. 已发现的历史废弃: 结构化信号 schema（schema.rs "Retired"）——02 已按此设计；其余事件未见废弃标记。
4. 每次上游 sync 后建议重跑本审计（scripts/audit-events.py，P0 工具阶段建）。
