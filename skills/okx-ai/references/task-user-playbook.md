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
| Subscription task ops | "subscribe task / subscription task / auto-renew / trial cancel / reject delivery / 申请退款 / 退款 / refund / claim refund / my subscription tasks / 订阅扣费 / 订阅花了多少 / subscription cost" | §Subscription below |
| Negotiate with provider | "negotiate with XXX" | Sub session handles automatically |
| Re-submit / nudge | "重新提交 / 催一下" | [`task-user-intent-routing.md`](task-user-intent-routing.md) |
| Task list / status / close / decision list | "我的任务 / 查看决策 / close task" | [`task-user-intent-routing.md`](task-user-intent-routing.md) |

---

## Subscription

### Subscription branching (integrated into create_task playbook)

The `create_task` playbook (returned by `next-action --message '{"event":"create_task"}'`) handles both subscription and regular tasks in a single unified flow. It collects Description (and optionally Provider) first, then runs `asp-match` to determine service type, and branches:

```
Step 1: Description, Provider (optional)
  → Step 3: asp-match (auto-discover if no provider)
    → [supportSubscription == true?]
      → YES (subscription): Currency/Budget auto from service, auto-set useTrial, ask autoRenew → subscription confirmation form → create-subscribe
      → NO  (regular): collect Currency, Budget, Max budget → regular confirmation form → create-task
```

If a single ASP returns both subscription and non-subscription services, display each with `[Subscription]` / `[One-time]` label and let the user choose. The chosen service determines the branch.

### Subscription-specific field rules

| Field | Source | Notes |
|---|---|---|
| `serviceId` | from `asp-match` response | auto-filled |
| `useTrial` | `supportTrial == true` (or `supportTrail == true` — legacy typo, check both) from `asp-match` → auto `true`; otherwise `false`. Display hours from `freeTrial` field | **auto-filled, do NOT ask user** |
| `autoRenew` | ask user explicitly before form — no default | 0=off, 1=on |
| `copyTrade` | parse `serviceDescription` for actionable trading signal indicators (buy/sell direction, entry price, TP/SL, position size); if eligible → **ask user explicitly** "Enable auto copy-trade? (yes/no)", yes → 1, no → 0; if not eligible → 0 (skip the question) | **must ask user when eligible** |
| `serviceTokenAmount` | from `asp-match` response `feeAmount` | must match listing price |

The `create-subscribe` CLI command handles the full flow internally: providerConfirmStatus → EIP-712 terms signing → create API → sign uopData → broadcast(bizType=101). Wait for `sub_created` event to confirm success.

See `task-user-actions-publish.md` **Appendix A2** for the subscription confirmation form template.

### Post-creation: Offline-deliverables question

AFTER a `create-subscribe` succeeds — in **both** the normal branch and the degraded branch (`deviceRoutingDegraded: true`) — render this question block so the user can decide what happens to deliverables produced while they are offline. Chinese-language sessions render it **VERBATIM**; other languages translate faithfully, preserving meaning, per the §Localization banner. `{任务名}` is the **just-created REAL subscription title** — never a hard-coded sample.

> 「{任务名}」订阅任务已创建成功 ✅
> 您离线期间，这个任务会持续产生交付物。重新上线后，这批交付物怎么处理？
> · 补推给我（默认）—— 上线后补上，后台照常接收并处理
> · 清理掉 —— 离线消息直接丢弃，后台不再接收，避免白白消耗算力
> 💡 用 Codex / Claude Code 的话：选「补推」时，消息也是先到后台，要在对话里看到还需说一句「监听 {任务名}」。

**Old comm-package branch** — read the `create-subscribe` success envelope's `offlineReplaySupported` (the CLI already probed it; **never run `okx-a2a capabilities` yourself**). When it is `false`, append this VERBATIM line to the END of the question block above (the four-segment block + 💡 line itself stays byte-identical). Chinese sessions render it verbatim; other languages translate faithfully, preserving meaning, per the §Localization banner:

