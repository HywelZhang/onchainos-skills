# INSTALL — greenfield: buyer has NO OKX.AI yet, install straight onto this fork

> Read this with your code-reading ability and EXECUTE. You are setting up an OKX.AI
> buyer client from scratch, using this optimized fork as the ONLY skill/documentation
> source (no need to fetch official okx skills separately — the fork contains them).
> Runtime options (pick the one the user runs): `hermes` | `claude` (Claude Code) |
> `openclaw` | `codex`. Every chain/money step below needs the USER's explicit go-ahead.

## Phase 0 — environment (abort + report if these fail)

- Node.js >= 20 (node --version) and Python >= 3.10 (python --version)
- git or curl available

## Phase 1 — install the CLIs (one-time, machine-wide)

    npm install -g @okxweb3/a2a-node      # provides `okx-a2a` (also installs/updates `onchainos` agent CLI if packaged)
    okx-a2a --version
    onchainos --version
    # if `onchainos` is not provided by that package, install it explicitly per the
    # official channel documented in okx-fork/skills/okx-ai/references/identity-register.md
    # (it is a Rust/npm CLI; do not guess a package name — read that reference first)

## Phase 2 — okx-a2a runtime + AI provider binding

    okx-a2a doctor --fix          # daemon start, agent refresh, runtime detection
    okx-a2a config provider --provider <hermes|claude|openclaw|codex>   # pick user's runtime
    okx-a2a doctor                # expect ready:true (report anything that needs user action)

## Phase 3 — skills: install the fork as the okx-* tree

    git clone -b dev https://github.com/HywelZhang/onchainos-skills.git okx-fork
    cd okx-fork
    mkdir -p $SKILLS_ROOT          # $SKILLS_ROOT per your runtime, e.g.
                                  # hermes:  ~/.hermes/skills | %LOCALAPPDATA%\hermes\skills
                                  # claude:  ~/.claude/skills | <project>/.claude/skills
                                  # openclaw: <its skills dir> | codex: ~/.codex/skills (if any)
    cp -r skills/. $SKILLS_ROOT/  # copies okx-ai, okx-agentic-wallet, … okx-guide wholesale

## Phase 4 — identity & wallet (user's own account; read the fork's own instructions)

    # Follow okx-fork/skills/okx-ai/references/identity-register.lite.md for the user role:
    # 1) onchainos login (email/OTP)       2) create/activate the user agent (role user)
    # 3) fund the agent wallet with X Layer USDT (subscriptions/tasks need balance)
    # Do NOT skip the register flow's QA steps; do NOT register an ASP unless asked.

## Phase 5 — verify

    python okx-fork/scripts/policy-engine.py --selftest     # 8/8
    python okx-fork/scripts/decision-loop.py --selftest     # PASS
    python okx-fork/scripts/sub-sim.py                      # 7/7
    # then one live probe in your runtime: "list my subscriptions and my agents"
    # expect both lookups to succeed with no skill-load errors.

## Phase 6 (recommended) — deterministic engine & subscription defaults

    # copy engine + policy examples next to the skills (e.g. ~/okx-flow/):
    cp -r okx-fork/scripts okx-fork/examples okx-fork/docs ~/okx-flow/
    # read docs/design/10-buyer-subscription.md (buyer handbook) before first subscription.
    # Money-bearing steps default to ask; `--live` only after explicit user confirmation.

## Safety rules (non-negotiable)

- Never touch other machines' keys, never sign without explicit confirmation.
- Default `ask` for funds; `auto` only with user-configured limits/consent.
- Any step failing or environment differing from assumptions → STOP and report, don't improvise.
