# Verification Report — WBW-14118 (Subscription-Message Device Routing)

Branch `feat/wbw-14118-subscription-device-routing`, base `master` @ `6a0b55fd`.
**Rework attempt 2** — closes the single Review blocker (provenance tokens in new `.rs`
comments) and folds in the non-blocking findings R2–R6. The safety-critical routing/degrade
logic was verified correct in attempt 1 and is preserved.

> **Scoped verification (ONC-12).** This private fork substitutes the repo-wide `onchainos_check`
> gate — which checks whole files / recursive trees and fails on pre-existing `master` debt — with
> **correctly scoped** equivalents recorded verbatim below. All `cargo` checks are scoped to the
> changed files/modules; `cargo fmt --all` and whole-crate `clippy` cleanups are deliberately NOT run
> (primer §10). Pre-existing failures are reproduced on `master` and excluded.

## Changed `.rs` files (`git diff --name-only master...HEAD -- '*.rs'`)
```
cli/src/audit.rs
cli/src/commands/agent_commerce/mod.rs
cli/src/commands/agent_commerce/task/user/create_subscribe.rs
cli/src/commands/agent_commerce/task/user/device_routing.rs
cli/src/commands/agent_commerce/task/user/mod.rs
cli/src/commands/agent_commerce/task/user/subscription_ops.rs
```

## 0. Provenance gate (Review binding rule R01 / §7.1 — the blocker this rework closes)
The rule bans provenance tokens (Jira keys, AC numbers, section refs) in **new `.rs` source
comments** with the same force as raw `println!` JSON or CJK in source. All 16 flagged comment
lines were rewritten to keep the design-intent prose and drop the reference token (e.g.
`// AC-02: always send deviceList` → `// always send deviceList when routing is configured`).

**Gate command (per R01) — MUST return empty:**
```
git diff master...HEAD -- '*.rs' | grep '^+' | grep -iE 'WBW-[0-9]|AC-[0-9]|§[0-9]'
→ (empty)   # exit 1 (no match) = PASS
```
Scope note: this applies to `.rs` comments ONLY. Section refs inside skill `.md` docs are
legitimate documentation cross-refs and are intentionally left in place (and in this report).

## 1. Build
- `cargo build` (debug) → **exit 0**.
- `cargo build --release` → **exit 0**, binary `cli/target/release/onchainos` (`onchainos 4.4.3`).

## 2. Tests
- Scoped: `cargo test --bin onchainos -- device_routing subscription_ops create_subscribe audit`
  → **90 passed, 0 failed** (was 77; +13 new tests from R2–R6, see below).
- Full regression: `cargo test --bin onchainos` → **1994 passed, 0 failed** (was 1981).
- New tests added this rework:
  - **R2** (`create_subscribe.rs`): `create_body_always_embeds_device_list_even_when_empty`,
    `create_body_carries_devices_and_provider_when_present`,
    `create_success_envelope_carries_degrade_marker` — request-body assertions that `deviceList`
    is ALWAYS embedded (even empty) and the success envelope always carries `deviceRoutingDegraded`.
  - **R3** (`device_routing.rs`): `normalize_page_params_*` (floor <1, pass-through incl. >100),
    `pagination_done_*` (empty / short / reached-total / MAX_PAGES cap / continue-on-full-page),
    `resolve_total_never_under_reports_aggregated_rows`, `device_ids_drops_empty_ids`.
  - **R4** (`subscription_ops.rs`): `my_subscriptions_struct_parse_tolerates_non_string_array_elements`
    — struct (my-subscriptions) parse path now as tolerant as subscribe-detail's `normalize_str_array`.
  - **R6** (`device_routing.rs`): `normalize_form_b_rejects_empty_job_id`,
    `create_device_set_all_excluded_degrades_to_unexcluded_this_device`,
    `create_device_set_all_excluded_incl_this_device_is_empty_but_flagged`.

## 3. Clippy (scoped to changed files, `-D warnings` semantics)
- `cargo clippy --bin onchainos` — my **new code introduces ZERO clippy warnings**
  (confirmed via `--message-format=short` filtered to the 4 changed files).
