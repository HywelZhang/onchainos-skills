# Catalog Probe — design rationale & fallbacks

Maintainer-oriented background for `SKILL.md` §6 (catalog probe for DApps **not** in the Plugin Resolver Table). The operational command lives in §6; this file explains *why* it's shaped that way and documents the fallbacks. Reading this file is **not** required to route correctly.

---

## Why the prefix match (not exact `<name>-plugin`)

The plugin-store catalog uses inconsistent suffix conventions:

- Most plugins: `<name>-plugin` (e.g. `raydium-plugin`, `aave-v3-plugin`)
- Some: `<name>-ai` (e.g. `uniswap-ai`)
- Some: `<name>-v2-plugin` (e.g. `velodrome-v2-plugin`)
- Some: bare names (e.g. `meme-trench-scanner`, `top-rank-tokens-sniper`)

A strict `${DAPP_LOWER}-plugin` exact match would miss `uniswap-ai` and `velodrome-v2-plugin`. Prefix-matching `^${DAPP_LOWER}(-|$)` against the live catalog catches all suffix conventions automatically — no need to update this skill every time a new plugin lands with a different naming style.

## Why the GitHub Contents API (not `npx skills add --list`)

`npx skills` has no `info` / `search` / `exists` subcommand today. The only catalog enumeration verb is `add --list`, which clones the whole repo and prints all entries — slow and over-broad. The GitHub Contents API gives a deterministic, ~0.1s "exists or not" check directly (~25× faster than the clone-and-install probe). The fallback to `npx skills add` preserves correctness when the API is unreachable.

## `jq` fallback (no `python3` on PATH)

§6's probe parses the Contents API with `python3`. Python 3 ships by default on macOS 10.15+, all common Linux distros, and Windows-Git-Bash-with-Python. If `python3` is missing, substitute `jq`:

```bash
CATALOG=$(curl -fsSL --max-time 5 "https://api.github.com/repos/okx/plugin-store/contents/skills" 2>/dev/null \
          | jq -r '.[].name' 2>/dev/null)
```

If neither `python3` nor `jq` is available, fall through to the `npx skills add … --yes --global` clone-and-install fallback automatically.

## Known limitations

- **Claude-Code-specific path.** The Read step in §4 uses `$HOME/.claude/skills/`. Codex / OpenCode / OpenClaw / Cursor users may need to substitute their agent's skills directory. Tracked as a follow-up against the `skills` CLI to add a `skills info <skill>` subcommand for cross-agent path resolution.
- **`2>/dev/null` silences stderr** (intentional — avoids noise across agent runtimes). If `npx` itself is broken or missing, the listing returns empty and every DApp is treated as "not installed". The fallback `npx skills add … --yes --global` path is idempotent and surfaces the underlying error via §4's failure-mode note — do not retry the listing in a loop.
