# UPGRADE — for buyers who already run an OKX.AI agent

> Read this file with your code-reading ability and EXECUTE the steps below.
> You are upgrading an existing OKX.AI (onchainos / okx-a2a) agent installation to
> this optimized fork. No re-install of wallets, identities, or runtimes is needed.

## What this fork changes

1. `skills/okx-*` — optimized reference docs (lite cards, thinner SKILL.md, i18n labels,
   deterministic routing help). Same CLI commands, same protocol; strictly better token/cost
   behavior and less hallucination.
2. `scripts/` + `examples/policy/` — OPTIONAL deterministic engine (watch-host, policy
   engine, decision loop, executor, signal envelope, sub-collect). Platform-neutral Python
   driving the same CLIs with 0 LLM tokens for mechanical steps.

## Prereqs (check first — abort and tell the user if any fail)

- `onchainos --version` works (agent commerce CLI installed)
- `okx-a2a doctor` reports ready (`--fix` if it lists fixable items; needs no wallet action)
- The user's identity/wallet is already set up — DO NOT re-register anything
- Git or curl available to fetch this repo

## Step 1 — fetch this repo (dev branch carries the optimization; main only tracks upstream)

    git clone -b dev https://github.com/HywelZhang/onchainos-skills.git okx-fork
    cd okx-fork
    # (or download+unzip the dev branch archive; then cd into it)
    # The fork's skills tree = okx-fork/skills/  containing the okx-* member dirs:
    #   okx-ai, okx-agentic-wallet, okx-agent-payments-protocol, okx-dapp-discovery,
    #   okx-defi, okx-dex-market, okx-guide

## Step 2 — locate the existing okx skills tree (self-locating, works on any platform)

Do NOT guess paths. Find the tree by its content signature:

    # look for directories containing okx-ai/SKILL.md or okx-agentic-wallet/_shared/preflight.md
    find ~ -maxdepth 6 -type d \( -name okx-ai -o -name okx-agentic-wallet \) 2>/dev/null | head

Let `P = parent-of-those-dirs` (the skills root whose members are the okx-* family).
Typical roots (verify by listing, don't assume):
- Hermes: `~/.hermes/skills/` or Windows `%LOCALAPPDATA%\hermes\skills\` (its `okx/` member dir)
- Claude Code: `~/.claude/skills/` or project `.claude/skills/`
- OpenClaw: its skills/plugins directory (locate via the find above)
- Codex CLI: its skills mount (often `~/.codex/skills/` or project-local; verify with Step-2 find).
  If your Codex has NO skills directory at all: still do Steps 3-4 into the nearest
  okx-* skills root you found, OR (no okx tree present) skip to the greenfield path in
  `docs/INSTALL.md`; afterwards tell your user-agent to read `okx-fork/skills/okx-ai/`
  for okx.ai-domain instructions on demand (the fork's reference docs work fine loaded
  as plain context — that is the whole point of the lite cards).

If you find MULTIPLE candidate roots, pick the one actively loaded by your runtime
(your platform's loaded-skills list); leave others untouched.

## Step 3 — backup (mandatory, enables rollback)

Back up every `okx-*` member dir under P:

    mkdir -p $P/okx-backup-$(date +%Y%m%d)
    for d in $P/okx-*; do [ -d "$d" ] && mv "$d" $P/okx-backup-$(date +%Y%m%d)/ 2>/dev/null; done
    # NOTE: do this once per session; use a fixed date var if your shell doesn't repeat it.
    # Simpler portable variant:  cp -r $P/okx-ai $P/okx-backup-<date>/  (repeat per member),
    # or on Windows native PowerShell:  Copy-Item -Path $P\okx-* -Destination $P\okx-backup-<date> -Recurse

## Step 4 — swap the whole okx-* family (copy the DIRECTORY TREE, never per-file,
## never "install" commands that flatten subdirs — `_shared/` must arrive intact)

    rm -rf $P/okx-ai $P/okx-agentic-wallet $P/okx-agent-payments-protocol \
           $P/okx-dapp-discovery $P/okx-defi $P/okx-dex-market $P/okx-guide
    cp -r okx-fork/skills/. $P/
    # git-bash/cp note: trailing '/.' copies directory CONTENTS — exactly the 7 members.

Verify `_shared` survived and the fork marker exists:

    ls $P/okx-ai/SKILL.md
    grep -c "FORK" $P/okx-ai/SKILL.md          # expect >= 1
    ls $P/okx-agentic-wallet/_shared/preflight.md
    ls $P/okx-ai/references/task-subscription-signal.lite.md

## Step 5 — verify the upgrade is live (per your platform)

- List your loaded skills and confirm `okx-ai` now points to the fork (same name, new content).
- Run one cheap real probe in a fresh session, e.g. ask your agent:
  "list my subscriptions and my agents" — expect it to load `task-user-playbook.lite.md`
  (smaller file) and succeed on both lookups.
- Confirm NO errors like "skill not found" or missing `_shared` references.

## Step 6 (OPTIONAL) — deterministic engine

The engine is independent of your runtime and runs alongside your agent:

    cd okx-fork
    python scripts/policy-engine.py --selftest        # 8/8 expected
    python scripts/decision-loop.py --selftest        # PASS expected
    python scripts/watch-host.py --selftest           # PASS expected
    python scripts/executor-lite.py --selftest        # PASS expected
    python scripts/signal-envelope.py --selftest      # 9/9 expected
    python scripts/sub-sim.py                         # 7/7 expected

To operate it: read `docs/design/10-buyer-subscription.md` (buyer subscription handbook)
and `docs/optimization-plan.md`. Every money/chain action is behind explicit flags
(`--dryrun` default; `--live` only after the user confirms) — keep that default.
Policies live in `examples/policy/` (JSON, one file per subscription scope); the agent
may write new ones for the user on request, never auto-execute funds without
`ask`-mode confirmation unless limits+consent exist.

## Safety rules (non-negotiable)

- Never touch wallet keys, never create/re-register identities, never sign anything
  without the user's explicit confirmation.
- Money-bearing steps: default ask; `auto` only when the user configured limits/consent.
- If a step above fails or the environment differs from what the doc assumes, STOP and
  report the discrepancy to the user instead of improvising a workaround.

## Rollback (if the user reports anything broken)

    rm -rf $P/okx-ai $P/okx-agentic-wallet $P/okx-agent-payments-protocol \
           $P/okx-dapp-discovery $P/okx-defi $P/okx-dex-market $P/okx-guide
    mv $P/okx-backup-<date>/* $P/   # restore every backed-up member
    # then re-verify with the Step-5 probe before telling the user it's fixed

## Version note

The fork's skills are written against current onchainos CLI surfaces (v4.5.x-era:
`service-match`, `create-subscribe`, `my-subscriptions`, `pending-decisions-v2` …).
If a command in a reference doc is missing from the installed CLI, run
`okx-a2a update` and/or update `onchainos` (npm) first, then retry.
