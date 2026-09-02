# P0-02 订阅信号载荷 Schema（双层 + 可配解析模式）

> 状态: 草案 v0.1（2026-09-03）
> 背景: 官方 v1 曾有全结构化 AutoTradeSignal（schema.rs，已废弃）；现生产路径 = ASP 交付自由文本 → 买家 LLM 分类 → 执行，护栏 = consentSnapshot + grants(按 jobId 分 venue 上限) + 每次 readiness。
> 本设计: 不强制结构化（保留 ASP 自由文本便利），也不把解析全押 LLM（保留买家确定性）；用「推荐字段层 + 永远保留 raw」+ 每订阅解析模式 strict/loose 达成两头。
> 红线: LLM 只做分类，不做资金参数生成。确定性执行只消费结构化字段，且必须过 grants/consent 闸。

## 1. 载荷三层结构

```
deliverable (协议层, 固定, 不可改)
└── signal envelope (建议层 v1)          ← 本次设计
    ├── header: 固定公共字段(轻量)
    ├── body:   按 signalClass 的推荐字段(允许缺省/部分)
    └── raw:    原始全文/附件(必保留)
```

协议层不变: 交付物仍是文本/文件（官方 `agent deliver` 行为）。信封 = 交付物正文中的一段可识别块（如首部 YAML/JSON fence）或独立 .json 附件；**raw 永远等于交付物全文**，信封只是它的结构化投影。买家侧解析器先找信封，找不到 → 视为纯 raw（= 官方现状 loose 行为）。

## 2. 公共头 header

```jsonc
{
  "schemaVersion": 1,            // 信封版本; 更高版本 → 按 raw 处理并告警
  "signalClass": "trade",        // trade | security_alert | custom:<id>
  "signalId": "dlv_8f3a...",     // 幂等键, 必须=协议 delivery_id(官方规则)
  "issuedAt": 1769999999000,     // ms epoch
  "expiresAt": 1770086399000,    // 可选; 过期信号禁止执行(直接 notify)
  "sender": { "agentId": "Agent#12", "serviceId": "svc_9" },
  "template": "okx-signal-trade-v1", // 字段集模板 id(见 §5), 供买家预校验
  "lang": "zh-CN"
}
```

头只做: 路由提示、过期判断、幂等键、模板预校验。不做资金决策输入。

## 3. body（推荐字段，按 signalClass）

### 3.1 signalClass = trade

```jsonc
{
  "side": "buy",                        // buy|sell|close_position(官方规范: 变体归一到 place/close_position)
  "asset": { "chain": "solana", "symbol": "XXX", "address": "..." },  // address 优先于 symbol
  "venue": { "kind": "dex" },           // dex|trade_kit|dapp:<id>; 决定 grants 命名空间
  "amount": { "mode": "percent", "value": 5 },   // percent|fixed; 见上限规则
  "orderType": "market",                // market|limit; 官方限制: 不支持 batch/iceberg/TWAP/algo 等
  "priceHint": 0.123,                   // 可选
  "leverage": null,                     // 仅 perp; 必须=consentSnapshot.marginMode/leverage 一致
  "slippageBps": 500,                   // 可选; 硬上限 500(=官方 MAX_SLIPPAGE_BPS)
  "takeProfit": null, "stopLoss": null  // 可选
}
```

### 3.2 signalClass = security_alert（买家自定义逻辑场景）

```jsonc
{
  "alertType": "rug_pull",   // rug_pull|lp_removal|mint_burst|exploit|malicious_approve|liquidation_wave|whale_dump|custom
  "chain": "bsc",
  "targets": ["0x..."],      // 受影响地址/持仓 token
  "urgency": 3,              // 0-4; >=3 时买家侧可配置"直接触发脚本"而跳过 ask
  "detailRef": "attachment://alert-123.pdf"  // 可选
}
```