> 💡 当前通信包版本暂不支持离线回放偏好。您现在的选择会保存，待通信包升级后生效（升级命令：{fixCommands}）；升级前，所有订阅消息仍会正常补推。

`{fixCommands}` is rendered from the envelope's `offlineReplayFixCommands`, one command per line. When `offlineReplaySupported` is `true` (or the field is absent), add nothing — the question block stays exactly as above.

Branching on the user's reply:
- **No choice made, OR explicit 补推 / keep** → do **NOT** write anything (the server default is already `0` = 补推). Take no action.
- **清理 / discard** → run `onchainos agent subscribe-offline-update --job-id <this subscription's jobId> --flag 1`. Then confirm based on that command's own success envelope `offlineReplaySupported`:
  - `true` (or the field is absent) → 「好的，离线期间的消息会直接清理，不再补推。」
  - `false` → 「好的，偏好已保存：通信包升级后，离线期间的消息会直接清理、不再补推；升级前仍会正常补推。」
- **Write failure** → do **NOT** roll back or retry the create (the subscription is already created and unaffected). Tell the user the offline-cleanup setting was not saved and stays at the 补推 default, and that they can change it later. Non-blocking — surface as a plain notice, not an error.

### Post-creation: Watch check (mandatory)

After `create-subscribe` succeeds, check the CLI output for a `[Watch]` block:
- `[Watch]` block present → read `skills/okx-ai/references/watch-core.md`, execute watch, then **end this turn**.
- No `[Watch]` block → **end this turn immediately**.

🛑 This is the **last action before ending the turn** — no other commands after it. DApp plugin pre-install is handled later when the `sub_created` event arrives.

### Subscription management (user-initiated)

| Intent | Command | Notes |
|---|---|---|
| Subscription detail | `subscribe-detail {subId}` | show subscription detail |
| Enable auto-renew | `start-autorenew {subId}` | on-chain, needs EIP-712 sign; may require approve |
| Cancel subscription (trial cancel / close auto-renew) | `subscribe-cancel {subId}` | unified: trial → cancel auto-conversion, no charge incurred, Closed; active → close auto-renew, current period continues to expiry |
| Apply for refund (退款 / 发起退款 / 申请退款 / 拒收 / 申请仲裁 / 申请评审 / 仲裁 / 评审 / refund / dispute / evaluation / arbitration) | `reject {id} --reason "..."` | **unified command** — auto-detects subscription vs regular task. User says any of these keywords → **always use `reject`** as the first step |
| Claim refund after timeout | `claim-auto-refund {id}` | 🛑 **NEVER use as first step** — only after `reject` AND ASP misses 1-day response window |
| Active subscription cost | `subscribe-cost` | total monthly cost of active formal subscriptions (no params needed) |
| 让本机开始接收某订阅消息 (start receiving on this device) | `subscribe-device-update --job-id <id> --device-list <fresh list + this device>` | **fresh-read first** (`subscribe-detail`/`my-subscriptions`); if this device is already present, tell the user & do NOT re-write; after write, re-read and mark ✅是（本次新增） |
| 让某台/某几台指名设备开始接收某订阅 (start receiving on named device(s)) | `subscribe-device-update --job-id <id> --device-list <fresh list ∪ named device ids>` | **fresh-read first** (`subscribe-detail`/`my-subscriptions`); resolve device name→id via `device-list` — a name that cannot be resolved must **not** be fabricated (surface the raw id / count and ask the user to clarify); build the new `--device-list` as the **UNION** of the just-read list and the named ids; overwrite; re-read; confirm with this VERBATIM copy frame: 「好的，「Y」现在会同时推送到 X1 和 X2。」 where the device-name list enumerates the **complete post-write receiving set from the re-read** (readable names, not just the newly added devices; two devices joined by 和, three or more separated by 、 with 和 before the last) |
| 停止向某设备推送某订阅 (stop pushing to a device) | `subscribe-device-update --job-id <id> --device-list <fresh list − device>` | resolve device name→id via `device-list`; after write, read back remaining receivers; copy: 「已停止向 X 推送「Y」。现在这个任务只会推到 Z。」（名称不可得时降级为数量，绝不编造名称） |
| 改离线交付物处理方式 (change offline-deliverables handling later — 「离线消息别清了」/「改成补推」/「改成清理」/「离线消息帮我清理」) | `subscribe-offline-update --job-id <id> --flag <0\|1>` (0=补推, 1=清理) | **fresh-read first** (`subscribe-detail` → current `offlineReceiveFlag`); if it already equals the target value, tell the user no change is needed and do **NOT** re-write; otherwise write the target flag, then re-read `subscribe-detail` to confirm the new 离线交付物 value. On a successful **`--flag 1`** write, branch the confirmation on the write envelope's `offlineReplaySupported` (read from the envelope; never run `okx-a2a capabilities`): `true`/absent → 「好的，离线期间的消息会直接清理，不再补推。」；`false` → 「好的，偏好已保存：通信包升级后，离线期间的消息会直接清理、不再补推；升级前仍会正常补推。」 The **`--flag 0`** direction keeps its current copy and behavior unchanged. |
| 列出登录设备 (list devices) | `device-list` | render §Device List; ms→local time is already CLI-derived (`lastOnlineLocal`) |
| 监听任务/消息（未指定任务）(listen, no task specified) | — | confirm exactly one task（「一次只能监听一个」）→ turn on this-device receipt → enter the existing watch flow (`watch-core.md`) → tell the user new messages push live into this conversation |

