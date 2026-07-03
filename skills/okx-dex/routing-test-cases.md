# okx-dex — Routing Test Cases (merge trial)

> **Rename note (2026-07-02):** the merged skill was initially named `okx-dex-data`; all `okx-dex-data` mentions below are historical references to what is now **`okx-dex`**. After the rename, a smoke re-run confirmed routing still holds (see the smoke-test note at the end of the update block below).

> **2026-07-02 update — validated against the real harness.** The 150 relevant cases in `onchianos-skills-testing` (`test-cases.json`, old names remapped to `okx-dex-data`; `run.sh` `TARGET_OKX` whitelist updated) were run with real `claude` subprocesses (sonnet, max-turns 3). First pass: 144/150. The 3 real failures ("my DEX trade history" ×3 misrouting to `okx-agentic-wallet`; "rugged before?" not firing) were compression regressions — the trigger words `my DEX trade history/交易记录` and `rug history` had been cut from the merged description. Restoring them (description now 1020/1024 chars) fixed 5 of 6 on re-run; the 6th (T249) expects "`okx-dex-data` or none" and the harness cannot express mixed expectations, so it is a pass in substance. **Net: 150/150.** A parallel 16-case `defi` category (T281–T296) validated the `okx-defi` merge at 16/16, including Aave/Lido hard-blocks to `okx-dapp-discovery` and boundaries vs `okx-wallet-portfolio`/`okx-dex-data`/`okx-dex-swap`. Lesson recorded: description compression must preserve per-capability trigger phrases verbatim, not just capability nouns — the two regressions were both nouns kept / trigger phrases dropped.
>
> **Post-rename smoke test (`okx-dex-data` → `okx-dex`)**: 9 cases re-run — the two swap-boundary cases (T01 "Swap 1 ETH for USDC", T296 "Swap … at the best rate") still route to `okx-dex-swap` (the shorter name did not start absorbing swap intent), and one case per capability (price, PnL-history, signal, trenches-rug, index, leaderboard, compound) all still route to `okx-dex`. 9/9 correct.
>
> **Tombstone smoke test**: per user decision the 8 original skill directories are RETAINED as deprecated tombstones (original body frozen under a redirect banner; `description` replaced with a trigger-word-free deprecation notice; `metadata.deprecated: true`). With all 28 directories present in the routing pool, a 10-case smoke (each capability + defi invest/portfolio + both swap boundaries) came back 10/10 — no query was captured by a deprecated skill name.
>
> **Final full-suite run (2026-07-02, post-restructure + minimal tombstones)**: all 166 dex+defi cases re-run with the finished branch state — Binance-style Intent Routing SKILL.md, capability three-file references, tombstones reduced to a one-line description ("DEPRECATED: merged into okx-dex. Use okx-dex instead.") and one-line body. **163/166 correct, recall 98.1%, precision 100%, zero wrong.** The 3 misses: T175/T249 allow "`okx-dex` or none" (harness cannot express mixed expectations — substantively passes) and T105 is a flaky borderline ("have they rugged before?" — passed in an earlier run, no skill invoked this time). Transcript-level audit confirmed **zero Skill invocations of any tombstone name across all 166 cases** — the minimal deprecation descriptions do not divert traffic.
>
> **Test-environment hazard (for future runs)**: the harness's child sessions inherit the machine's personal skills directory (`~/.agents/skills`, symlinked into `~/.claude/skills`). A mid-day run collapsed to 9% recall because a child executing the preflight auto-install had refreshed that directory with the PUBLISHED old skill set (bare names, full descriptions), which out-competed the `--plugin-dir` skills under test. Before trusting any run, verify the personal skills dir holds the same skill set as the branch under test.

Purpose: check whether merging `okx-dex-token` / `okx-dex-market` / `okx-dex-signal` / `okx-dex-social` / `okx-dex-trenches` / `okx-dex-ws` into one `okx-dex-data` skill causes trigger dilution — i.e. whether a fresh session can still (a) pick this skill over a sibling skill for the right queries, (b) get correctly excluded on hard-block queries that must go elsewhere, and (c) pick the right internal capability once inside.

## Methodology

30 queries were split into 6 batches of 5. Each batch was given to a **fresh, context-free** general-purpose agent (no knowledge of the "expected" answer or of this analysis) along with:
1. A pool of 5 candidate skill descriptions only — `okx-dex-data`, `okx-dapp-discovery`, `okx-security`, `okx-dex-swap`, `okx-defi-invest` — simulating what a router sees before any skill body is read.
2. Instructions to pick one skill per query, and — only if `okx-dex-data` was picked — read its body's Capability Routing table to also name a sub-capability (Token/Market/Signal/Social/Trenches/WS).

This is a proxy for real routing, not a guarantee — it doesn't include the other ~20 real skills as noise, and the agents were told explicitly that this was a routing exercise. Treat a pass here as necessary, not sufficient.

## Results — 30/30 correct

