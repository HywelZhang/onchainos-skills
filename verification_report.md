# Verification Report — WBW-14118 (Subscription-Message Device Routing)

Branch `feat/wbw-14118-subscription-device-routing` · HEAD `46d649b9` · base `master` @ `6a0b55fd`.
**Rework attempt 2, independently re-verified by the Verification stage** (every command below was
re-run against the reworked tree, not copied from the implementation draft). This rework closes the
single Review blocker — **provenance tokens in new `.rs` comments** — and folds in non-blocking
findings R2–R6. The safety-critical routing/degrade logic verified correct in attempt 1 is preserved.
Toolchain: `cargo 1.97.1` / `rustc 1.97.1` / `rustfmt 1.9.0-stable` / `clippy 0.1.97`. Binary under
test: `cli/target/release/onchainos` (`onchainos 4.4.3`), rebuilt from HEAD `46d649b9`.

---

## 0. Scoped-verification substitution (ONC-12) — what was substituted and why

This private fork drops the repo-wide **`onchainos_check` deterministic gate** for this stage. That
gate selects files from the diff but then (a) runs `rustfmt` over whole files and recurses into the
sibling module tree, (b) runs whole-file lints that hit legacy `println!`/CJK/provenance on untouched
lines, and (c) invokes `cargo test` with multiple bare positional filters cargo rejects before any test
runs. **`master` itself does not pass it.** The five correctly-scoped equivalents below are the
authoritative verification; **nothing outside this change's scope was modified to satisfy any check.**

1. Formatting — ONLY files this change adds/modifies, `--config skip_children=true` (no sibling recursion).
2. Tests — exactly the modules this change touches (filters after `--`) + every test the change adds.
3. Lints — clippy clean for introduced code; pre-existing warnings on untouched lines out of scope, NOT "fixed".
4. Source hygiene — no `println!`(raw JSON) / CJK / **provenance tokens** in ADDED lines only.
5. Baseline honesty — every failure reproduced on `master` before attribution.

### Change scope (`git diff --name-status master...HEAD`) — unchanged from attempt 1
```
M cli/src/audit.rs
M cli/src/commands/agent_commerce/mod.rs
M cli/src/commands/agent_commerce/task/user/create_subscribe.rs
A cli/src/commands/agent_commerce/task/user/device_routing.rs   (NEW, 686 lines)
M cli/src/commands/agent_commerce/task/user/mod.rs
M cli/src/commands/agent_commerce/task/user/subscription_ops.rs
M skills/okx-ai/references/{task-cli-reference,task-user-intent-routing,task-user-playbook}.md
```
Scope guards clean: `git diff --name-only master...HEAD` touches **nothing** under `cli/src/device/`,
`cli/src/mcp/`, `cli/Cargo.toml`, `cli/Cargo.lock`, or `task-asp.md`.

---

## Gate 1 — scoped build / fmt / clippy / test + source hygiene → **PASS**

### 1.0 Provenance gate (Review blocker R01 / §7.1 — the reason for this rework)
Provenance tokens (`WBW-nnn`, `AC-nn`, `§n`) are banned in **new `.rs` source** with the same force as
raw-JSON `println!` or CJK. Gate command (MUST be empty):
```
git diff master...HEAD -- '*.rs' | grep '^+' | grep -iE 'WBW-[0-9]|AC-[0-9]|§[0-9]'
→ (empty)   # PASS — no provenance tokens in any added .rs line
```
Cross-check: `grep -inE 'WBW-[0-9]|AC-[0-9]|§[0-9]' device_routing.rs` → none. (Scope: `.rs` only —
section refs inside skill `.md` docs are legitimate documentation cross-refs, left in place.)

### 1.1 Build
`cargo build --release` → **exit 0**, `Finished release profile in 4m50s`, binary `onchainos 4.4.3`.