If the user does not specify a `subId`, use `subscribe-detail` to check the subscription, or ask the user to provide it.

**Device-routing safety flows (must be encoded as copy/behavior):**
- **Clear-list confirmation:** if a removal would empty the device list, first explicitly warn 「该订阅将没有任何设备接收消息」 and obtain confirmation, only then write.
- **Overwrite from fresh read:** the new `--device-list` is ALWAYS built from the just-re-read list (`subscribe-detail` / `my-subscriptions`), never from conversational memory — `subscribe-device-update` overwrites wholesale.
- **Neutral copy:** promise only 「本订阅任务的消息」; make no promise about system-notification scope.

### Reject + refund flow (detailed)

> **Intent mapping**: "退款" / "发起退款" / "申请退款" / "拒收" / "申请仲裁" / "申请评审" / "仲裁" / "评审" / "refund" / "dispute" / "evaluation" / "arbitration" / "apply for refund" → `reject` (Step 1 below).
> The `reject` command is unified — it auto-detects subscription vs regular task by `jobType`.
> 🛑 `claim-auto-refund` is NOT the entry point — NEVER call it directly for any refund/退款 intent. It is only used in Step 3 after ASP timeout.
<!-- intent: 申请仲裁 / 仲裁 / arbitration are kept here as input aliases for recognition only — do not delete them or reduce their occurrences. When any action word in this list matches, route straight to reject (the refund / refusal flow) and return NO legacy-role rename prompt; that is a deliberate decision, not an omission — these are task actions, not the Evaluator role. -->

When the user is unhappy with a delivery (subscription or regular task):

```
Step 1 — Reject (on-chain, user initiates)
  onchainos agent reject {id} --reason "quality not met"
  → auto-detects: subscription → /subscribe/{id}/reject; regular → pre-reject/reject dual-sign
  → status = Rejected
  → ASP has 1 day to respond

Step 2 — ASP responds (one of three outcomes)
  A. ASP agrees to refund → sub_asp_agree event → status = Failed (funds returned)
  B. ASP files dispute   → sub_asp_dispute event → status = Disputed (awaiting DM evaluation)
  C. ASP does not respond within 1 day
     → user may claim refund manually:

Step 3 — Claim refund (only after ASP timeout)
  onchainos agent claim-auto-refund {subId}
  → status = Failed (funds returned)
```

