# User Session Playbook — Protocol Card (lite)

> Buyer-side user session: publish / subscribe / subscription ops / device routing / pause auto copy-trade / signal-receipt watch. Answer directly — **do NOT 6-step forward**, no sub-session handoff. Load-cost: this card ONLY; ❌ never also load `task-core.md` / `task-user-sub-playbook.md` (sub-session files = context bloat). Pause/stop auto copy-trading → jump straight to §Pause auto copy-trade.
> 🌐 Localization: user-facing copy in the user's language — English verbatim; non-English faithful translation, labels/values/structure preserved. Instructions stay English; zh-CN strings → `labels.zh-CN.md`. Escalate (§Escalation) on any doubt.

## Reading Order

| When | File | Rule |
|---|---|---|
| publish a task | `task-user-actions-publish.md` | on demand |
| attachment / provider switch / deliverables | `task-user-actions.md` | on demand — ONLY §2 / §3 / §4 |
| a specific command | `task-cli-reference.md` | **do NOT read full file — grep** |

⚡ Re-reading an in-context file costs 1 LLM round + thousands of tokens for zero new info.

## User Intent Routing

Free-form task text, no pending decision matches → load `task-user-intent-routing.md`, follow its flow.

| Intent | Trigger examples | Route to |
|---|---|---|
| Publish task | "subscribe / subscription task / publish / create a task / use or buy a service from Agent/ASP #XXXX / initiate a direct conversation with this provider" | `task-user-actions-publish.md` |
| Add attachment / image | "attach a file/image to a task" | `task-user-actions.md` §2 |
| Switch provider / stop task | "switch provider / stop task" | `task-user-actions.md` §3 |
| View deliverables | "view / list deliverables" | `task-user-actions.md` §4 |
| Designated-provider x402 | "send a request to this endpoint" | `task-user-actions-publish.md` §5 |
| Subscription ops | "auto-renew / trial cancel / reject delivery / apply for refund / claim refund / my subscriptions / subscription charge / subscription cost" | §Subscription below |
| Negotiate with provider | "negotiate with XXX" | sub session handles automatically |
| Re-submit / nudge | "re-submit / nudge" | `task-user-intent-routing.md` |
| Task list / status / close / decision list | "my tasks / view decisions / close task" | `task-user-intent-routing.md` |

