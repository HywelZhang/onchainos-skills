# Changes Summary — Multi-Device Login: Subscription-Message Device Routing (WBW-14118)

Branch: `feat/wbw-14118-subscription-device-routing` (base `master` @ `6a0b55fd4191c42388754ec21ed728df008e62d2`).
Implements the dev-PRD behavior contract per A-ARCH / A-CLISPEC / A-MCPSPEC / A-SKILLUPD / A-PLAN.

## Scope
Two new buyer-side CLI commands under `onchainos agent`, three enriched commands, audit
labels + redaction, and buyer-side `skills/okx-ai/` routing/rendering docs. **No MCP surface**
(grounded exception). Neither new command takes `--chain`. `cli/src/device/` untouched
(only reads `device::id::get_cached_device_id()`); `task-asp.md` untouched.

## Files changed (Rust)

| File | Change |
|---|---|
| `cli/src/commands/agent_commerce/task/user/device_routing.rs` | **NEW** — `handle_subscribe_device_update`, `handle_device_list`, `fmt_unix_millis` (ms→local), dual-envelope decode + paginate-to-completion, batch-update normalize/validate/emit, `fetch_all_device_ids` + `resolve_create_device_set` reuse helpers, 19 unit tests. |
| `cli/src/commands/agent_commerce/mod.rs` | 2 new `AgentCommand` clap variants (`subscribe-device-update`, `device-list`) + `--exclude-device` on `create-subscribe`; 3 `run()` map arms. |
| `cli/src/commands/agent_commerce/task/user/mod.rs` | `mod device_routing;`; 2 new `TaskCommand` variants + `exclude_device` field; 3 `run_task` dispatch arms. |
| `cli/src/commands/agent_commerce/task/user/subscription_ops.rs` | `SubscriptionInfo` + `device_list` / `category_codes` (tri-state tolerant) + derived `this_device_receives`; `my-subscriptions` per-row normalize + `thisDeviceReceives` + top-level `thisDeviceId`; `subscribe-detail` json-path enrichment; `normalize_str_array` / `device_receives` helpers; 5 new tests. |
| `cli/src/commands/agent_commerce/task/user/create_subscribe.rs` | `--exclude-device` param; always sends `deviceList` (all logged-in devices minus excluded); degrade branch (this-device only + `deviceRoutingDegraded:true` + notice, never aborts); output + test update. |
| `cli/src/audit.rs` | `agent_sub()` labels `subscribe-device-update` / `device-list`; `--items`→`REDACT_FULL`, `--job-id`→`REDACT_ADDR`; 3 new redaction tests (device-list flags stay visible). |

## Files changed (skills — buyer-side `okx-ai` only)

| File | Change |
|---|---|
| `skills/okx-ai/references/task-user-intent-routing.md` | §Subscriptions: +4 rows (device-list, start/stop receipt, single-task listen); preserved 订阅-vs-任务 disambiguation (device routing = subscription concept, buyer side). |
| `skills/okx-ai/references/task-cli-reference.md` | +`subscribe-device-update` / `device-list` entries; enriched-output notes on `create-subscribe` / `my-subscriptions` / `subscribe-detail`; Contents index updated. |
| `skills/okx-ai/references/task-user-playbook.md` | §My Subscriptions +2 device columns w/ per-device expansion + **mandatory degraded render**; §Subscription Detail 2-column device table + degraded fallback; new §Device List (no 是否在线) + §Create-subscribe device preview/degrade; §Subscription management toggle/clear-list/fresh-read safety flows. |

`skills/okx-ai/SKILL.md` frontmatter and `task-asp.md` intentionally **unchanged**. `cli/Cargo.toml` / `Cargo.lock` untouched (no new dependency).

## Backend endpoints
- `POST /priapi/v1/aieco/task/subscribe/device/batchUpdate` — batch device update; success iff `data == true`.
- `GET  /priapi/v5/wallet/agentic/agent/device-list?page=&pageSize=` — paginate to completion.
- `POST /priapi/v1/aieco/task/subscribe/create` — now always carries `deviceList`.
- `GET  /priapi/v1/aieco/task/subscribe/my` + `/{subId}` — enriched output.

## Grounded drift from the architecture (source code wins — arch §0)
1. **batchUpdate uses `post_with_identity` (JSON), not `raw_post_with_identity`.** In this codebase
   `raw_post_with_identity` is for hand-rolled multipart bytes (`Vec<u8>` + content-type) and routes
   through the same `wallet_api::handle_response` that unwraps `data` and errors on `code != "0"` —
   it does NOT return the full `{code,msg,data}` envelope the arch assumed. `post_with_identity`
   (JSON body) is therefore the faithful, simpler choice with identical response semantics. Success =
   `Ok(Value::Bool(true))`; `code != "0"` → `Err` (backend msg verbatim); `code "0"` + non-`true`
   `data` → error echoing the returned `data`.
2. **device-list uses `get_with_agent_id` (JWT + agenticId), not `get_with_identity`.** The latter
   appends `?sessionCert=…`, which cannot coexist with the `?page=&pageSize=` query string (double
   `?`). The wallet endpoint authenticates with JWT + agenticId; `get_with_agent_id` sends exactly
   that (mirrors `my-subscriptions`).
3. **`DevicePage` models only `{list,total}`** (backend `page`/`pageSize` ignored — the CLI echoes the
   request inputs per spec) → also avoids a clippy `dead_code` warning under `-D warnings`.

## MCP (T9)
No MCP surface added (grounded exception, A-MCPSPEC). Verified `cli/src/mcp/mod.rs` still has zero
`subscri` / `device-list` / `batchUpdate` hits.

## Verification
See `verification_report.md` for the exact scoped build/fmt/clippy/test commands, outputs,
pre-existing exclusions (ONC-12), CLI smoke (incl. the mandatory degraded device-list path), and the
two explicit "not done" decisions (post-login heartbeat; offline-message-backfill).
