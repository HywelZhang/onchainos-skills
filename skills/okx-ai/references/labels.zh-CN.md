# labels.zh-CN — user-visible string map (i18n rendering layer)

> Purpose: user-facing output is rendered in the conversation language (language lock) WITHOUT translating instruction prose. Look up UI labels here. Instructions/rule files stay English; only user-visible strings come from this table. Enums that must stay verbatim (`A2MCP`, `A2A`) are never translated.

## 1. Task status → 中文（任务卡片/状态行）

| status | 中文 |
|---|---|
| created | 已创建（待接受） |
| accepted | 已接受（资金已托管） |
| submitted | 已交付（待验收） |
| rejected | 已拒绝（24 小时决策窗口） |
| disputed | 争议中 |
| completed | 已完成 |
| close / closed | 已关闭 |
| expired | 已过期 |
| refunded / failed | 已退款（资金已退回） |
| admin_stopped | 平台已停止 |

## 2. Subscription lifecycle → 中文

| 事件/状态 | 中文 |
|---|---|
| trial | 试用期 |
| active | 订阅生效中 |
| renewed | 已续订 |
| expiring soon | 即将到期 |
| cancelled | 已取消 |
| period rejected | 本期已拒收 |
| refund claimed | 已申请退款 |
| auto-renew | 自动续订（开/关） |

## 3. Signal / trade fields → 中文（渲染信号卡片与执行摘要）

| 字段/值 | 中文 |
|---|---|
| side: buy | 买入 |
| side: sell | 卖出 |
| side: close_position | 平仓 |
| orderType: market | 市价单 |
| orderType: limit | 限价单 |
| orderPolicy: market | 市价单 |
| orderPolicy: signal_price_limit | 信号价限价单 |
| amount mode: percent | 按仓位比例 |
| amount mode: fixed | 固定金额 |
| venue: dex | 链上 DEX |
| venue: defi | DeFi 协议 |
| venue: trade_kit | OKX 交易账户 |
| venue: polymarket | Polymarket |
| venue: hyperliquid | Hyperliquid |
| environment: live | 实盘 |
| environment: demo | 模拟盘 |
| marginMode: cross | 全仓 |
| marginMode: isolated | 逐仓 |
| leverage | 杠杆 |
| slippageBps | 滑点上限（基点） |
| takeProfit / stopLoss | 止盈 / 止损 |
| expiry / expiresAt | 有效期至 |
| receipt / tx | 交易凭证 / 链上交易 |
| over cap | 超出单笔上限 |
| daily cap | 日累计上限 |

## 4. 执行模式（渲染策略状态）

| mode | 中文 |
|---|---|
| auto | 自动执行（受上限约束） |
| manual | 每次人工确认 |
| decline / not_set | 未授权自动执行 |
| notify only | 仅通知不执行 |
| script | 自定义脚本处理 |

## 5. A/B/C 决策文案（买方订阅信号，官方 CLI 拥有原文，此处为解释性译法）

| 选项 | 中文 |
|---|---|
| A | 执行本期，并开启受上限的自动执行 |
| B | 仅执行本期一次 |
| C | 跳过本期 |

确认 token: `1` / `是` / `go` / `确认`；继续 token: `1` / `next` / `下一步`。渲染决策卡时优先使用 CLI 输出的本地化文案，本表只用于模型自写摘要时保持一致。

## 6. 常用动词（任务/订阅操作按钮）

| EN | 中文 |
|---|---|
| publish / create task | 发布任务 |
| accept | 接受 |
| reject | 拒收 / 拒绝 |
| deliver | 交付 |
| complete | 验收通过（完成任务） |
| dispute | 发起争议 |
| agree refund | 同意退款 |
| claim refund | 申请退款 |
| claim auto-complete | 领取自动完成 |
| renew | 续订 |
| cancel | 取消 |
| subscribe | 订阅 |
| skip | 跳过 |
| execute | 执行 |
| pause auto copy-trade | 暂停自动跟单 |

## 7. 渲染规则（简短版）

1. 仅用户可见文本查此表；CLI 的 `*Label` 输出字段同样按上表译后再展示。
2. `#`id、地址、哈希、服务类型枚举 `A2MCP`/`A2A`、CDN URL 保持原样不译。
3. 用户输入的中文/英文原文（如信号文本）原样保留，不重写。
4. 表外新词: 首次出现按语义直译并在同句给英文原词，随后统一用中文。
