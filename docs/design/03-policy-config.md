# P0-03 per-node Policy 配置 Schema 与默认值

> 状态: 草案 v0.2（2026-09-03）— 修订: 全部默认动作可自定义（含 per-wire 覆盖），安全约束移至执行桥闸门
> 定位: 目标1(该直触发就直触发)/目标2(hook 出口)/目标3(订阅级灵活配置)的统一配置面。
> 配套: 节点清单 = 01；信号解析模式引用 02 §4。
> 原则: 默认动作只决定"没配置时怎么走"；**任何节点/事件的 mode 都可自定义**（direct/llm/ask/script/hybrid）；安全靠执行桥闸门（grants/consent/幂等/journal）在运行时强制，不靠配置层禁止。

## 1. 配置来源与优先级

```
作用域:  订阅级(最高) > 任务级 > 角色级 > 全局(最低)
同一作用域内两棵树:
  events.<wire>   按协议事件名直配(最高, 支持 "*" 兜底)      ← 每个事件都能单独自定义
  nodes.<flow-id> 按流程节点配(01 的节点ID)
再往上是该作用域的域默认 → 角色默认 → 全局默认
```

覆盖规则: 订阅级未配置回退任务级 → 角色级 → 全局。敏感字段(钱包/脚本参数)只存本地，随 gitignore，不进仓库。

存储布局（对齐官方 <onchainos_home> 惯例）:
```
<onchainos_home>/policies/
  global.yaml  role-buyer.yaml  role-asp.yaml
  sub-<subId>.yaml   job-<jobId>.yaml
<onchainos_home>/autotrade/grants/<jobId>.json   # 官方已有, 复用(不改格式)
```

## 2. Schema（YAML）

```yaml
schemaVersion: 1

# ---- 通用 ----
runtime:
  executor: bridge          # bridge(官方执行桥, 默认) | raw-cli(只读节点可用)
  journal: true             # 每次执行写 outcome journal
  notifyTo: [console]       # console | hermes | telegram:... (host 决定)

# ---- 信号(订阅) ----
signal:
  parseMode: loose          # strict | loose | notify | custom:scripts/alert-handler.py
  trustAsps:                # 白名单: 这些 agentId 的 strict 声明可信
    - "Agent#12"
  rules:                    # 信号规则(risk_grade 节点, direct 执行, 顺序匹配)
    - when: { asset.symbol: "XXX", side: buy }
      then: ask             # 或 auto / abort / ignore / script:xxx
    - when: { alertType: rug_pull, urgency: ">=3" }
      then: script:scripts/emergency-exit.py
  defaultWhenNoRule: ask    # 未命中规则的兜底: ask | notify | abort

# ---- 资金闸(auto 必需) ----
limits:
  venueGrants:              # 格式=官方 grants.json; 上限含校验期(一次性 grant 后逐笔核销)
    dex: { maxBuy: "200 USDC", maxSell: "200 USDC" }
    trade_kit: { maxBuy: "500 USDT", maxSell: "500 USDT" }
  dailyCap: "1000 USDT"
  assetWhitelist: [XXX, YYY]      # 空=不限制
  assetBlacklist: [SCAM1]
  requireConsentSnapshot: true    # 官方规则: 执行前必须有持久化 consent

# ---- 按事件直配(最高优先; "*" 兜底所有未配事件) ----
events:
  sub_user_reject:          # ASP 收到买家拒收本期
    mode: llm               # 自定: 先让 LLM 起草回应
  "*":
    mode: ask               # 未知/未配事件兜底: 一律人工(安全默认), 可全局改为 direct

# ---- 按流程节点配(01 节点ID) ----
nodes:
  buyer.sub.decide:
    mode: auto              # auto | script:xxx | llm | ask | hybrid
    confirm: once           # always | once | never(仅 auto+grant 且金额在限内)
    vetoFallback: ask       # pre-hook veto 后: ask | abort | notify
  buyer.task.complete:
    mode: ask               # 资金释放; 默认 ask, 可覆盖 auto/script(闸门: consent+grant)
  buyer.sub.signal_manual:
    mode: ask

# ---- hooks(节点前后出口; 目标2) ----
hooks:
  buyer.sub.signal_received:
    post: [scripts/audit-log.py]          # observer: 失败仅记日志
  buyer.sub.pre_execute:
    pre:  [scripts/check-drawdown.py]     # control: 非0退出=veto → vetoFallback
  asp.sub.on_request:
    post: [scripts/provision.py]          # ASP 自定义操作空间
  asp.task.deliver:
    pre:  [scripts/quality-gate.py]

# ---- ASP 输出模板(可选, 见 02 §5) ----
signalProfile:
  template: okx-signal-trade-v1
  classes: [trade]
  fieldCoverage: { trade: [side, asset, venue, amount, orderType, slippageBps] }
  schemaMode: strict
```

字段约束: `mode` 枚举对任何节点开放（可自定义），解析器只做域内合法性检查（如某节点的 script 是否在白名单目录）；`then: script:` 与 hook 必须指向白名单目录 `scripts/` 内的文件（禁止任意路径/shell 拼接），超时默认 30s。

## 3. 节点默认值表（v1 出厂默认; 全部可覆盖）

> 默认值只决定"没配置时怎么走"。任何节点都可用 `nodes.<id>.mode` 或 `events.<wire>` 覆盖——不存在"不可配置"的节点。下表"常见覆盖"列只列推荐做法；auto/script 的硬性前提 = 该节点涉及的执行桥闸门齐备（limits/consent/journal），闸门不齐 → 执行被拒并降级 ask（fail-safe），而不是配置被拒。

