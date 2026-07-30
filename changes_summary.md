# Changes Summary — Multi-Device Login: Subscription-Message Device Routing (WBW-14118)

Branch: `feat/wbw-14118-subscription-device-routing` (base `master` @ `6a0b55fd4191c42388754ec21ed728df008e62d2`).
Implements the dev-PRD behavior contract per A-ARCH / A-CLISPEC / A-MCPSPEC / A-SKILLUPD / A-PLAN.

> **This is rework attempt 2.** Attempt 1 was functionally complete and correct; Review flagged one
> binding-rule violation — provenance tokens in new `.rs` comments (PRD R01 / §7.1) — and recommended
> five non-blocking correctness/test-quality improvements (R2–R6). This attempt closes the blocker and
> folds in R2–R6 in the same rework commit. The safety-critical routing/degrade logic is unchanged.

## Rework changes (attempt 2)

### Blocking fix — F1: strip provenance tokens from new `.rs` comments (16 lines)
Rewrote every flagged comment to keep the design-intent prose while dropping the reference token.
No behavior change (comments only). Per-file counts:

| File | Lines | Tokens removed |
|---|---|---|
| `device_routing.rs` | 9 | `AC-01`×3, `AC-06`×3, `AC-02`, `§4.4`, `AC-01` (validate doc) |
| `audit.rs` | 3 | `WBW-14118`×3 |
| `subscription_ops.rs` | 3 | `WBW-14118`×2, `AC-03 / AC-04` |
| `create_subscribe.rs` | 1 | `§4.4` |

Gate (per R01) — `git diff master...HEAD -- '*.rs' | grep '^+' | grep -iE 'WBW-[0-9]|AC-[0-9]|§[0-9]'`
returns **empty**. (Section refs inside skill `.md` docs are legitimate cross-refs and are out of
R01/§7.1 scope — left in place, including in this summary.)

### Non-blocking folds (same commit)
- **R2** — `create_subscribe.rs`: extracted pure `build_create_body` + `build_create_success`
  helpers (behavior-identical) and added request-body assertions proving `deviceList` is ALWAYS
  embedded (even empty) and the success envelope always carries `deviceRoutingDegraded`.
- **R3** — `device_routing.rs`: extracted pure `normalize_page_params`, `pagination_done`,
  `resolve_total`, `device_ids` helpers out of `fetch_all_devices` / `fetch_all_device_ids`
  (behavior-preserving) and unit-tested pagination stop conditions, `< 1` normalization,
  `> 100` pass-through, the `MAX_PAGES` safety cap, and the empty-id filter.
- **R4** — `subscription_ops.rs`: the my-subscriptions struct parse path now uses a tolerant
  `deserialize_with` (`de_opt_str_array`, delegates to `normalize_str_array`), so a non-string
  `deviceList` / `categoryCodes` array element is dropped rather than failing the whole list parse —
  matching subscribe-detail's raw-Value tolerance.
- **R5** — `device_routing.rs`: `resolve_total(page_total, acc_len) = max(page_total, acc_len)` so an
  empty terminal page echoing `total: 0` never under-reports the rows actually aggregated (display-only).
- **R6** — `device_routing.rs`: (a) Form B (`--items`) now rejects an empty `jobId` like Form A;
  (b) `resolve_create_device_set` no longer returns a silent `(empty, degraded=false)` when every
  fetched device is excluded — it degrades (flag `true`), falling back to this device unless it too
  was excluded (then empty + flagged). The verified fetch-failure / empty-fetch degrade paths are
  byte-identical.

## Scope
Two new buyer-side CLI commands under `onchainos agent`, three enriched commands, audit
labels + redaction, and buyer-side `skills/okx-ai/` routing/rendering docs. **No MCP surface**
(grounded exception). Neither new command takes `--chain`. `cli/src/device/` untouched
(only reads `device::id::get_cached_device_id()`); `task-asp.md`, `Cargo.toml`/`Cargo.lock`,
`cli/src/mcp/*`, and SKILL.md frontmatter untouched.

## Files changed (Rust)

