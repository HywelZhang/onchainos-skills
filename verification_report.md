# Verification Report — WBW-14118 (Subscription-Message Device Routing)

Branch `feat/wbw-14118-subscription-device-routing`, base `master` @ `6a0b55fd`.

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

## 1. Build
- `cargo build` (debug) → **exit 0**.
- `cargo build --release` → **exit 0**, binary `cli/target/release/onchainos`.

## 2. Tests
- Scoped: `cargo test --bin onchainos -- device_routing subscription_ops audit create_subscribe`
  → **77 passed, 0 failed**.
- Full regression: `cargo test --bin onchainos` → **1981 passed, 0 failed** (proves the
  `--job-id`→REDACT_ADDR change introduced no crate-wide regression).
- New tests: device_routing (AC-01 body/boundaries 0/1/100/101, AC-06 ms-vs-seconds RED + 3 envelope
  shapes, AC-02 create device-set 3 branches), subscription_ops (AC-03 tri-state, AC-04 membership),
  audit (`--items`/`--job-id` redacted, `--device-list`/`--page`/`--page-size` visible).

## 3. Clippy (scoped to changed files, `-D warnings`)
- `cargo clippy --bin onchainos` — **new code introduces ZERO clippy warnings**.
- 4 warnings appear in my changed files but are **pre-existing on `master`** (reproduced):
  - `subscription_ops.rs:148,153` — `map_or(true, |o| o.is_empty())` in `handle_start_autorenew`
    (identical on `master:...subscription_ops.rs:148,153`; not touched by this change).
  - `create_subscribe.rs:116,121` — same lint in the `providerConfirmStatus` null-check
    (on `master:...create_subscribe.rs:114,119`; shifted +2 only by the added `--exclude-device`
    field above; not touched by this change).
  - **Excluded as pre-existing debt (ONC-12); not "fixed" to avoid modifying unrelated code.**

## 4. Formatting (scoped)
- New file `device_routing.rs` → `rustfmt --edition 2021 --check` **clean (0 diffs)**.
- The five edited pre-existing files carry **pervasive pre-existing rustfmt debt on `master`**
  (isolated per-file counts on `master`: `audit.rs` 2, `subscription_ops.rs` 24,
  `create_subscribe.rs` 18 diff hunks). Running `cargo fmt` would rewrite hundreds of unrelated
  pre-existing lines — precisely the whole-file debt ONC-12 excludes and the primer §10 forbids
  (`never cargo fmt --all`). Edits to these files therefore follow each file's established
  hand-formatting for consistency; the non-compliance is pre-existing and excluded.

## 5. MCP guard (T9)
`grep -rncE "subscri|device-list|device_routing|batchUpdate|fetch_all_device" cli/src/mcp/mod.rs`
→ **0** (exit 1, no match). No MCP tool/`fetch_*` delegate added — grounded exception (A-MCPSPEC).

## 6. CLI smoke (release binary `cli/target/release/onchainos`, `onchainos 4.4.3`)

Every invocation returns a well-formed `output::*` JSON envelope.

| Command | Output | Exit |
|---|---|---|
| `agent subscribe-device-update --help` | lists `--job-id` / `--device-list` / `--items` (no command-level `--chain`) | 0 |
| `agent device-list --help` | lists `--page` (default 1) / `--page-size` (default 20) | 0 |
| `agent create-subscribe --help` | now lists `--exclude-device <EXCLUDE_DEVICE>` | 0 |
| `agent subscribe-device-update --items '[]'` (0 items) | `{"ok":false,"error":"no subscriptions to update: provide --job-id or a non-empty --items array"}` — **local error, no request** | 1 |
| `agent subscribe-device-update --items '<101 items>'` | `{"ok":false,"error":"too many items (101); at most 100 subscriptions per batch"}` — **local error, no request** | 1 |
| `agent subscribe-device-update --job-id 0xJOB --device-list d1` (valid) | passes client validation, proceeds to auth → `{"ok":false,"error":"session has expired; run onchainos wallet login first: …"}` | 1 |
| **`agent device-list` (DEGRADED PATH)** | `{"ok":false,"error":"session has expired; run onchainos wallet login first: …"}` | 1 |
| `agent device-list --page 2 --page-size 50` | same well-formed error envelope | 1 |
| `agent device-list --page-size 500` (`>100`) | same well-formed error envelope | 1 |

**Mandatory degraded device-list path (A-CLISPEC / A-ARCH §3.5/§7.3):** the `device-list` endpoint is
not live in production, and this sandbox has no wallet session, so `device-list` fails and returns a
well-formed `{ok:false,error}` envelope with **exit 1** — the exact signal the skill's degraded render
consumes ("其他设备接收状态暂不可用 / device info temporarily unavailable", never presenting this
device as the full picture). In this sandbox the degrade manifests at the auth boundary (no session);
against production with a live session it manifests identically at the not-yet-live `/device-list`
endpoint — same envelope shape + exit code, same degraded render. The 0/101-item validation boundaries
fail locally with **no request sent** (AC-01). The `create-subscribe` degrade branch
(`deviceRoutingDegraded:true`, this-device only, no abort) is unit-tested by
`resolve_create_device_set` (5 tests) — an end-to-end create smoke is not reachable here without a
wallet session (create fails earlier at auth).

## 7. Explicit "NOT done" decisions (must appear in the MR description)
- **Post-login heartbeat** (A-ARCH §Open decision / PRD §7.3): **NOT implemented this run.** It touches
  the login path (regression risk for a listing convenience); the required degrade-to-this-device
  branch (§4.4) + the mandatory "other devices' status temporarily unavailable" render (§4.3) already
  give correct, honest behavior when a device is not yet listable.
- **Offline-message-backfill preference** (PRD / A-ARCH §10): **out of scope — not implemented.** No
  backend endpoint exists to store/read this preference; it must not appear in any template.

## 8. Scoped commands run (audit trail)
```
git diff --name-only master...HEAD -- '*.rs'
rustfmt --edition 2021 --check cli/src/commands/agent_commerce/task/user/device_routing.rs   # clean
cargo build ; cargo build --release
cargo clippy --bin onchainos
cargo test --bin onchainos -- device_routing subscription_ops audit create_subscribe          # 77 ok
cargo test --bin onchainos                                                                     # 1981 ok
grep -rncE "subscri|device-list|device_routing|batchUpdate" cli/src/mcp/mod.rs                 # 0
```