| 节点 | 默认 mode | 常见覆盖 | 说明 |
|---|---|---|---|
| buyer.task.publish_intent | llm+ask | llm | 语义收集 |
| buyer.task.publish | ask | auto/script(闸门: 预算上限+journal) | L3 escrow |
| buyer.task.accept_asp | ask | auto/script(闸门: consent) | L3 |
| buyer.task.review_deliverable | hybrid | llm/script | 内容检查(默认过 LLM) |
| buyer.task.complete / reject | ask | auto/script(闸门: consent+grant) | L3 资金 |
| buyer.task.claim_refund | direct+notify | ask/script | 协议兜底 |
| buyer.task.dispute | llm+ask | llm | L4 |
| buyer.sub.signal_received | direct | —(常态直触发, 亦可挂 hook) | 落盘/校验 |
| buyer.sub.signal_parse | strict→direct / loose→llm | notify/custom | 按 02 §4 |
| buyer.sub.risk_grade | direct | script(自定义规则) | 规则引擎 |
| buyer.sub.decide | ask | auto/script/llm | auto 需 limits 完整否则执行被拒 |
| buyer.sub.pre_execute / execute / post_execute | direct | —(执行桥路径固定) | 永不 llm |
| buyer.sub.exec_exception | hybrid | ask/script | 异常分派 |
| buyer.sub.security_alert | script(有规则)/ask(无规则) | llm | 买家自定义逻辑场景 |
| buyer.sub.subscribe_create | ask | auto(闸门: 首次人工 once) | consentSnapshot 采集 |
| buyer.sub.renew_decision | ask | auto(闸门: grant+cap) | L3 |
| asp.sub.selected | hybrid | ask/auto(策略: 自动接单) | 接受/拒绝 |
| asp.sub.on_request | direct(hook 出口) | script | 目标3 ASP 空间 |
| asp.task.deliver | llm(生成)+ask(提交) | script | 内容缝 |
| asp.task.rejected_respond | llm+ask | llm | L4 |
| asp.task.claim_complete | direct+notify | — | 协议兜底 |
| asp.task.dispute_phase | llm+ask | llm | L4 |

## 4. 闸门与降级规则（运行时强制，配置不可绕过）

1. L3 节点要 auto/script 生效: 必须同时满足 limits 里该 venue grant 已配 + consentSnapshot 存在 + journal 开启 + （首次）人工确认 once。不满足 → 执行被拒 + 降级 ask + 提示缺哪一项。
2. direct 节点不消费 LLM，出错走重试（5xx/网络一次）+ journal 恢复，不猜测。
3. llm 节点输出永远是"建议/草稿"，落到 L2/L3 动作前必须过确认或结构化校验；LLM 生成的资金参数被执行桥拒绝（红线，见 01 §0.4）。
4. hook 分两类: observer（post 日志/通知类，失败不阻断）与 control（pre 校验类，非零退出 = veto）。veto 后按 vetoFallback（默认 ask，即退回人工）。
5. 新事件到达但无任何配置 → 按 events."*"（默认 ask）+ 日志（fail-safe: 未知写操作一律人工）。
6. 配置本身也过闸: script 白名单目录校验、hook 超时、金额/频率上限的解析在任何执行前完成。

## 5. 落地示例

### 5.1 买家: 信任的 strict ASP + copy-trade 自动执行
```yaml
signal: { parseMode: strict, trustAsps: ["Agent#12"], defaultWhenNoRule: notify }
limits: { venueGrants: { dex: { maxBuy: "200 USDC", maxSell: "200 USDC" } }, dailyCap: "1000 USDT" }
nodes:
  buyer.sub.decide: { mode: auto, confirm: once }
```
效果: Agent#12 的每期信号信封字段齐 → risk_grade 过规则 + grant 在限 → 执行桥直下单；字段缺 → 拒绝并 notify；超限 → 拒绝。

### 5.2 买家: 安全告警自定义逻辑（紧急赎回）
```yaml
signal: { parseMode: loose }
nodes:
  buyer.sub.decide:
    mode: script:scripts/alert-policy.py   # 脚本自决(查持仓/链上状态 → 决定赎回/调仓/忽略)
events:
  "*": { mode: notify }                    # 例: 全局只通知, 不动任何钱
```
LLM 只做 alertType 分类(loose)，资金动作全在买家脚本内（对脚本负责）。

### 5.3 ASP: 订阅创建时触发自有预置 + 交付质量门
```yaml
hooks:
  asp.sub.on_request: { post: [scripts/provision.py] }
  asp.task.deliver: { pre: [scripts/quality-gate.py] }
signalProfile: { template: okx-signal-trade-v1, classes: [trade], fieldCoverage: { trade: [side,asset,venue,amount] }, schemaMode: strict }
```

## 6. 开放问题

1. 配置编辑面: v1 用 YAML 文件 + `onchainos policy set/get` 命令，还是只文件？(建议 v1 只文件+校验命令，v2 加交互)
2. limits.grants 与官方 autotrade/grants 的核销联动: 官方是"文件内 cap"语义还是"逐笔核销"(executor 有 EXECUTION_LATCH/journal)？需读 grants 校验核心确认后定联动语义（P1 前置）。
3. notifyTo 通道由宿主实现（Hermes console/telegram），CLI 只发事件——宿主适配器接口待定。
4. role=evaluator 节点暂缓（01 附录 A），schema 的 events."*" 已保证其事件默认安全落 ask/notify。
5. events.<wire> 的 wire 名以 state_machine.rs parse 函数为准（如 sub_user_reject）；上游 sync 后需对账（scripts/audit-events.py 辅助）。