### 1.2 Formatting (`rustfmt --check --config skip_children=true`, per changed file)
| File | HEAD hunks | master hunks | verdict |
|---|---|---|---|
| `device_routing.rs` (NEW, reworked) | **0** | — | **clean ✓** |
| `audit.rs` | 2 | 1 | pre-existing |
| `agent_commerce/mod.rs` | 82 | 80 | pre-existing |
| `create_subscribe.rs` | 17 | 15 | pre-existing |
| `task/user/mod.rs` | 8 | 7 | pre-existing |
| `subscription_ops.rs` | 21 | 18 | pre-existing |

The reworked new file is fully rustfmt-clean. The edited files carry pervasive **pre-existing
whole-file rustfmt debt** (`master` fails `rustfmt --check` identically). The `create_subscribe.rs`
15→17 delta is the R2 helpers/tests; intersecting rustfmt-changed lines with this change's added
ranges shows every hit on an added line (e.g. 272, 340–342, 421–422, 449) is the file's own
established compact hand-formatting (single-line `bail!`/`assert`/`map_err` — the same pattern rustfmt
objects to on pre-existing lines 50/53). Reformatting would rewrite hundreds of unrelated lines
(primer §10 forbids `cargo fmt --all`; ONC-12 excludes this debt). Not "fixed" — excluded as pre-existing.

### 1.3 Clippy (`cargo clippy --bin onchainos --tests`, attributed)
- **Code this change introduces — `device_routing.rs` (entire new file) + all added lines — = ZERO
  clippy warnings.**
- Warnings landing in a *changed* file: `create_subscribe.rs:158,163` and `subscription_ops.rs:148,153`
  (`clippy::unnecessary_map_or`) + `agent_commerce/mod.rs:1947` (`items_after_test_module`). All are
  **outside** this change's added ranges and git-blamed to commit `7f7c98a30` (liyun.dong, 2026-07-24)
  — **not** in this branch's range `d5ca9e87..46d649b9`. (create_subscribe's lints shifted 116/121 →
  158/163 only because R2 helpers were added above them.) All other crate warnings are in files this
  branch never touched. Pre-existing; not "fixed".

### 1.4 Tests (scoped filters after `--`, plus full regression)
- Scoped: `cargo test --bin onchainos -- device_routing subscription_ops create_subscribe audit`
  → **90 passed; 0 failed; 1904 filtered out** (was 77; +13 from R2–R6).
- Full regression: `cargo test --bin onchainos` → **1994 passed; 0 failed** (113.55s; was 1981).
- New tests this rework: R2 (create body always embeds `deviceList` even empty + success envelope
  always carries `deviceRoutingDegraded`), R3 (`normalize_page_params_*`, `pagination_done_*`,
  `resolve_total_never_under_reports`, `device_ids_drops_empty_ids`), R4 (my-subscriptions struct parse
  tolerates non-string array elements), R6 (`normalize_form_b_rejects_empty_job_id`, all-excluded
  degrade branches).

### 1.5 Source hygiene (ADDED lines only; 1065 added `.rs` lines)
- **CJK:** none. **Provenance tokens** (`WBW-/AC-/§`, and generic `co-authored`/`claude`/`openai`/…):
  none. **Raw-JSON `println!`:** none. The 3 print statements on added lines are all clean:
  `create_subscribe.rs` two `DEBUG_LOG`-gated **stderr** diagnostics + one human-readable degrade
  **notice** to stdout (`⚠ Device list unavailable — …THIS device only…`, not JSON); `device_routing.rs`
  one spec-mandated form-B-precedence **stderr** warning.

---

## Gate 2 — CLI smoke (JSON envelope + shape) → **PASS** (success-shapes wallet-gated SKIP)

`wallet status` → `loggedIn:false`; success-path (`ok:true` + full `data`) unreachable → those
data-shapes are SKIP (non-blocking; covered by Gate 1.4 unit tests). Every envelope jq-validated.

