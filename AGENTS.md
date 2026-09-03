# AGENTS.md

This repository is the **optimized fork** of OKX's onchainos-skills (agent commerce /
OKX.AI task & subscription domain). Working branch: `dev` (main tracks upstream only).

If you are an agent running on a buyer machine that already has OKX.AI installed
(onchainos CLI + okx-a2a runtime + okx-* skills) and the user asks you to upgrade to
this fork: follow `docs/UPGRADE.md` end-to-end. It is written to be executed by an
agent: locate the existing okx skills tree by content signature, back it up, swap the
whole tree, verify, and (optionally) enable the deterministic engine scripts.
Runtimes covered: Hermes, Claude Code, OpenClaw, Codex CLI (`okx-a2a config provider
--provider <hermes|claude|openclaw|codex>` binds the runtime).

If the buyer has NO OKX.AI installed yet and wants to start directly on this fork:
follow `docs/INSTALL.md` (greenfield: CLIs → runtime binding → skills → identity).

If you are here to develop: read `docs/optimization-plan.md` (state + backlog) and the
design docs under `docs/design/` (01 node inventory … 10 buyer subscription handbook).
Deterministic engine scripts live in `scripts/` with selftests; example per-subscription
policies live in `examples/policy/`.

Money/chain actions in this repo's scripts are gated behind `--dryrun` (default) and
`--live` (explicit confirmation). Keep that default. Never sign or move funds without
the user's explicit go-ahead.