**Subscription tiebreaker vs `okx-agent-payments-protocol`** (bare "subscribe / subscription / my subscriptions", no AI-task or payment context — decide BEFORE loading a reference):
- AI-service / agent-marketplace context (`jobId` / `subId` / ASP / Agent#N / provider / task / trial / renew / deliver / `periodCount`) → **stays here** (§Subscription).
- Payment context (HTTP 402 / Permit2 / allowance / API endpoint URL / `paymentId` / recurring API billing) → `okx-agent-payments-protocol`.
- Neither → ask once: AI-service subscription (agent marketplace) or paid-resource subscription (x402)?

**Answer-directly:** `my subscriptions` / `subscription detail` / `device list` / receipt on-off / offline replay-discard — answered here; never routed to ASP/provider rendering. **Device routing = a subscription concept** (A2A subscription-service delivery only; never one-shot tasks). Pause/stop auto copy-trading: never load `task-user-sub-playbook.md`, query subscription state, or resolve an agent id.

## Deposit-address QR (insufficient-balance — MANDATORY)

🛑 If `fundingNoticeCommand` exists, run it and follow its output exactly. `image-notify` → `markdownImage` under option 1. Never summarize the 4 options / address / gas / resume.

## Subscription

### Subscription-specific field rules

| Field | Rule |
|---|---|
| `serviceId` | from `task-service-select` response — auto-filled |
| `useTrial` | `subscriptionInfo.supportTrial==true` → auto `true` else `false` (hours from `subscriptionInfo.freeTrial`) — auto-filled, **do NOT ask** |
| `autoRenew` | ask explicitly before the form, no default — 0=off, 1=on |
| `serviceTokenAmount` | from `task-service-select` `subscriptionInfo.feeAmount` — must match the selected fee |

**Automatic signal execution** (local config — NOT an ASP business parameter; never in `serviceParams`): default `auto`; explicit opt-out → `manual`. Inspect the ASP description ONLY to learn which supported settings to ask about; persist mode/amount/cap/quote/environment/margin mode/order policy ONLY from the user's reply. Amount + cap: optional positive decimals; quote default `USDT`; Trade Kit env `live`/`demo`; margin `cross`/`isolated`; order policy `market`/`signal_price_limit`. Missing fields → ONE natural-language question, no choices. **Never render** mode / per-signal amount / per-signal cap / margin mode / order policy as confirmation rows (existing Trade Kit environment row = only display exception).

**Signal preflight** (schema-v2 `autoTradePreflight` advisory — optional prep, NOT a subscription input): non-ready / auth-not-checked Trade Kit → exactly ONE optional two-choice card: install/configure Trade Kit | Later, continue subscribing. Prepare → load `okx-cex-auth` directly if already installed; ONLY if unavailable: scope the security scan to `okx/agent-skills`, install after a passing scan, load `okx-cex-auth`. Delegate all CLI/OAuth/API-key setup to it; re-run readiness. **Never auto-install; never block subscription creation**; other tool reminders = concise notices.

**`autoTradeConfigured`** (success envelope): `true` → no extra execution-consent question. `false` → subscription succeeded but local config not persisted — report failure, NO decision card. **`next-action` route:** its confirmation form is the sole field authority — never merge Skill-appendix / other-card fields; `task-user-actions-publish.md` **Appendix A2** only for a direct/fallback route with no CLI confirmation form.

### Post-creation: Offline-deliverables question

After `create-subscribe` success, render the block verbatim / translated (`{jobTitle}` = **just-created REAL title**, never a sample) and do **NOT** wait — continue immediately to §Post-creation: Watch check; preference handled when the reply arrives; never delays initial watch / `sub_created`. Render the device-routing line too (order: success title → device line → question; informational — no question, no wait; never end/pause for it).

> "{jobTitle}" subscription created ✅
> Messages will go to all logged-in devices. You can change device delivery anytime.
> This task keeps producing deliverables while you are offline. What should happen when you return?
> · Replay Missed Deliverables (default) — deliver them when you return; the background process keeps receiving and processing them
> · Discard Offline Deliverables — drop them while offline so the background process does not consume resources
> 💡 In Codex / Claude Code, replayed messages first reach the background process. To see them here, say "listen to {jobTitle}."

**Old comm-package branch:** read `offlineReplaySupported` from the success envelope (CLI already probed it — **never run `okx-a2a capabilities` yourself**). `false` → append verbatim / translated (question/options + 💡 byte-identical; device line between title and question):
> 💡 This communication package does not yet support offline-replay preferences. Your choice is saved and takes effect after upgrading (`{fixCommands}`); until then, all subscription messages are replayed normally.

`{fixCommands}` ← envelope `offlineReplayFixCommands`, one per line. `true`/absent → add nothing.

Reply: no choice / replay / keep → **do NOT write** (server default `0` = replay). Discard → `onchainos agent subscribe-offline-update --job-id <this jobId> --flag 1`; then `offlineReplaySupported` `true`/absent → "Offline deliverables will be discarded, not replayed."; `false` → "Preference saved: offline deliverables will be discarded after the communication package is upgraded; until then, they will still be replayed." Write failure → do **NOT** roll back / retry creation; say not saved, replay default stands, changeable later. Notice, not an error.

### Post-creation: Watch check (mandatory)

Offline question rendered without waiting → NOW inspect CLI output, start watch; never await the preference reply.
- `[Watch]` block present → read `skills/okx-ai/references/watch-core.md`, enter its Watch generation. Notification / deliverable / empty poll does NOT end the turn — dispatch the full batch, re-enter the same scoped command until `watch-core.md` says stop or a `decision_request` needs the user.
- No `[Watch]` block → **end the turn immediately**.

🛑 This handoff = **last non-Watch action in the creation flow**; `watch-core.md` owns the rest of the turn. No unrelated creation commands after it; "last creation action" ≠ stop after the first watch result. `sub_created` → notification + start watch ONLY: no DApp-name re-scan, no auto plugin install, no tool pre-select (install/config surfaced up-front as non-blocking `autoTradePreflight`; visible flow only on explicit choice). Fresh Trade Kit probe on every delivery that resolves to Trade Kit. Failed delivery stays visible + execution-blocked; restoring readiness never auto-replays it; future deliveries continue normally.

### Subscription management (user-initiated)

**Update core (all `subscribe-device-update` / `subscribe-offline-update` rows):** fresh-read FIRST (`subscribe-detail <id> --format json` / `my-subscriptions`); build `--device-list` from that read, never memory (command overwrites wholesale — short by one id = that device silently stops); re-read after every write. Tri-state / clear-list / fresh-`null` / neutral copy → §Pause auto copy-trade safety flows. No `subId` on other ops → `subscribe-detail` or ask; signal-receipt phrases → §Signal-receipt watch entry, NOT this fallback.

| Intent | Command | Intent-specific rules |
|---|---|---|
| Detail | `subscribe-detail {subId} --format json` | **always `--format json`** when consuming fields (text default lacks `thisDeviceReceives` / joined names) |
| Enable auto-renew | `start-autorenew {subId}` | on-chain, EIP-712 sign; may need approve |
| Cancel (trial cancel / close auto-renew) | `subscribe-cancel {subId}` | unified: trial → cancel auto-conversion, no charge, Closed; active → close auto-renew, current period runs to expiry |
| Refund family | `reject {id} --reason "..."` | **unified** (auto-detects by jobType) — ALWAYS `reject` first for refund / apply-for-refund / reject-delivery / dispute / request-evaluation / arbitration |
| Claim refund after timeout | `claim-auto-refund {id}` | 🛑 **NEVER first** — only after `reject` AND ASP misses the 1-day window |
| Active cost | `subscribe-cost` | total monthly cost of active formal subscriptions; no params |
| Pause / stop auto copy-trading | `autotrade-consent-set --job-id <jobId> --mode pause` | → §Pause auto copy-trade; do NOT load `task-user-sub-playbook.md` / query state / resolve agent id |
| Receiving on THIS device | `subscribe-device-update --job-id <id> --device-list <fresh + this device>` | `deviceList:null` (default-all) → already receiving, do **NOT** write. Explicit array: device present → no write; else union → write → re-read → `✅ Yes (added now)` |
| Receiving on named device(s) | `subscribe-device-update --job-id <id> --device-list <fresh ∪ named ids>` | resolve name→id via `device-list`, never fabricate. `null` → all logged-in already receive: no change, do **NOT** write. Else union → overwrite → re-read → "Okay, Y will now be sent to X1 and X2." — complete receiver set, readable names (2 → `and`; 3+ → commas + `and`) |
| Stop pushing to a device | `subscribe-device-update --job-id <id> --device-list <explicit set − device>` | subtract from explicit fresh `deviceList`. `null` → materialize complete `device-list` minus target; unavailable → STOP. Never `null` → `[]` / partial. Re-read: non-empty → "Stopped sending Y to X. This task now goes only to Z." (count if unnamed — never invent); empty → "Stopped sending Y to X. No device now receives this subscription." |
| Offline handling later | `subscribe-offline-update --job-id <id> --flag <0\|1>` (0=replay, 1=discard) | flag == `offlineReceiveFlag` → no change, do **NOT** write; else write → re-read. `1` → confirmations per §Post-creation; `0` keeps behavior |
| List devices | `device-list` | render §Device List; `lastOnlineLocal` already CLI-derived |
| Receive / verify / resume signals | — | §Signal-receipt watch entry: one ACTIVE buyer subscription, this device receives without dropping others, sticky scoped watch; never guess historical jobId / global watch |
| Listen, no task specified | — | confirm exactly one task ("Only one task can be watched at a time") → enable this-device receipt → `watch-core.md` existing-subscription scoped-watch authorization gate → messages appear live here |

### Signal-receipt watch entry

Trigger (any language): receive / start receiving signals · are you receiving signals · resume watching subscribed services · continue receiving signals · resume subscription · restore subscription; prompted `listen to <title>` when the title resolves from the just-created/rendered buyer-subscription context; bare restore/resume with an ACTIVE buyer subscription in focus (even without "signals"/"watch"). Interrogative → read-only ONLY when the same message asks why/how/basis or device config (not watch start). Compound request: stop in steps 1–3 ends only this branch — continue each independently authorized action unless conditional on receipt success.

1. **Resolve exactly one ACTIVE buyer subscription.** Named title/jobId wins; else ONE unambiguous current focus (fresh list/detail, the notification replied to, the active scoped-watch exchange) — recency alone NEVER counts. Bare action, new session, no focus → `onchainos agent my-subscriptions --role buyer`, keep `statusName == "ACTIVE"` only: one → proceed; several → user chooses; zero → stop, explain. Never guess a historical jobId / global watch.
2. **Fresh-read receipt:** `subscribe-detail <jobId> --format json`. Not ACTIVE anymore → explain no new business signal possible; stop, no watch.
3. **This device receives, no receiver dropped:** `thisDeviceReceives == true` → no write (preserve `deviceList:null`). `false` + `null` → inconsistent routing: explain, stop, no write/watch. `false` + explicit array → resolve this device id, UNION, `subscribe-device-update`, re-read; missing id / malformed / write failure / read-back failure → stop without watch. Latest detail MUST show `thisDeviceReceives == true` right before watch.
4. **Sticky scoped watch via the authorization gate:** load `watch-core.md`, run §Existing-subscription scoped-watch authorization gate BEFORE banner / any watch call. Pass → canonical banner + `okx-a2a user watch --json --job-id <jobId>`; jobId sticky per re-entry. Never global watch; never claim starting watch proves a new signal.

### Restore execution-configuration reply

Reply to a preceding `autotrade-watch-precheck` restore-config ask binds ONLY to that turn's exact local `continuationId`: `autotrade-consent-continue --job-id <sameJobId> --agent-id <sameAgentId> --continuation-id <exactId>` with ONLY `--trade-amount` / `--cap` / `--quote` / `--environment` / `--margin-mode` / `--order-policy` values authored in that reply. Explicit disable → add `--mode manual`; affirms displayed auto default → add `--mode auto`. Mode on resume = recorded confirmation — never treat the continuation's default `auto` as confirmation. Never infer values / authorization from ASP prose.
`validationErrors` / `missingFields` remain → ask once ONLY those, natural language, end turn. Complete → run the exact returned `consentCommand`, then resume that subscription at §Signal-receipt watch entry step 2 (fresh-read before watch). Never A/B/C; never a delivery-time authorization decision. Generic amount/currency message without the preceding bound prompt ≠ consent authority.

### Pause auto copy-trade

Latency-sensitive, user-session-owned local authorization toggle — clears automatic-execution authorization for **that one subscription** so a later actionable signal requests execution configuration again:

```bash
onchainos agent autotrade-consent-set --job-id <jobId> --mode pause
```

- `jobId` ← the copy-trade notification being replied to; bare request, >1 auto-following subscription → ask which, never guess.
- Do **not** query subscription detail / resolve agent id / load `task-user-sub-playbook.md` / add an extra confirmation. Scope = this `jobId` only.
- Success → existing `consentMode:"pause"`, `cleared:true`, `jobId`. Tell the user: automatic execution paused; subscription + signal receipt remain active. Does NOT cancel the subscription; does NOT disable signal receipt.

**Device-routing safety flows (copy/behavior; canonical for every device render/update in this card):**
- **Tri-state (never collapse):** `deviceList:null` / missing = unconfigured → **all logged-in buyer devices receive by default**; `[]` = none chosen; non-empty array = only those ids. (`thisDeviceReceives` already applies this buyer-side.) Never truthiness / `unwrap_or_default` reasoning equating `null` with `[]`.
- **Clear-list:** emptying the list → warn "No device will receive this subscription", confirm before writing.
- **Fresh `null` = a routing mode, not an empty base list:** enabling any device = no-op; disabling one requires materializing the complete `device-list` first.
- **Neutral copy:** promise only "messages for this subscription task" — no system-notification-scope promise.

### Reject + refund flow (detailed)

Refund / apply for refund / reject delivery / dispute / evaluation / request evaluation / arbitration → `reject` — unified, auto-detects by `jobType`. 🛑 `claim-auto-refund` NEVER the entry point (Step 3 only). Arbitration aliases → `reject` directly, no legacy-role rename prompt (task actions ≠ Evaluator role).
1. **Reject (on-chain, user initiates):** `onchainos agent reject {id} --reason "quality not met"` → subscription: `/subscribe/{id}/reject`; regular: pre-reject/reject dual-sign → `Rejected` → ASP has **1 day**.
2. **ASP responds:** A. agrees refund → `sub_asp_agree` → `Failed` (funds returned). B. files dispute → `sub_asp_dispute` → `Disputed` (awaiting DM evaluation). C. silent past 1 day → user may claim:
3. **Claim (only after ASP timeout):** `onchainos agent claim-auto-refund {subId}` → `Failed` (funds returned).

Rules: `reject` requires `--reason` (≤2000 chars); subscriptions: **one rejection per subscription**. `claim-auto-refund`: only status = `Rejected` AND the window passed. Dispute filed → wait for the Dispute Manager's ruling (existing on-chain flow).

## My Subscriptions (buyer view)

Trigger: `my subscriptions` / `subscription list` / `what am I subscribed to`. Run `onchainos agent my-subscriptions --role buyer` → `{ "list":[…], "thisDeviceId", "thisDeviceName" }` PLUS `onchainos agent device-list`. One row per subscription; **never drop Provider / Billing Period; Next Charge = ONE date, not a period**; one column per real device (readable names — never D1/D2). Legend above (localized): `✅ Receives task messages; ❌ Does not receive task messages`. Row: `{title} | Agent#{providerAgentId} | {statusName} | {serviceTokenAmount} | {nextCharge} | {autoRenew==1?"✓":"✗"} | {billingPeriod} | device cells…`
- **Status:** `statusName` verbatim (`ACTIVE/REJECTED/DISPUTED/COMPLETED/CLOSED/FAILED/INIT/UNKNOWN_<n>`). **Fee:** `serviceTokenAmount` string verbatim, never float (only `serviceTokenAddress` — no symbol). **Billing Period:** `trialType==1` → `Trial Period`; positive `periodIndex` → `Billing Period {periodIndex}`; else `—`.
- **Next Charge** (derived; no CLI field): non-`ACTIVE` → `—`; trial → prefer `trialEndTime`, fallback legacy `trailEndTime` (AC-17), trial-conversion date, `Date Unavailable` if both absent; else `autoRenew==1` → `subEndTime`; `autoRenew==0` → `No Renewal`. Epoch s → locale date.
- **Device columns:** build once — `thisDeviceId` first + `(This Device)`, others in `device-list` order; no routing summaries / repeated rows / aliases; wide OK. Names / per-cell tri-state / degraded → canonical rules in §Subscription Detail.
- **Display-only:** never proactively offer receipt-on on a list render (product retracted that prompt) — only on explicit request. Empty → "You have no subscriptions." Row detail → pass its **`jobId`** to `subscribe-detail`.

## Post-login subscription display (login-flow-triggered)

**Trigger:** a newly completed wallet login — NOT standalone free-text intent, NOT `wallet status`. [`wallet.md`](../../okx-agentic-wallet/references/wallet.md) owns the entry (step 3 after a successful login poll). Do **NOT** add trigger words to `SKILL.md`.
**Data (mandatory):** successful `wallet login --phase poll` may return the aggregated snapshot at `data.postLoginSubscriptions` — `subscriptions` = exact buyer `my-subscriptions` payload; `devices` = complete `device-list` payload (or `null` on query failure). `wallet status` never returns it. Consume the snapshot; **never run a follow-up `my-subscriptions` / `device-list` here.** User-initiated §My Subscriptions stays separate.
**New-device routing (login only):** non-empty User `agenticId` → before the login heartbeat the CLI checks device existence in the complete device table, then ALWAYS sends the heartbeat (probe success optional). Proved-new device: durable state → registered → added to EVERY subscription's explicit `deviceList` by fresh-list union + batched overwrite (≤100 items/request); `deviceList:null` stays null (already default-all). Progress persisted per confirmed batch; state `completed` before rendering → retries touch only unfinished jobs; cleanup failure can't re-enable a later manual opt-out. Snapshot returned only after routing succeeds → table never appears before the new device is configured. Already-registered device, no pending work → never rewritten on re-login. No `agenticId` / probe fails → heartbeat still registers/refreshes, but routing + table suppressed.
**Zero-disturb (mandatory):** snapshot omitted when the lookup errors (no identity / transport / auth), times out, or returns an empty list → output NOTHING OKX.AI-related (no table / line / 💡 / error / mention). Login concludes normally.
**Render:** reuse §My Subscriptions as-is (matrix, real names, ordering/disambiguation, tri-state, `thisDeviceReceives` authority, legend, degraded render). Above legend + table (verbatim / translated): "Here are your subscriptions and each device's message-receipt state. You can change device delivery anytime." Below, exactly ONE 💡 with a REAL title from this render (never a sample): "💡 In Codex / Claude Code, task messages do not appear automatically. To see them here, say \"listen to {a real subscribed title from this render}.\""

### Post-login executable-subscription profile restore

ACTIVE executable subscriptions received by this device: CLI restores the bounded execution profile but **never creates / changes local consent** — no `autoTradeAuthorizationPrechecks`, no auth ask, no decision card at login. Existing `auto` / `manual` preserved; missing policy configured only on explicit restore of that subscription's watch; unreadable policy = blocking local error. Login: never ask receipt/listening; enabling only on explicit later request.

## Subscription Detail

Trigger: row select / `subscription detail` / `show this subscription`. `onchainos agent subscribe-detail <jobId> --format json` — id = the row's **`jobId`** (primary key; no separate `subId`) → one `SubscriptionInfo`; **`--format json` mandatory when consuming fields** (default lacks `thisDeviceReceives` / joined names). Card:

> **{title}** — {statusName}
> Subscriber: Agent#{buyerAgentId}
> Provider: Agent#{providerAgentId}
> Trial: {trialType==1 ? "Yes" : "No"}
> Fee: {serviceTokenAmount} (token {serviceTokenAddress[0:6]}…) / period
> Auto-Renew: {autoRenew==1 ? "On" : "Off"}
> Billing Period: {periodIndex}
> Offline Deliverables: {offlineReceiveFlag==1 ? "Discard" : "Replay (Default)"}

- Amount fields (`serviceTokenAmount` / `paymentTokenAmount` / `paymentCurrencyAmount`) = **strings** — verbatim, never float; only `serviceTokenAddress` (no symbol) — short address. Offline Deliverables = `offlineReceiveFlag`: `1` → `Discard`; `0`/absent → `Replay (Default)`; exists ONLY here — tolerate absence, never error.
- After the card: **two-column device table** (no field repeats), one row per device — `| Logged-in Device | Receives Task Messages |`. Current-device row: 🌟 prefix + `(This Device)` (e.g. `🌟xxxxxxx (iPhone 15) (This Device)`); 🌟 **exclusive to §Subscription Detail**.
- **Device-name rules (canonical for all renders):** names from joining explicit `deviceList` × `device-list`; `null` → every logged-in buyer device (default-all). Readable `deviceName`, escape Markdown separators / line breaks; duplicates → short device-id suffix each (keep `(This Device)`); `deviceList` id absent from a usable table → `Device Name Unavailable ({short deviceId})`; empty name → raw id / count. **Never fabricate a name.**
- **Per-cell receipt (canonical):** `null` → every buyer device ✅ Yes; `[]` → none; array → membership; current-device row ALWAYS = CLI `thisDeviceReceives`, never recompute. Units: subscription times = Unix **seconds**; device-list times = **ms**.
- **Degraded (MANDATORY — device table unavailable):** known current device + `Other device receipt states unavailable` rows (§My Subscriptions variant: row per subscription + one `{thisDeviceName} (This Device)` column; above-table line "Other device names and receipt states are unavailable."; no name → `Device Name Unavailable ({short thisDeviceId})`, never bare `(This Device)`). Never present one device as the full set.

## Device List

Trigger: `device list` / `list my logged-in devices` / `which devices are online`. `onchainos agent device-list` → `{ "list":[…], "total", "thisDeviceId" }` (CLI paginates to completion — render the full set). **Three columns — no Online** (CLI emits no `online`): `| Device | Last Online | Received Subscription Messages |`
- Device: readable `deviceName`; empty → raw id / count, never fabricate; `(This Device)` when `isThisDevice==true`. Last Online: `lastOnlineLocal` **verbatim** — never re-convert / parse `lastOnlineTime`.
- Received: join each `deviceId` × subscription `deviceList` from `my-subscriptions` (`null` = all logged-in; `[]` = none; array = membership). List subscriptions received, or Yes/No per subscription.
- `list:[]` → no devices currently listable. Error (endpoint not live / transport) → degraded render per §Subscription Detail / §My Subscriptions; never partial-as-complete.

## Create-subscribe device routing

Only `create-subscribe` signal subscriptions — NOT `create-task`. Creation defaults to ALL logged-in devices: do **not** run `onchainos agent device-list` before, no device table, no branching on count / names. Create-time device selection/exclusion **unsupported** — user asks to choose/include/exclude → explain: starts on all logged-in devices; adjust after creation. Never translate the request into CLI flags; never claim it was applied. Post-creation view/change requests → §Device List + §Subscription management (fresh-read).

## Renders — zh/EN examples (i18n)

Convention: EN structure / field labels with faithful zh-CN values (`labels.zh-CN.md` §2 / §6); machine enums verbatim.

Example 1 — subscription list row (§My Subscriptions, zh-CN):
```
1 | BTC 信号订阅 | Agent#12 | ACTIVE 订阅生效中 | 10 | 2026-10-03 | ✓ | Billing Period 2 | 我的 MacBook Pro (This Device) ✅ | Kevin 的 MacBook ✅
```
(EN: Service | Provider | Status (`statusName` verbatim + zh label) | Fee (raw string) | Next Charge (one date) | Auto-Renew | Billing Period | device cells = status gate + tri-state; this-device cell from `thisDeviceReceives`.)

Example 2 — pause auto copy-trade confirmation (zh-CN):
```
⏸ 已暂停自动跟单（订阅 Agent#12 · BTC 信号）
自动执行已暂停：该订阅的下一条信号将先请求执行配置，不会自动下单。
订阅与信号接收保持生效（未取消订阅）。
```
(EN: `consentMode:"pause"`, `cleared:true` → paused; subscription + signal receipt remain active; neutral copy — only this subscription task's messages promised.)

## Escalation

→ Full reference: `task-user-playbook.md` — verbatim templates, render nuances, login-flow detail, exception branches. Read it when this card leaves any ambiguity, an exception path applies, or a flow needs copy not reproduced here.
→ UI strings: `labels.zh-CN.md`. Sibling routing: `task-user-intent-routing.md` (free-form text), `watch-core.md` (watch entry / authorization gate).