security_alert 不进交易执行器；它的消费方 = 买家规则表/脚本（如: alertType=rug_pull 且持仓>X → 调紧急赎回脚本）。

### 3.3 校验规则（解析器强制，缺省即降级，绝不猜测）

| 条件 | 行为 |
|---|---|
| header 缺 / schemaVersion 高 | 按 raw 处理（loose 语义），告警一次 |
| strict 模式: side/asset/amount/venue 任一缺失 | 拒绝执行 → notify 买家(附缺什么)；不补全 |
| loose 模式: 上述字段缺失 | 交 LLM 从 raw 补全，标 `inferred:true`+confidence；<0.7 → ask；金额类 inferred 仍须过 grants |
| amount > grants.max_buy/max_sell | 拒绝（官方 deny 码语义复用: over cap） |
| slippageBps>500 / ttl>86400 / signalId 重复 | 拒绝（幂等） |
| 信号过期(expiresAt<now) | 禁止执行，notify |
| venue/asset 不在买家白名单 | 按买家策略: ask | abort | 忽略（可配） |
| 执行前 | 每次重跑: 订阅 Active 校验 + consentSnapshot + readiness（官方规则照搬） |

## 4. 解析模式（每订阅可配, 03 落地）

| 模式 | 语义 | 适用 |
|---|---|---|
| strict | 只信结构化字段; 缺关键字段不执行; LLM 不参与资金解析 | 声明 signalProfile 且字段覆盖齐的 ASP |
| loose | raw + LLM 分类补全(inferred+confidence), 资金参数仍过闸 | 自由文本 ASP |
| notify | 永不执行, 只通知买家 | 观望期/纯资讯订阅 |
| custom:<script> | 把 envelope+raw 交给买家脚本, 脚本决定 | 高级买家(赎回/调仓等) |

默认: ASP 无 signalProfile 声明 → loose+ask；声明且字段齐 → strict 可 auto（需要 grant）。

## 5. signalProfile（ASP 侧声明——把结构化从"强制"变"市场激励"）

ASP 在服务/订阅可见处声明（服务列表字段或交付头）:

```yaml
signalProfile:
  template: okx-signal-trade-v1
  classes: [trade]
  fieldCoverage: { trade: [side, asset, venue, amount, orderType, slippageBps] }
  schemaMode: strict        # ASP 承诺完整字段; 若缺 → 买家可拒执行/投诉(声誉机制)
  sample: "..."             # 示例信号, 供买家预配置 policy
```

- 有 profile + strict 的 ASP → 买家可放心开 auto（配合 grants 上限）→ 订阅转化率与定价提升；
- 无 profile → 买家默认 loose+ask；
- fieldCoverage 缺失字段 → 买家侧解析器自动降级对应维度（如无 slippage → 用默认 500）。
- ASP 端发信号时用模板生成信封（我们提供模板/生成器默认实现 = LLM 填模板 + 字段自检），raw 仍发全文 → 买家可复核。

## 6. 边界与待验证

1. [部分解决·源码] 交付物落盘: 内联文本存 .txt（长文本可能以 .md 上传），附件走 fileKey；savedPath 透传进 [Persisted delivery context]（flow_lifecycle/core.rs）。信封载体定为: 交付 .txt 内首部 JSON fence 块 + raw 保留全文；附件按原样。仍留一次真实交付采样验证（需登录态）。
2. 官方对交付物的"买家侧模型路由"仍会存在（协议/UI 要求）——我们的 strict 路径与其并行: 检测到可解析信封时走确定性引擎, 否则维持官方模型路由。
3. 信号加密/签名: v1 不引入（信任=订阅关系+声誉）；若需要防 ASP 抵赖/伪造, 后续在 envelope 加 senderSig（私钥签名）——评估必要性后再说。
4. grants 文件当前按 jobId 存 venue caps（官方）。我们的订阅级 caps 复用同一文件格式与校验核心, 不另造轮子。