- The only warnings located in my changed files are **4 pre-existing `map_or(true, …)` lints**,
  reproduced on `master` (2 in each file, `grep -c 'map_or(true'` → 2/2):
  - `subscription_ops.rs:148,153` — `handle_start_autorenew` null-checks (NOT touched).
  - `create_subscribe.rs:158,163` — `providerConfirmStatus` / `typedData` null-checks (NOT touched;
    shifted down only by the added `build_create_body` / `build_create_success` helpers above them).
  - **Excluded as pre-existing debt (ONC-12); not "fixed" to avoid modifying unrelated code.**

## 4. Formatting (scoped)
- New file `device_routing.rs` → `rustfmt --edition 2021 --check` **clean (0 diffs)** (re-run after
  the R3/R5/R6 edits, which were `rustfmt`-formatted in place — the only file so treated).
- The edited pre-existing files (`audit.rs`, `subscription_ops.rs`, `create_subscribe.rs`) carry
  **pervasive pre-existing rustfmt debt on `master`**; running `cargo fmt` would rewrite hundreds of
  unrelated pre-existing lines — the whole-file debt ONC-12 excludes and primer §10 forbids
  (`never cargo fmt --all`). New edits in these files follow each file's established hand-formatting.

## 5. MCP guard
`grep -rncE "subscri|device-list|device_routing|batchUpdate|fetch_all_device" cli/src/mcp/mod.rs`
→ **0** (exit 1, no match). No MCP tool / `fetch_*` delegate added — grounded CLI-only exception
(A-MCPSPEC). No MCP surface added this rework either.

## 6. Scope guards (§6 / rework §3)
`git status --short` shows **no** working-tree change under `cli/src/device/`, `cli/src/mcp/`,
`Cargo.toml` / `Cargo.lock`, or `task-asp.md`. No heartbeat / offline-backfill path added.

## 7. CLI smoke (release binary `cli/target/release/onchainos`)

| Command | Output | Exit |
|---|---|---|
| `agent device-list --help` | lists `--page` / `--page-size` (no command-level `--chain`) | 0 |
| `agent subscribe-device-update --items '[]'` (0 items) | `{"ok":false,"error":"no subscriptions to update: provide --job-id or a non-empty --items array"}` — **local error, no request** | 1 |
| **`agent subscribe-device-update --items '[{"jobId":"","deviceList":["d1"]}]'`** (R6a) | `{"ok":false,"error":"--items entries must each carry a non-empty jobId"}` — **Form B empty jobId now rejected like Form A** | 1 |
| **`agent device-list` (DEGRADED PATH)** | `{"ok":false,"error":"session has expired; run onchainos wallet login first: …"}` — well-formed error envelope | 1 |
| `agent device-list --page-size 500` (`>100`) | same well-formed error envelope | 1 |

**Mandatory degraded device-list path (A-CLISPEC / A-ARCH §3.5/§7.3):** the `device-list` endpoint is
not live in production and this sandbox has no wallet session, so `device-list` fails with a
well-formed `{ok:false,error}` envelope + **exit 1** — the exact signal the skill's degraded render
consumes (never presenting this device as the full picture). The `create-subscribe` degrade / all-excluded
branch (`deviceRoutingDegraded:true`) is unit-tested via `resolve_create_device_set` (now 7 branch tests)
and the create request-body assertions (R2); an end-to-end create smoke is unreachable here without a
wallet session (create fails earlier at auth).

## 8. Explicit "NOT done" decisions (must appear in the MR description)
- **Post-login heartbeat** (A-ARCH §Open decision / PRD §7.3): **NOT implemented this run.** It touches
  the login path (regression risk); the required degrade-to-this-device branch + the mandatory
  "other devices' status temporarily unavailable" render already give correct, honest behavior.
- **Offline-message-backfill preference** (PRD §9): **out of scope — not implemented.** No backend
  endpoint exists to store/read this preference; it must not appear in any template.

## 9. Scoped commands run (audit trail)
```
git diff master...HEAD -- '*.rs' | grep '^+' | grep -iE 'WBW-[0-9]|AC-[0-9]|§[0-9]'   # empty (PASS)
rustfmt --edition 2021 --check cli/src/commands/agent_commerce/task/user/device_routing.rs   # clean
cargo build ; cargo build --release
cargo clippy --bin onchainos --message-format=short   # only 4 pre-existing map_or in changed files
cargo test --bin onchainos -- device_routing subscription_ops create_subscribe audit   # 90 ok
cargo test --bin onchainos                                                             # 1994 ok
grep -rncE "subscri|device-list|device_routing|batchUpdate|fetch_all_device" cli/src/mcp/mod.rs   # 0
```