Key rules:
- `reject` requires `--reason` (max 2000 chars); for subscriptions, one rejection allowed per subscription.
- `claim-auto-refund` is only valid when status = Rejected AND the ASP response window has passed.
- If the ASP files a dispute, the user must wait for the Dispute Manager's ruling (follows the existing on-chain dispute resolution flow).

## My Subscriptions (订阅列表 — buyer view)

Trigger: user asks for their subscriptions (`我的订阅` / `订阅列表` / `我订阅了哪些服务` / `my subscriptions` / `what am I subscribed to`). Routing entry lives in [`task-user-intent-routing.md`](task-user-intent-routing.md).

Command: `onchainos agent my-subscriptions --role buyer` → JSON `{ "list": [ … ], "thisDeviceId": <String|null> }`. Render each element as one row (localize labels for non-CN users). **Render ALL columns below — never drop 服务商 or 期数, and never merge 下次扣款 into a raw period range; 下次扣款 is a single derived date per the rule below.**

| # | 服务 | 服务商 | 状态 | 费用 | 下次扣款 | 自动续费 | 订阅期数 | 已登陆设备 | 设备是否接收任务消息 |
|---|------|--------|------|------|---------|---------|------|------|------|
| 1 | {title} | Agent#{providerAgentId} | {statusName} | {serviceTokenAmount} | {下次扣款} | {autoRenew==1?"✓":"✗"} | {期数} | {deviceName}{（本设备）if this device} | {✅是/否} |

