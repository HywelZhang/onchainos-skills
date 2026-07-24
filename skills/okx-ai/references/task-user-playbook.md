# User's User Session Playbook

> 🌐 **[Localization]** — all user-facing content must match the user's language. English users: template verbatim. Non-English: translate faithfully, preserving all field labels, data values, structure.

---

## Reading Order

1. **This file**: pre-flight, intent routing, communication boundary, decision relay — read once.
2. **[`task-user-actions-publish.md`](task-user-actions-publish.md)**: on demand — read when the user wants to publish a task.
3. **[`task-user-actions.md`](task-user-actions.md)**: on demand — read only the specific section needed (§2 attachment / §3 terms / §4 deliverables).
4. **[`task-cli-reference.md`](task-cli-reference.md)**: do NOT read full file. Use `grep` for the specific command you need.

⚡ Re-reading a file already in context costs 1 LLM round + thousands of tokens for zero new information.

---

## User Intent Routing

> When the user-session receives free-form text targeting a specific task and no pending decision matches, load [`task-user-intent-routing.md`](task-user-intent-routing.md) and follow its routing flow.

| Intent | Trigger examples | Route to |
|---|---|---|
| Publish task | "发布任务 / create a task / 帮我发个任务" | [`task-user-actions-publish.md`](task-user-actions-publish.md) |
| Add attachment / image | "补充附件 / attach file to task" | [`task-user-actions.md`](task-user-actions.md) §2 |
| Switch provider / stop task | "换服务商 / switch provider / 关闭任务 / stop task" | [`task-user-actions.md`](task-user-actions.md) §3 |
| View deliverables | "查看交付物 / view deliverables" | [`task-user-actions.md`](task-user-actions.md) §4 |
| Designated-provider A2A | "指定服务商 / use the service of Agent X / 购买Agent/ASP的服务 / buy service from Agent/ASP #XXXX / initiate a direct conversation with this provider" | [`task-user-actions-publish.md`](task-user-actions-publish.md) §5 |
| Designated-provider x402 | "send a request to this endpoint" | [`task-user-actions-publish.md`](task-user-actions-publish.md) §6 |
| Negotiate with provider | "negotiate with XXX" | Sub session handles automatically |
| Re-submit / nudge | "重新提交 / 催一下" | [`task-user-intent-routing.md`](task-user-intent-routing.md) |
| Task list / status / close / decision list | "我的任务 / 查看决策 / close task" | [`task-user-intent-routing.md`](task-user-intent-routing.md) |

---

## My Subscriptions (订阅列表 — buyer view)

Trigger: user asks for their subscriptions (`我的订阅` / `订阅列表` / `我订阅了哪些服务` / `my subscriptions` / `what am I subscribed to`). Routing entry lives in [`task-user-intent-routing.md`](task-user-intent-routing.md).

Command: `onchainos agent my-subscriptions --role buyer` → JSON `{ "list": [ … ] }`. Render each element as one row (localize labels for non-CN users):

| # | 服务 | ASP | 状态 | 试用 | 当前周期 | 下次扣款 | 自动续费 |
|---|------|-----|------|------|---------|---------|---------|
| 1 | {title} | Agent#{providerAgentId} | {状态文案} | {trialType==1?"试用中":"—"} | {subStartTime}~{subEndTime}（按日期渲染） | {下次扣款} | {autoRenew==1?"✓":"✗"} |

- **状态文案**: map `statusName`(+`trialType`) per the status map (试用中/生效中/已拒单…). INIT→待激活；UNKNOWN_<n>→原样。
- **下次扣款** (no CLI field — derive): `trialType==1` → `subStartTime`(试用转正扣款日); else `autoRenew==1` → `subEndTime`; `autoRenew==0` → "不续费". Render epoch-seconds as a date.
- All timestamps are **epoch seconds** — render as the user's locale date, never raw numbers.
- Empty list → "你还没有任何订阅。" Do NOT invent rows.
- To open one row's full detail, pass that row's **`jobId`** to `subscribe-detail` (§订阅详情).

## Subscription Detail (订阅详情)

Trigger: user selects a row / asks about one subscription (`订阅详情` / `这个订阅的情况` / `subscription detail`). Command: `onchainos agent subscribe-detail <jobId>` — the positional id is the **`jobId`** from the list (the response primary key; there is no separate `subId`). → single `SubscriptionInfo`. Render:

> **{title}** — {状态文案}
>
> 订阅方：Agent#{buyerAgentId}
> 服务方：Agent#{providerAgentId}
>
> {trialType==1 ? "试用期：{trailStartTime} ~ {trailEndTime}" : ""}（按日期渲染）
> 当前周期：{subStartTime} ~ {subEndTime}（第 {periodIndex} 期）
> 缓冲截止：{subBufferEndTime}
>
> 费用：{serviceTokenAmount}（token {serviceTokenAddress 前 6 位}…）/ 周期
> 自动续费：{autoRenew==1 ? "已开启" : "未开启"}
> 自动跟单：{copyTrade==1 ? "已开启" : "未开启"}

- 金额字段（`serviceTokenAmount` / `paymentTokenAmount` / `paymentCurrencyAmount`）是**字符串**，原样展示，绝不转 float。
- token 符号 CLI 不提供，仅有 `serviceTokenAddress`（展示短地址）。
