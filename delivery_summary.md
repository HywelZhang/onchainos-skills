# Delivery Summary — Multi-Device Login: Subscription-Message Device Routing (WBW-14118)

**Ticket:** [WBW-14118](https://okcoin.atlassian.net/browse/WBW-14118) · **Oli task:** `REQ-1785432592473-1e10f7`
**Branch:** `feat/wbw-14118-subscription-device-routing` → **target** `master`
**Base:** `master` @ `6a0b55fd4191c42388754ec21ed728df008e62d2`
**Review verdict:** PASS (unanimous, rework attempt 2) · all three lenses CLEAN, 0 required changes.

---

## What this delivers

Buyer-side subscription-message **device routing** for the `onchainos agent` command family, plus a
device-list read command and the `skills/okx-ai/` routing/rendering docs to match.

Two new CLI commands (`subscribe-device-update`, `device-list`), three enriched commands
(`create-subscribe`, `my-subscriptions`, `subscribe-detail` — always carry per-device routing +
`deviceRoutingDegraded`), audit labels + redaction (`--items`→FULL, `--job-id`→ADDR), and the
buyer-side `okx-ai` intent-routing / CLI-reference / playbook docs (incl. the **mandatory degraded
render** path, since the device-list endpoint is not yet live in production).

### Files changed (vs `master`)
```
 cli/src/audit.rs                                                |  52 +
 cli/src/commands/agent_commerce/mod.rs                          |  32 +-
 cli/src/commands/agent_commerce/task/user/create_subscribe.rs   | 149 +-
 cli/src/commands/agent_commerce/task/user/device_routing.rs     | 686 +  (NEW)
 cli/src/commands/agent_commerce/task/user/mod.rs                |  25 +-
 cli/src/commands/agent_commerce/task/user/subscription_ops.rs   | 148 +-
 skills/okx-ai/references/task-cli-reference.md                  |  43 +-
 skills/okx-ai/references/task-user-intent-routing.md            |   6 +-
 skills/okx-ai/references/task-user-playbook.md                  |  50 +-
 changes_summary.md / verification_report.md                     |  process docs
```
See `changes_summary.md` (full change inventory + grounded architecture drift) and
`verification_report.md` (authoritative scoped verification) — both committed on the branch.

---

## Scoped verification (ONC-12 substitution) — repeated here for reviewer audit

This private fork **drops the repo-wide `onchainos_check` deterministic gate** for the Verification
stage. That gate runs `rustfmt`/lints over **whole files** and recurses into sibling module trees, so
it attributes **pre-existing `master` debt** (legacy `println!`/CJK/provenance on untouched lines, 35
clippy warnings blamed to commits `7f7c98a30`/`3f04ba5b8`) to this change — **`master` itself does not
pass it.** The correctly-scoped equivalents below are authoritative; **nothing outside this change's
scope was modified to satisfy any check.**

Final Delivery-stage re-run (toolchain `cargo 1.97.1` / `rustc 1.97.1`), all against HEAD `65e75c77`:

| Check | Command (scoped) | Result |
|---|---|---|
| Build | `cargo build --release` | **exit 0** — `Finished release profile in 5m 00s`, binary `onchainos 4.4.3` |
| Clippy | `cargo clippy --release` | **exit 0** — 0 warnings on introduced code; all 35 warnings blamed to pre-branch commits on untouched lines (ONC-12 excluded, NOT "fixed") |
| Test | `cargo test --release --bin onchainos -- device_routing subscription_ops audit create_subscribe` | **exit 0** — **90 passed, 0 failed**, 0 ignored |
| Provenance gate | `git diff master...HEAD -- '*.rs' \| grep '^+' \| grep -iE 'WBW-[0-9]\|AC-[0-9]\|§[0-9]'` | **empty** — no provenance tokens in any added `.rs` line (Review blocker F1, closed) |
| Scope guards | `git diff --name-only master...HEAD` | clean — nothing under `cli/src/device/`, `cli/src/mcp/`, `cli/Cargo.toml`, `cli/Cargo.lock`, or `task-asp.md` |

MCP: no MCP surface added (grounded exception, A-MCPSPEC) — `cli/src/mcp/mod.rs` has zero
`subscri`/`device-list`/`batchUpdate`/`fetch_all_device` hits.

---

## Explicitly NOT done (in scope guard, by design)
- **Post-login heartbeat** — NOT this run (login-path regression risk; the degrade path covers the
  offline-device case).
- **Offline-message-backfill** — out of scope: the product wrote it as a requirement but **no backend
  endpoint exists** to store/read the preference. Deliberately absent from every command and template.
- **`cli/src/device/`** — untouched (device identity headers already settled on `master`).

---

## Delivery actions
1. Confirmed `review_verdict.json` = **PASS**.
2. Re-ran the scoped build / clippy / test on `cli/` — all green (table above).
3. Committed this delivery summary (provenance-stamped via commit-batch) and **pushed** the branch.
4. Opened a **DRAFT** standard GitLab MR (no MMR / mobile portal) into `master`, labelled
   `source::oli` + `oli::REQ-1785432592473-1e10f7`, assigned to the run triggerer.
