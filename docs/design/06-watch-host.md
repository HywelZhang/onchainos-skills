# P0-06 watch-host 设计（事件常驻监听 → policy 引擎）

> 状态: v0.1（2026-09-03）。OQ-3 结论: okx-a2a(@okxweb3/a2a-node) 为闭源 npm 包，内部不可改 → 采用"包装不改造": watch-host 调用 okx-a2a CLI 消费事件流。
> OQ-4 结论: 通知通道 = console（v1）。后续可加 telegram 适配器。

## 1. 为什么需要 watch-host（与官方用法的差异）

官方 watch 设计 = agent 会话内循环: 执行 `okx-a2a user watch --json` → 返回一批 → LLM 处理 → 重进。依赖一个活着的交互会话 + LLM 逐批解释（token 开销、会断）。

watch-host = 无头常驻: 后台自动循环同一命令，事件落本地队列，**按规则而不是按 LLM** 决定: 哪些直接进 policy 引擎(direct/script/ask)、哪些只是记录。效果与"改造守护进程直喂"等价——okx-a2a 的传输层(登录态/长轮询/队列/设备路由/版本协商)全部复用，我们只消费它的 stdout JSON。

## 2. 架构

```
okx-a2a user watch --json [--job-id X]   ← okx-a2a CLI(闭源, 不改造)
        │ stdout batch
        ▼
watch-host (scripts/watch-host.py)
  1. parse/normalize  →  {kind, jobId, terminal?, raw, decision?}
  2. 重进决策(规则, 非 LLM): 停止条件 vs 继续(超时/空/中间态标记)
  3. 事件分发:
     - decision_request / 需人决策 → notify(console) + 写 pending 队列
     - 订阅信号/任务事件 → 事件文件(JSONL) → 供 policy 引擎消费(未来)
     - 纯通知 → 落盘 + console 摘要
  4. 异常: 背退重试; 停止条件触发 → 退出(可被 supervisor 拉起)
        ▼
事件队列 <onchainos-home>/watch/events/*.jsonl   ← policy 引擎的输入(下一阶段)
console 通知(现) / telegram 等(未来适配器)
```

## 3. 循环规则（来自 watch-core 语义，编码为确定性逻辑）

- 每个批次处理后**无条件重进**（sticky `--job-id` 保持），除非触发停止条件。
- 停止条件（仅这些）:
  1. 后台恢复无法确认旧会话已退出 → 作废旧代次，不重启（由 supervisor/人工处理，见 watch-background-recovery）
  2. 用户显式 stop/unsubscribe
  3. scoped 会话(`--job-id`) 且批内任一 notification 的 userContent 含终态标记: `[Job Completed]` `[Job Auto-Completed]` `[x402 Job Completed]` `[Job Expired]` `[Job Closed]` `[Refund Settled]` `[Auto-Refund Settled]` → 处理完该批后停止(死 jobId 不会再发事件)
  - 全局会话不因单个任务终态停止。
- 非停止（必须继续）: 渲染完通知、全局会话出现终态标记、decision 已处理、空批次/长轮询超时、中间态标记(`[Deliverable Received]` `[x402 Deliverable Received]` `[Job Accepted]` `[Payment Mode Set]` `[Connecting ASP]` `[Job Created]` `[Replay Failed]` `[Rejection Confirmed]` `[Rating Submitted]` 等——不在停止清单的都不是停止)。
- 自动超时/唤醒: 收到 scheduler wake(`Pending decision_request auto-timeout reached. Re-enter watch now: okx-a2a user watch --json` 带可选 sticky job-id) → 走 stale-wake 时序守卫后重进（watch-wake-scheduling 语义）。
- 错误: 进程级错误背退(1s→5s→30s, 上限 5 次)后退出非零；网络/超时类不计数直接重进。
- 幂等: 事件按 (jobId, kind, raw-hash) 去重写盘，重放不重复。

## 4. 事件归一化 schema（JSONL 行）

```jsonc
{ "ts": 1769999999, "kind": "notification|decision_request|task_event|signal|raw",
  "jobId": "...", "sticky": true, "terminal": false,
  "markers": ["[Job Created]"], "raw": "…原文…", "source": "watch" }
```

消费方（policy 引擎/ask 通知）只读此 schema，与 okx-a2a 输出解耦。

## 5. 配置与接口

- 命令: `python scripts/watch-host.py [--once] [--job-id X] [--event-dir <dir>] [--cmd okx-a2a]`
- 环境: 需 okx-a2a 已装(`npm i -g @okxweb3/a2a-node`)且 doctor ready（登录态/设备）
- `--once`: 单批模式（调试/测试/supervisor 心跳模式）
- 输出: console 人类可读摘要 + 事件 JSONL
- 未来: `--notify telegram` 适配器; 事件直连 policy 引擎(本仓库下一阶段)

## 6. 验证状态

- [x] 骨架 + 解析/停止条件/去重逻辑离线自测(selftest fixture)
- [x] 真机端到端（2026-09-03, okx-a2a 0.2.10 + 邮箱登录态）: watch-host --once 收到 1 条真实通知（runtime-switch "Switched to Hermes"）并归一化落盘；事件 schema 经真实输出验证；空批次/长轮询路径与停止条件逻辑见 selftest
- [ ] 长跑验证: 连续监听 ≥24h 无漏事件（有活跃订阅/任务事件流后做）

## 7. 前置环境（2026-09-03 已就绪）

- npm i -g @okxweb3/a2a-node (0.2.10)；`okx-a2a doctor --fix` 完成: native launcher okx-a2a.exe(SEA)、provider=hermes、daemon running(pid 动态, ready)、agent refresh(2 agents)
- 已知限制: Hermes plugin 安装需 bash/WSL（Windows 不支持）——watch-host 不依赖该 plugin，无影响
- autostart 未装（可选，需管理员终端 `okx-a2a daemon autostart install`；现在需要时 `okx-a2a daemon start`）

## 8. supervisor 落地（2026-09-03, OQ-12=B: cron 心跳）

- 首选: 循环进程（前台/会话级）`python scripts/watch-host.py` —— 最简单，事件延迟 ≈ 0
- 持久化: `scripts/install-watch-task.ps1` 注册 Windows 计划任务（默认每 5 分钟跑一次 `--once`，日志 %LOCALAPPDATA%\okx-watch-host\watch.log）。schtasks 当前用户创建可能要求密码或管理员终端——失败时用管理员 PowerShell 重跑
- 心跳模式事件延迟 ≤ 间隔(默认 5 min)；对 decision_request 的及时性要求高时可缩短到 1 min
- 长跑验证目标: ≥24h 无漏事件（需活跃订阅/任务流）

## 9. 待办（下一阶段）

- supervisor 方案: Hermes 后台进程 / 系统服务 / cron 心跳(`--once` 每 N 秒)选型（OQ-3 后续）
- decision_request 的 ask 呈现: console 卡 + 回执通道（先 console 打印，人工在 CLI 会话回复）
- policy 引擎接入: 事件队列 → nodes/events 决策(03 设计)