| File | Change |
|---|---|
| `cli/src/commands/agent_commerce/task/user/device_routing.rs` | **NEW** — `handle_subscribe_device_update`, `handle_device_list`, `fmt_unix_millis` (ms→local), dual-envelope decode + paginate-to-completion, batch-update normalize/validate/emit, `fetch_all_device_ids` + `resolve_create_device_set` reuse helpers; pure pagination helpers (R3/R5); Form-B empty-jobId reject + all-excluded degrade (R6); 27 unit tests. |
| `cli/src/commands/agent_commerce/mod.rs` | 2 new `AgentCommand` clap variants (`subscribe-device-update`, `device-list`) + `--exclude-device` on `create-subscribe`; 3 `run()` map arms. |
| `cli/src/commands/agent_commerce/task/user/mod.rs` | `mod device_routing;`; 2 new `TaskCommand` variants + `exclude_device` field; 3 `run_task` dispatch arms. |
| `cli/src/commands/agent_commerce/task/user/subscription_ops.rs` | `SubscriptionInfo` + `device_list` / `category_codes` (tri-state tolerant, non-string-element-tolerant via `de_opt_str_array` — R4) + derived `this_device_receives`; `my-subscriptions` per-row normalize + `thisDeviceReceives` + top-level `thisDeviceId`; `subscribe-detail` json-path enrichment; `normalize_str_array` / `device_receives` helpers; tests. |
| `cli/src/commands/agent_commerce/task/user/create_subscribe.rs` | `--exclude-device` param; always sends `deviceList` (all logged-in devices minus excluded); degrade branch (this-device only + `deviceRoutingDegraded:true` + notice, never aborts); `build_create_body` / `build_create_success` pure builders + request-body tests (R2). |
| `cli/src/audit.rs` | `agent_sub()` labels `subscribe-device-update` / `device-list`; `--items`→`REDACT_FULL`, `--job-id`→`REDACT_ADDR`; 3 redaction tests (device-list flags stay visible). |

## Files changed (skills — buyer-side `okx-ai` only)

| File | Change |
|---|---|
| `skills/okx-ai/references/task-user-intent-routing.md` | §Subscriptions: +rows (device-list, start/stop receipt, single-task listen); preserved 订阅-vs-任务 disambiguation. |
| `skills/okx-ai/references/task-cli-reference.md` | +`subscribe-device-update` / `device-list` entries; enriched-output notes on `create-subscribe` / `my-subscriptions` / `subscribe-detail`. |
| `skills/okx-ai/references/task-user-playbook.md` | §My Subscriptions +2 device columns w/ per-device expansion + **mandatory degraded render**; §Subscription Detail 2-column device table + degraded fallback; §Device List (no 是否在线) + §Create-subscribe device preview/degrade; toggle/clear-list/fresh-read safety flows. |

`skills/okx-ai/SKILL.md` frontmatter and `task-asp.md` intentionally **unchanged**. No new dependency.

## Backend endpoints
- `POST /priapi/v1/aieco/task/subscribe/device/batchUpdate` — batch device update; success iff `data == true`.
- `GET  /priapi/v5/wallet/agentic/agent/device-list?page=&pageSize=` — paginate to completion.
- `POST /priapi/v1/aieco/task/subscribe/create` — now always carries `deviceList`.
- `GET  /priapi/v1/aieco/task/subscribe/my` + `/{subId}` — enriched output.

## Grounded drift from the architecture (source code wins)
1. **batchUpdate uses `post_with_identity` (JSON), not `raw_post_with_identity`.** In this codebase
   `raw_post_with_identity` is for hand-rolled multipart bytes and routes through the same response
   handler that unwraps `data` / errors on `code != "0"` — identical semantics, so the JSON variant
   is the faithful, simpler choice. Success = `Ok(Value::Bool(true))`; otherwise error echoing `data`.
2. **device-list uses `get_with_agent_id` (JWT + agenticId), not `get_with_identity`.** The latter
   appends `?sessionCert=…`, which cannot coexist with the `?page=&pageSize=` query string (double `?`).
3. **`DevicePage` models only `{list,total}`** (backend `page`/`pageSize` ignored — the CLI echoes the
   request inputs per spec).

## MCP
No MCP surface added (grounded exception, A-MCPSPEC). `cli/src/mcp/mod.rs` still has zero
`subscri` / `device-list` / `batchUpdate` / `fetch_all_device` hits.

## Explicit "NOT done" (for the MR description)
- **Post-login heartbeat** — NOT this run (login-path regression risk; degrade path covers it).
- **Offline-message-backfill** — out of scope (no backend endpoint; must not appear in any template).

## Verification
See `verification_report.md` for the exact scoped build/fmt/clippy/test commands, the provenance
gate result, the 90 scoped / 1994 full passing tests, the pre-existing exclusions (ONC-12), the CLI
smoke (incl. the mandatory degraded device-list path and the new R6a Form-B rejection), and the MCP /
scope guards. Repeat this summary + the two "not done" decisions in the MR description.