- **状态**: 直接展示 CLI 返回的 `statusName`（ACTIVE / REJECTED / DISPUTED / COMPLETED / CLOSED / FAILED / INIT / UNKNOWN_<n>），原样输出、不翻译成中文。试用 vs 正式改由「期数」列区分（`trialType==1` 显示"试用期"）。
- **费用**: `serviceTokenAmount` 字符串原样展示（绝不转 float）；CLI 不提供 token 符号，仅 `serviceTokenAddress`。
- **期数** (按状态分派): `trialType==1` → "试用期"; else `periodIndex` 为合法正整数(> 0) → `第{periodIndex}期`; else (`periodIndex` 为 null 或 ≤ 0) → "—"。
- **下次扣款** (no CLI field — derive): `statusName != "ACTIVE"` → "—"; else `trialType==1` → 读 `trialEndTime`(正拼, 优先) 或 `trailEndTime`(`trail*` 旧拼, fallback) 双读(复用 AC-17)，渲染为日期(试用转正扣款日)，两者皆缺 → "日期暂缺"; else `autoRenew==1` → `subEndTime`; `autoRenew==0` → "不续费". Render epoch-seconds as a date.
- **已登陆设备 / 设备是否接收任务消息** (per-device expansion): a subscription logged in on N devices occupies **N rows** — the `#` and all leading subscription columns **repeat unchanged** across that subscription's rows. 已登陆设备 = that device's readable `deviceName` (join each id in the subscription's `deviceList` from `my-subscriptions` against the `device-list` rows to get names; the **this-device** row gets a prominent marker `（本设备）`). 设备是否接收任务消息 = **✅是** when the device id ∈ this subscription's `deviceList`, else **否**; the this-device row's value comes directly from the CLI `thisDeviceReceives` flag — never recompute it. When a device name is unavailable, **degrade to a count / raw id — never fabricate a name**.
- **Degraded render (MANDATORY — device table unavailable):** when `device-list` fails or is empty, fall back to **one row per subscription** and **explicitly state that other devices' receipt status is temporarily unavailable** (e.g. 「其他设备的接收状态暂不可用」). It is forbidden to present the one known (this) device as the full picture. The this-device row still shows ✅是/否 from `thisDeviceReceives`; all other devices are shown as unavailable, not omitted silently.
- **Display-only rule:** on any list render, do **not** proactively ask whether to turn on receipt (product retracted that prompt); turning on happens only on explicit user request.
- All timestamps are **epoch seconds** — render as the user's locale date, never raw numbers.
- Empty list → "你还没有任何订阅。" Do NOT invent rows.
- To open one row's full detail, pass that row's **`jobId`** to `subscribe-detail` (§订阅详情).

## Post-login subscription display (login-flow-triggered)

**Trigger (entry layer):** the wallet login flow itself, NOT a user utterance. The single entry is the routing line in [`wallet.md`](../../okx-agentic-wallet/references/wallet.md) → Authentication step 3 ("After login"). Do **NOT** add any trigger words to `SKILL.md` for this display — the login flow is the only entry. Command: `onchainos agent my-subscriptions --role buyer`.

**Zero-disturb (mandatory).** If the command errors (no OKX.AI identity, transport/auth failure) OR the subscription list is empty, output **nothing** OKX.AI-related — no table, no opening line, no 💡 hint, no error, no mention that a check ran. The login flow concludes normally. Never surface the attempt.

**Non-empty render.** Reuse §My Subscriptions **as-is**: same per-device expansion (a subscription on N devices occupies N rows; the `#` and all leading subscription columns repeat unchanged), same `deviceList` × `device-list` name join, same pagination-to-completion, same `thisDeviceReceives` / `thisDeviceId` / `（本设备）` handling, and the same **mandatory degraded render** when `device-list` fails/empty (fall back to one row per subscription and explicitly state 「其他设备的接收状态暂不可用」 — never present this device as the full picture). Only the two deltas below differ.

- **Delta (a) — column header:** the device-name column header is **「已登陆设备名称」** (the second device column keeps **「设备是否接收任务消息」**, identical to §My Subscriptions / §Subscription Detail). All other columns and their derivation rules are exactly those of §My Subscriptions:

| # | 服务 | 服务商 | 状态 | 费用 | 下次扣款 | 自动续费 | 订阅期数 | 已登陆设备名称 | 设备是否接收任务消息 |
|---|------|--------|------|------|---------|---------|------|------|------|
| 1 | {title} | Agent#{providerAgentId} | {statusName} | {serviceTokenAmount} | {下次扣款} | {autoRenew==1?"✓":"✗"} | {期数} | {deviceName}{（本设备）if this device} | {✅是/否} |

- **Delta (b) — surrounding copy.** Precede the table with this VERBATIM opening line (Chinese-language sessions: render verbatim; other languages: translate faithfully, preserving meaning, per the §Localization banner):

  > 这是你订阅的服务和每台设备的消息推送状态。想让某台设备开始或停止接收，随时告诉我就行。

  Follow the table with exactly **one** 💡 hint line telling Codex / Claude Code users that messages do not auto-appear — they must say 「监听 + 任务名」 in the conversation to see a task's messages there. The example task name MUST be one of the user's **real** subscribed task titles from this very render — never a hard-coded sample:

  > 💡 在 Codex / Claude Code 里，某个任务的消息不会自动出现——想在对话里看到它，对我说「监听 + 任务名」即可（例如「监听 {填入本次渲染里用户真实订阅的某个 title}」）。

**No follow-up question.** Display only. Do **NOT** ask whether to turn on receipt or start listening (product retracted that prompt) — enabling happens only when the user explicitly asks later.

## Subscription Detail (订阅详情)

Trigger: user selects a row / asks about one subscription (`订阅详情` / `这个订阅的情况` / `subscription detail`). Command: `onchainos agent subscribe-detail <jobId>` — the positional id is the **`jobId`** from the list (the response primary key; there is no separate `subId`). → single `SubscriptionInfo`. Render:

> **{title}** — {statusName}
>
> 订阅方：Agent#{buyerAgentId}
> 服务方：Agent#{providerAgentId}
> 是否在试用期：{trialType==1 ? "是" : "否"}
> 费用：{serviceTokenAmount}（token {serviceTokenAddress 前 6 位}…）/ 周期
> 自动续费：{autoRenew==1 ? "已开启" : "未开启"}
> 已订期数：第 {periodIndex} 期
> 离线交付物：{offlineReceiveFlag==1 ? "清理掉" : "补推给我（默认）"}

- 金额字段（`serviceTokenAmount` / `paymentTokenAmount` / `paymentCurrencyAmount`）是**字符串**，原样展示，绝不转 float。
- token 符号 CLI 不提供，仅有 `serviceTokenAddress`（展示短地址）。
- 离线交付物 = 详情响应的 `offlineReceiveFlag`：`1` → 清理掉；`0` 或字段缺失 → 补推给我（默认）。该字段仅在订阅详情响应中出现——任何地方都要容忍它不存在，缺失时一律按补推给我（默认）渲染，绝不因缺字段报错。

After the card, append a **device table with only the two device columns** — subscription-level fields are already shown in the card above and are NOT repeated. One row per device; the **this-device** row is prefixed with 🌟 and gets the `（本设备）` marker (the product PRD renders e.g. `🌟xxxxxxx（iphone 15）本设备`) — this 🌟 prefix is **exclusive to the §Subscription Detail table**.

| 已登陆设备 | 设备是否接收任务消息 |
|---|---|
| {🌟 if this device}{deviceName}{（本设备）if this device} | {✅是/否 from `thisDeviceReceives` / membership} |

- 已登陆设备 names come from joining the detail's `deviceList` ids against `device-list` rows; **degrade to a raw id / count when a name is unavailable — never fabricate a name**.
- 设备是否接收任务消息 = ✅是 when the device id ∈ `deviceList`; the this-device row reads the CLI `thisDeviceReceives` flag directly.
- Subscribe time fields render as Unix **seconds** (device-list times are ms — different unit).
- **Degraded fallback:** two rows — the this-device row (known) + an explicit `其他设备接收状态暂不可用` row — when the device table is unavailable. Never present this device as the full picture.

## Device List (设备列表)

Trigger: `设备列表` / `我登录了哪些设备` / `哪些设备在线` / `device list`. Command: `onchainos agent device-list` → JSON `{ "list": [ … ], "total", "thisDeviceId" }` (paginated to completion CLI-side; render the full set as-is). Render **three columns — no 是否在线 column** (the CLI emits no `online` field; never synthesize one):

| 设备 | 最后在线时间 | 接收的订阅任务消息 |
|---|---|---|
| {deviceName}{（本设备）if `isThisDevice`} | {lastOnlineLocal} | {derived — see below} |

- **设备**: readable `deviceName` (may be empty → show raw `deviceId` / a count, never fabricate); the `isThisDevice==true` row gets the `（本设备）` marker.
- **最后在线时间**: render `lastOnlineLocal` **verbatim** — it is already CLI-formatted local time; never re-convert or re-parse `lastOnlineTime`.
- **接收的订阅任务消息**: derived by joining each `deviceId` against the subscriptions' `deviceList` (from `my-subscriptions`) — e.g. list which subscriptions that device receives, or 是/否 for a specific subscription in context.
- Empty list (`list: []`) → tell the user no devices are currently listable. If the command errors (endpoint not live yet / transport), see the degraded render in §My Subscriptions / §Subscription Detail — state that device info is temporarily unavailable rather than presenting a partial picture as complete.

## Create-subscribe device preview

Before creating a subscription, show the device table (设备 + 最后在线时间 from `device-list`) and tell the user the task's messages will **auto-push to all logged-in devices**, and any device can be disconnected later. Precede the device table with this VERBATIM pre-create line (Chinese-language sessions: render verbatim; other languages: translate faithfully, preserving meaning, per the §Localization banner):

> 您当前登录了以下设备，本任务消息会自动推送给所有已登陆设备。想让某台设备不再接收，随时告诉我。

On create, the CLI always sends `deviceList` explicitly (all logged-in devices minus any excluded).

- **Degrade:** if `device-list` fails/empty, the create still **proceeds with this device only**, the CLI returns `data.deviceRoutingDegraded: true`, and the skill tells the user only this device was set (do NOT abort). Surface this as a plain notice, not an error.
