# Keyword glossary (中文) — OKX Agent Payments Protocol

> Loaded on demand from `../SKILL.md` when the user's query is in Chinese, or when you
> need the exact Chinese phrasing for a user-facing card / status line. The routing rules,
> gates, and behavior are defined in `SKILL.md`; this file only carries the Chinese
> trigger vocabulary and the Chinese equivalents of the card / narration phrasings.

## Chinese trigger keywords

Any of these (中文) route to this skill, same as the English triggers in `SKILL.md`:

- 按量计费、支付上限、支付通道、关闭/充值/续费/结算通道、关闭会话、结算会话、凭证、会话支付、付款链接、创建支付、支付状态
- 订阅 / 续订 / 周期扣款 / 取消订阅 / 升级套餐 / 降级套餐 → `period` scheme (see `subscription.md`)

Any close / topup / settle / voucher / refund near a `channel_id` or session context = MPP
mid-session op → `session.md`.

## Chinese card phrasing

The confirmation-card lead line (中文), equivalent to the English form in `SKILL.md` Rule-1 example:

> `准备通过 **OKX Agent Payments Protocol** 完成本次支付，下面是扣款明细，请确认……`

Keep **OKX Agent Payments Protocol** as a bolded English noun phrase even inside a Chinese sentence.

## Chinese status-narration anchors

Same ❌ / ✅ rules as the anchor table in `SKILL.md` — these are the verbatim Chinese equivalents.
When narrating in Chinese, obey these exactly:

| ❌ 不要说 | ✅ 可以说 |
|---|---|
| "收到 HTTP 402，触发 OKX Agent Payments Protocol" / "Detected `PAYMENT-REQUIRED`, loading `exact`" | _(保持静默 — 检测 / 路由属于内部逻辑)_ |
| "CLI 选了 `exact`，组装 `PAYMENT-SIGNATURE` 头" / "走 TEE 路径" | "签名完成，正在重放请求" |
| "检测到 2 个 scheme：exact (USD₮0)、aggr_deferred (USDG)" / "正在查余额筛选候选" | _(保持静默 — 枚举 + 余额检查属于内部逻辑；只有推荐卡片对用户可见)_ |
| "进入 session / charge 模式" | "支付通道已开" — 描述用户可见的效果，而非内部模式 |
| "按之前的偏好，直接付不再确认" | _(禁止 — 不存在这种偏好；每次都必须走确认关卡)_ |

## Chinese rendering rules

- Payment type (Step A4, `WWW-Authenticate: Payment`): render as `单次支付` / `会话支付（多请求）` — **NEVER** `单次购买`.
- Request-parameter capture examples (Step A1): "查 San Francisco 的天气" → `city=San Francisco`; "翻译成中文" → `lang=zh`.