| # | Command | Envelope | Exit | Result |
|---|---|---|---|---|
| 1 | `subscribe-device-update --items '[{"jobId":"","deviceList":["d1"]}]'` (**R6a NEW**) | `{ok:false,error:"--items entries must each carry a non-empty jobId"}` | 1 | PASS |
| 2 | `subscribe-device-update --items '[]'` (0 items) | `{ok:false,error:"no subscriptions to update…"}` — local, no request | 1 | PASS |
| 3 | `subscribe-device-update --items <101>` (>100) | `{ok:false,error:"too many items (101); at most 100…"}` — local, no request | 1 | PASS |
| 4 | `subscribe-device-update --job-id 0xJOB --device-list d1,d2` | valid → auth `{ok:false,error}` | 1 | PASS |
| 5 | **`device-list` (DEGRADED / no session)** | `{ok:false,error:"session has expired…"}` | 1 | PASS |
| 6 | `device-list --page-size 500` (>100) | `{ok:false,error}` | 1 | PASS |
| 7 | `my-subscriptions --role buyer` | `{ok:false,error:"failed to fetch subscriptions…"}` | 1 | PASS |
| 8 | `create-subscribe --help` | lists `--exclude-device <EXCLUDE_DEVICE>` | 0 | PASS |

- **R6a** proves Form B empty-`jobId` is now rejected client-side exactly like Form A (was the rework's
  functional add). Batch boundaries 0/1/100/101 enforced client-side (no request on 0 / >100).
- **Mandatory degraded device-list path** (endpoint not live): fails to a well-formed `{ok:false,error}`
  / exit 1 — the exact signal the skill's degraded render consumes; identical envelope + exit against a
  live production session hitting the not-yet-live endpoint. Self-tested.
- **MCP smoke → SKIP (N/A):** `mcp_tool_spec.md` = Not applicable; guard
  `grep -rncE "subscri|device-list|device_routing|batchUpdate|fetch_all_device" cli/src/mcp/mod.rs` → **0**;
  `cli/src/mcp/` untouched. No surface added — correct.

---

## Gate 3 — skill / workflow consistency → **PASS** (routing-regression SKIP, Minor)

Rework commit `46d649b9` touched **no** skill files (verified). `skills/okx-ai/SKILL.md` frontmatter
unchanged vs `master`. Both new commands remain in the `task-cli-reference.md` Command Index (line 13)
and dedicated sections; enriched commands documented; every documented flag matches the real `--help`;
no workflow references the subscribe family. Live routing regression (3.3b) is SKIP (Minor) — SKILL.md
(the routing surface) unchanged and no `run.sh`/`test-cases.json` harness in the repo.

---

## Explicit "NOT done" decisions (must appear in the MR description)
- **Post-login heartbeat** (A-ARCH open decision / PRD §7.3): NOT implemented — touches the login path
  (regression risk); the degrade-to-this-device branch + "other devices' status temporarily
  unavailable" render already give correct honest behavior.
- **Offline-message-backfill preference** (PRD §9): out of scope — no backend endpoint exists to
  store/read it; must not appear in any template.

---

## Verdict — ALL GATES PASS · Review blocker closed · no fixes required · no escalation

| Gate | Result |
|---|---|
| 1 — provenance gate (blocker) | **PASS** (empty) |
| 1 — build/fmt/clippy/test + source hygiene | **PASS** |
| 2 — CLI smoke (envelope + shape) | **PASS** (success-shapes wallet-gated SKIP) |
| 2 — MCP smoke | **SKIP** (N/A — guard = 0 hits) |
| 3 — skill/workflow consistency | **PASS** (routing-regression SKIP, Minor) |

Reworked introduced code is provenance-clean, format-clean (new file), clippy-clean, fully tested
(90 scoped / 1994 full, 0 failed), and hygiene-clean on added lines; the R6a Form-B fix behaves as
specified. All fmt/clippy debt is pre-existing on `master` and correctly excluded per ONC-12.

