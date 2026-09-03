# P0-09 ASP 侧执行器设计（确定性 ASP 驱动）

> 状态: 设计稿 v0.1（2026-09-03）。与 buyer 侧 executor-lite 对称。实现前需先做 provider 侧事件语义真机采样（okx-a2a task / next-action --role asp 的输出形状）。
> 目标: ASP 收到订阅需求/任务后可触发额外脚本（目标3）并确定性走完 接受→执行→交付→(争议) 机械节点, LLM 只留"交付物内容生成"缝。

## 1. 节点 → 命令映射（基于已实测/已见 CLI 面, 待真机确认项标 ⚠）

| ASP 节点 (01 §2) | 命令/机制 | 状态 |
|---|---|---|
| asp.task.apply (公开池匹配) | `onchainos agent next-action --role asp` 派生; provider_applied 事件 | ⚠ 采样 |
| asp.sub.selected / asp.task.accept | next-action --role asp (job_asp_selected → 协商/接受); confirm-accept 由 buyer 发起 | ⚠ 采样 |
| asp.task.deliver | `onchainos agent deliver`(job_accepted 门) + `--file-key`(附件) | buyer 侧已验证同族命令; deliver 签名待 --help |
| asp.task.rejected_respond | JobRejected → 仲裁/退款: 仲裁=`task dispute approveAndCreate…`(命令已见: agent subscribe-dispute 同族); 同意退款=subscribe-agree-refund 同族 | 待 --help |
| asp.task.claim_complete | ReviewExpired → `agent claim-auto-complete`(已见) | 签名待确认 |
| asp.sub.period_rejected | SubUserReject → agree-refund / dispute(命令已见) | 签名待确认 |
| 内容缝(信号/交付物生成) | signal-envelope.build_envelope + 自有数据/LLM → 交付 | ✅ 工具已就绪 |
| hook 出口(订阅创建等) | decision-loop run_hook 机制复用(白名单脚本+veto) | ✅ 工具已就绪 |

## 2. 状态机要点

- ASP 无"主动接单"（task-asp-accept.md: no proactive-accept path）——事件驱动: job_created(公开匹配)/job_asp_selected(指定) → next-action 派生 playbook。
- deliver 被 job_accepted 门约束: 未接受不能交付。
- 拒收响应有窗口(24h/订阅期) → 超时自动兜底(claim-auto-complete / 自动退款) 已是 direct 节点。
- 争议/退款 = L4 保留人工(llm+ask), 不进 auto 默认(01 §2 默认表)。

## 3. 实现路径（真机采样后）

1. 采样: 用 ASP 身份(#1792 需先激活) 跑 `agent next-action --role asp` + 一条真实 job_created/job_asp_selected 事件, 记录 playbook 形状与 deliver/争议命令签名。
2. 写 scripts/executor-asp.py（对称 executor-lite: watch/next-action → accept/negotiate 策略 → 内容缝调用 → deliver → 拒收响应策略; --dryrun/--live）。
3. ASP 订阅信号场景: 每期 signal-generate = 内容缝(LLM/脚本) → signal-envelope.build → deliver; 买家可 strict 解析(02) → 全自动闭环成型。
4. 接入 decision-loop hooks(asp.* 节点已有 03 默认表)。

## 4. 前置条件

- ASP agent #1792 当前 unavailable → 需激活(submit-approval 流程, 官方审核)才能真机采样。
- 或新建测试 ASP 身份（链上注册免费, OKX 付 gas）——作为待确认项(OQ-13: 是否激活 #1792 或新建 ASP 测试身份)。