| # | Query | Expected | Got | Confidence |
|---|---|---|---|---|
| 1 | 帮我搜一下 PEPE 这个代币 | dex-data / Token | ✅ | high |
| 2 | 这个地址的持仓集中度和跑路风险怎么样 | dex-data / Token | ✅ | high |
| 3 | 热门代币榜单 | dex-data / Token | ✅ | high |
| 4 | BTC 现在多少钱 | dex-data / Market | ✅ | high |
| 5 | BTC 5分钟K线 | dex-data / Market | ✅ | high |
| 6 | 帮我看下这个钱包的胜率和盈亏 | dex-data / Market | ✅ | high |
| 7 | 综合指数价格是多少 | dex-data / Market | ✅ | high |
| 8 | 聪明钱最近买了什么 | dex-data / Signal | ✅ | high |
| 9 | 牛人榜前十名 | dex-data / Signal | ✅ | high |
| 10 | 追踪这个地址的交易 | dex-data / Signal | ✅ | medium |
| 11 | 最新加密新闻 | dex-data / Social | ✅ | high |
| 12 | 市场情绪怎么样，现在是牛市还是熊市情绪 | dex-data / Social | ✅ | high |
| 13 | 这个币的热度走势和KOL榜 | dex-data / Social | ✅ | high |
| 14 | pump.fun 上有什么新盘 | dex-data / Trenches | ✅ | high |
| 15 | 这个开发者有没有跑路记录 | dex-data / Trenches | ✅ | high |
| 16 | 这个币的捆绑狙击者分析 (狙击 + analytical noun, must stay read-op) | dex-data / Trenches | ✅ | high |
| 17 | 我要写一个websocket脚本监控代币价格 | dex-data / WS | ✅ | high |
| 18 | onchainos ws start 怎么用 | dex-data / WS | ✅ | high |
| 19 | 谁在狙击这个币，分析一下捆绑情况 (狙击 + analytical, must stay read-op) | dex-data / Trenches | ✅ | high |
| 20 | 这个币最近的实际成交记录，包括买卖 (must be Token trade history, not Signal tracker) | dex-data / Token | ✅ | high |
| 21 | BTC 5分钟涨跌市场 (Polymarket hard block) | okx-dapp-discovery | ✅ | high |
| 22 | 帮我买最火的pump.fun币 (write intent) | okx-dapp-discovery | ✅ | high |
| 23 | 狙击这个pump.fun地址0xabc (bare 狙击 verb, write intent) | okx-dapp-discovery | ✅ | high |
| 24 | 这个代币安全吗，是不是貔貅盘 | okx-security | ✅ | high |
| 25 | 帮我把这个币卖了 (no named venue) | okx-dex-swap | ✅ | high |
| 26 | BTC 5分钟价格 (timeframe alone ≠ kline, ≠ Polymarket) | dex-data / Market | ✅ | high |
| 27 | 哪些代币被聪明钱集体买入了 (aggregated buy-only, must be signal list not tracker) | dex-data / Signal | ✅ | high |
| 28 | BTC 现在是看涨还是看跌情绪 (mood, not price) | dex-data / Social | ✅ | medium |
| 29 | polymarket上BTC涨跌怎么样 | okx-dapp-discovery | ✅ | high |
| 30 | 这个token的巨鲸持仓和是不是社区认证 | dex-data / Token | ✅ | high |

## Reading

- **No misroutes.** All 5 hard-exclusion cases (#21–25, #29) correctly left `okx-dex-data` for `okx-dapp-discovery` / `okx-security` / `okx-dex-swap`. All 6 internal capabilities were picked correctly on both plain and adversarial (狙击 disambiguation, kline-vs-price, tracker-vs-signal, sentiment-vs-price) cases.
- **But the description had zero headroom to get there.** The frontmatter `description` that produced this result is 981/1024 chars — reached only by cutting most of the redundant bilingual trigger-phrase examples the original 6 skills used (e.g. explicit example sentences like "什么是..." style query templates, extra synonyms). The original 6 descriptions summed to ~5,780 chars; even the single most compressed prior individual description (`okx-dex-market` at 1,313 chars) already exceeded the 1,024 budget on its own. A first, less aggressively trimmed merge attempt measured 1,279 chars and would have silently failed to load.
- **Practical consequence**: this description now has no room for a 7th capability, a new hard-block rule, or additional trigger phrasing without another compression pass — every future edit risks silently exceeding the runtime's description limit again. The 2 medium-confidence results (#10, #28) suggest the margin for error in ambiguous phrasing is thinner than the unmerged skills had.

## Not covered by this trial

- The full 26-skill competition pool (only 5 candidates were shown per batch).
- Cross-references from `README.md`, `AGENTS.md`, `CLAUDE.md`, `workflows/*.md`, and other skills (`okx-wallet-portfolio`, `okx-security`, `okx-agent-payments-protocol`, `okx-defi-invest`, `okx-how-to-play`, `okx-agentic-wallet`, `okx-onchain-gateway`, `okx-dapp-discovery`, `okx-defi-portfolio`) that still point at the 6 original skill names — a real cutover needs those updated too. `okx-dex-data` was built standalone on the `experiment/merge-dex-skills` branch; the original 6 skills are untouched.