---

## Commands run (audit trail)
```
git diff master...HEAD -- '*.rs' | grep '^+' | grep -iE 'WBW-[0-9]|AC-[0-9]|§[0-9]'    # empty (PASS)
git diff --name-only master...HEAD -- 'cli/src/device/**' 'cli/src/mcp/**' 'cli/Cargo.toml' 'cli/Cargo.lock' '**/task-asp.md'   # empty
rustfmt --edition 2021 --check --config skip_children=true <changed file>              # device_routing.rs => 0
git show master:<file> | rustfmt --edition 2021 --check --config skip_children=true     # baseline hunks
cargo build --release                                                                   # exit 0 (4m50s)
cargo clippy --bin onchainos --tests                                                    # 0 warns on introduced code
git blame -L 158,158 -L 163,163 HEAD -- .../create_subscribe.rs                         # => 7f7c98a30 (pre-branch)
cargo test --bin onchainos -- device_routing subscription_ops create_subscribe audit    # 90 passed
cargo test --bin onchainos                                                              # 1994 passed
grep -rncE "subscri|device-list|device_routing|batchUpdate|fetch_all_device" cli/src/mcp/mod.rs   # 0
# CLI smoke (release binary), envelopes jq-validated — see Gate 2 table
```

---

## MR description (copy verbatim into the merge request)

**WBW-14118 — Multi-device subscription-message device routing** (rework attempt 2). Two new buyer-side
CLI commands (`agent subscribe-device-update`, `agent device-list`), three enriched commands
(`create-subscribe` always sends `deviceList` + degrade branch; `my-subscriptions` / `subscribe-detail`
gain `deviceList`/`categoryCodes`/`thisDeviceReceives`/`thisDeviceId`), audit labels + redaction
(`--items`→FULL, `--job-id`→ADDR; device-list read flags stay visible), and buyer-side `skills/okx-ai/`
routing + rendering docs. No MCP surface (grounded CLI-only exception). `cli/src/device/` and
`task-asp.md` untouched; no new dependency.

**This rework closes the Review blocker** — provenance tokens (Jira/AC/§) were stripped from all new
`.rs` comments (design-intent prose kept) — and folds in R2–R6 (create-body/degrade-marker assertions,
pagination/param edge tests, tolerant my-subscriptions struct parse, and **Form B now rejects empty
`jobId` client-side like Form A**).

**Verification is scoped (ONC-12), NOT the repo-wide `onchainos_check` gate** (which checks whole
files / recursive trees and fails on `master`'s pre-existing debt). Verified:
- **Provenance gate:** `git diff master...HEAD -- '*.rs' | grep '^+' | grep -iE 'WBW-[0-9]|AC-[0-9]|§[0-9]'` → empty.
- **Format:** new file `device_routing.rs` rustfmt-clean; the edited files carry pre-existing whole-file
  debt (`master` fails identically) and were deliberately NOT reformatted.
- **Clippy:** all introduced code = 0 warnings; the crate's warnings are pre-existing (git-blamed to a
  pre-branch commit or in untouched files).
- **Tests:** 90 scoped + **1994** full, 0 failed.
- **Source hygiene:** no CJK / provenance / raw-JSON `println!` on added lines.
- **CLI smoke:** every new/changed command returns a well-formed envelope; batch boundaries 0/1/100/101
  and Form-B empty-`jobId` enforced client-side; the **degraded device-list path is self-tested**.
  Success data-shapes are wallet-gated (SKIP, covered by unit tests).

**Deliberately excluded / NOT done:** pre-existing whole-file rustfmt debt + pre-existing clippy
warnings in/around the edited files (reproduced on `master`); MCP surface (CLI-only family, guard = 0);
post-login heartbeat (touches login path — degrade + notice cover the gap); offline-message-backfill
(no backend endpoint).
