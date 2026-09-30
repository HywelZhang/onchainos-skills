# P0-11 CLI → 直连 API 重构方案（okx-ai 去 CLI 化）

> 状态: **v1.1**（2026-09-30）。v1.0 定稿后按用户决策收窄：**不做官方 CLI/MCP 兼容层，只实现本方案自身**（OQ-16 修订、形态 D/C 取消，见 §0）。
> 范围: okx-ai 域（单次任务 + 订阅任务 + 身份），不含 wallet/defi/dex-market 日常流程。
> 依据: 本仓 `cli/src/**`（源码可读）+ 本机 `@okxweb3/a2a-node@0.2.10` 实测 + `skills/okx-ai` 全量统计 + `~/.onchainos/audit.jsonl`（3,123 条真实调用）+ 实机只读命令。

---

## 0. 决策表

| OQ | 决策 | 影响 |
|---|---|---|
| OQ-13 消息面 | **C + A**：轮询优先覆盖订阅信号；XMTP 保留兜底交互聊天 | P2a / P2b 两个子任务 |
| OQ-14 运行形态 | **B 主 + A 轻量**（本地 HTTP 8788 + 仓库内库脚本） | agent 侧只需 `fetch` |
| OQ-15 语言栈 | **Python 主 + Node 仅 XMTP** | Node 退化为宿主侧 xmtp-bridge 单进程 |
| OQ-16 CLI shim | v1.1 曾定为"不做兼容层"；**v1.2 修订见 OQ-19** | — |
| OQ-17 P2 范围 | **交互式 + 信号都做** | P2a 轮询信号 / P2b bridge 交互 |
| OQ-18（新增） | 运行依赖 = 0 个 CLI；**基准数据**与**应急二进制**另行保留 | 见 §4.4 |
| **OQ-19（v1.2 修订）** | **签名/私钥/钱包/登录保持在官方 CLI，不在改造范围**；gateway 只管 okx-ai 任务与订阅**流程** | 写操作 = gateway 组参 + **CLI 执行**（宿主侧内部调用）；agent 侧仍无二进制；上游漂移面收窄（见 §12） |

---

## 1. 现状拆解

```
skills/okx-ai (dev: 37 文件 / 499,220 字符 ≈ 142k tokens)
 ├─ onchainos           Rust CLI（本仓 cli/ 源码可读，agent 命名空间 107 子命令）
 │   ├─ L1 HTTP    web3.okx.com  /priapi/v1/aieco/*（约 30 端点）
 │   │                    + /priapi/v5/wallet/agentic/*（约 20 端点：会话/签名/广播）
 │   ├─ L2 会话     JWT + refreshToken、sessionCert、agenticId 头、device-id 头
 │   ├─ L2 密码学   HPKE(DHKEM(X25519,HKDF-SHA256)+HKDF-SHA256+AES-256-GCM, info="okx-tee-sign")
 │   │              解出 32B Ed25519 seed → 本地签名 → gen-msg-hash → sign-msg → broadcast
 │   └─ L3 业务/状态 nextAction 信封、statusLabel 文案、状态机、幂等、audit.jsonl
 └─ okx-a2a            @okxweb3/a2a-node（Node，闭源 dist bundle，@xmtp/node-sdk 6.1.0）
     ├─ XMTP 消息面   user watch / session send / xmtp-send / user check
     └─ 反向依赖      shell 调用 onchainos
```

skill 实际用到：**onchainos 57 命令 + okx-a2a 12 命令**。

### 1.1 决定判断的实测证据

| 事实 | 证据 | 含义 |
|---|---|---|
| 服务端不返回业务文案 | `statusLabel/statusDescription` 在 `identity/utils.rs` 本地映射 | 渲染层在客户端 |
| 服务端不返回流程信封 | `nextAction` 在 `a2mcp_probe/flow.rs`、`receive.rs` 本地组装 | 功能层本就在客户端 |
| 交付物读本地盘 | `deliverables.rs` → `~/.onchainos/deliverables/`（本机 262KB） | 迁移保持目录布局 |
| 无本地私钥 | `crypto.rs:20-98`、`sign.rs:119-154`（TEE 托管，只签 msgHash） | 不需要 KMS/HSM |
| 附件面已是 HTTP | `chat/mod.rs:10-14` `im/attachments/xmtp/encrypted/{upload,download}` | 只有消息收发在 XMTP |
| 消息面无 HTTP 端点 | aieco 清单只有 eligible / system-config / risk | 交互聊天必须 XMTP |
| 设备身份是本地派生 | `device/id.rs`：`sha256(machine_id+"onchainos")`；订阅有 receive-device 白名单 | 须复现同一派生 |
| 上游带 DoH 故障转移 | `cli/src/doh/**` | CN 需 DoH 或代理 |
| CLI 输出里带 LLM 指令 | 实跑 `pending-decisions-v2 list` 输出含 "Render the line above…" | gateway 必须显式接管 |

---

## 2. 可行性矩阵

| 能力 | 今天靠 | 纯 HTTP | 成本 |
|---|---|---|---|
| 登录/续期/设备身份 | onchainos | ✅ | 低 |
| 查询（任务/订阅/服务/身份/评价） | onchainos | ✅ | 低 |
| 身份注册/更新/激活 | onchainos | ✅ | 中（需广播） |
| 创建任务/订阅并出资 | onchainos | ✅ | 中高 |
| 接单/交付/验收/拒绝/退款/评分 | onchainos | ✅ | 中高（幂等 + 双签） |
| nextAction / 文案 / 状态机 | onchainos 本地 | ✅（移植） | 中 |
| 附件上传/下载 | onchainos HTTP | ✅ | 低中 |
| 订阅信号交付 | okx-a2a 长轮询 | ✅（轮询） | 中 |
| 交互式聊天/澄清/peer | okx-a2a + XMTP | ❌ | 高（Node bridge） |

约 70% 可直连 HTTP；约 20% 需本地移植；10% 保留 Node bridge。

---

## 3. 核心判断：不需要改后端

"功能层面 API"今天就是客户端逻辑（§1.1 前两条），搬到本地层是**同架构换运行时**；服务端只提供资源级端点。反向证据：若逻辑在后端，CLI 不必有 107 子命令与 13k 行 agent_commerce。

---

## 4. 目标架构（v1.1：无兼容层）

```
     ┌──── agent 环境（任意 harness；只需能发 HTTP，或能跑 python）────┐
     │  skill: skills/okx-ai/ → 路由 + 信封契约 + verb 表（≤800 行）    │
     └──────────┬──────────────────────────────────┬──────────────────┘
      形态 B: HTTP 127.0.0.1:8788       形态 A: python scripts/okxai/cli.py <verb>
                ▼                                  ▼
 ┌────────────────────── okxai gateway（Python，宿主侧常驻）──────────────────────┐
 │ session   登录/续期/sessionCert/agenticId/device-id（沿用 ~/.onchainos 布局）   │
 │ crypto    X25519+HPKE 解 seed、Ed25519 签名、附件密钥（cryptography / pyhpke）  │
 │ domain    task / subscription / identity / refund / rating 状态机（移植语义）   │
 │ envelope  nextAction + statusLabel + 本地化模板 + 幂等键 + audit.jsonl          │
 │ transport HTTP → web3.okx.com/priapi/*（DoH 或 Clash 代理）                     │
 │ inbox     P2a 轮询（inProgress / subscribe / deliverables）→ 事件 JSONL（06 §4）│
 │           P2b 交互 → 调用 ▼                                                     │
 └──────────────────────────────┬─────────────────────────────────────────────────┘
                                ▼
        xmtp-bridge（Node，宿主侧单进程）: 优先 import @okxweb3/a2a-node 的 dist/index.js
        （导出面不足时同进程 spawn okx-a2a）→ 对 gateway 暴露 stdio JSON-RPC
```

**运行依赖 = 0 个 CLI**：agent 侧无二进制；宿主侧 = 1 个 Python 守护 +（仅 P2b）1 个 Node bridge 进程。`okx-a2a` 若有出现，只存在于 bridge 进程内部，对 agent 与 skill 均不可见。

### 4.1 形态

| 形态 | 交付 | agent 需要 | 用途 |
|---|---|---|---|
| **B**（主） | `okxai serve --port 8788`，宿主侧守护 | 只 `fetch` localhost | 只允许 HTTP 工具的 agent |
| **A**（轻量） | `python scripts/okxai/cli.py <verb>` | 能跑 python，无全局安装 | 沙箱可用解释器 |

（v1.0 的形态 C/D 已取消：不做官方 CLI/MCP 兼容。）

### 4.2 verb 面（26 个，只覆盖产品路径）

读（P0）：`session.status` · `agent.mine/get/search` · `service.match/list` · `task.mine/detail/inProgress/deliverables` · `sub.list/detail/cost` · `feedback.list` · `flow.next`

写（P1）：`task.create` · `task.accept/decline` · `task.deliver` · `task.complete/reject`（双签） · `task.claim` · `sub.create/cancel/autorenew/device/offline` · `refund.prepare/execute/confirm` · `rating.submit` · `service.paramUpdate`

消息（P2）：`watch.poll` · `msg.send/session` · `user.notify` · `file.up/down`

非产品路径能力（evaluator 质押/仲裁/投票、x402/a2mcp、跨链等 ~30 命令）**不再提供**（与 OQ-1 范围、OQ-8 evaluator 不做一致）。

信封与今天一致（`phase/decision/reason/nextAction/payload`），渲染与本地化规则不变。

### 4.3 取消并行期的收益（这条是 v1.1 的主要改动收益）

去掉兼容层不只是"少写点映射代码"，而是消除一类**真实竞态**：两个客户端同时持有同一份 `~/.onchainos/keyring.enc` + `session.json`，会同时刷新 JWT。若服务端对 refresh token 做轮换（单次有效），并行期就会出现"A 刷新把 B 踢下线"，表现为随机 401 与恢复抖动。不做并行期 = 这类故障面直接归零。

### 4.4 取消兼容层后必须补的两个替代物

| 替代物 | 内容 | 为什么必须 |
|---|---|---|
| **基准数据（golden fixtures）** | 改造前用现有 CLI 录制真实输出 | 原验收"逐字段 diff = 0"原本靠 CLI；去掉 CLI 后它是唯一回归裁判 |
| **应急二进制归档** | 当前 CLI 二进制 + a2a-node dist 各留一份离线归档 | 不做兼容层 ≠ 不留逃逸路径 |

**P0.0 已执行（2026-09-30）**：

- 录制脚本 `scripts/record-golden.py`（可复跑）；覆盖 18 条读路径命令，**16 条成功**（2 条为 4.5.2 参数形状差异）。
- 原始输出（含账号数据，**不进 git**）：`$ONCHAINOS_HOME/golden/cli/20260930-180440/`。
- 仓库内脱敏基准：`tests/cli-golden/manifest.redacted.json` + `tests/cli-golden/README.md`（含比对口径）。
- 应急归档：`$ONCHAINOS_HOME/archive/20260930/`（21MB：`onchainos-4.5.2.exe` sha256 `b97dd05d…16a6d` + `a2a-node-0.2.10/dist` + SHA256SUMS）。
- 归档二进制的 sha256 与 CLI 自记的 `~/.onchainos/binary_identity.json`（2026-09-02）**一致** → 与产出 `audit.jsonl`、通过既有验收的是同一二进制。
- **版本口径**：本仓 `Cargo.toml`=4.6.3，本机实装=**4.5.2**；基准以实装为准（gateway 只对齐服务端接口，不对齐 CLI 版本）。


> 隐私处理：真实输出含 agentId / 钱包地址 / jobId / 订阅金额，落在仓库之外（`$ONCHAINOS_HOME/golden/`），**不提交到公开仓库**；仓库内只放录制脚本 + 脱敏 manifest + 比对口径说明。

---

## 5. skill 瘦身

dev 现状 **37 文件 / 499,220 字符**，其中纯 CLI 机械面：

| 文件 | 字符 | 判定 |
|---|---|---|
| task-cli-reference.md | 58,221 | 纯 CLI 语法 → 删除 |
| task-user-playbook.md + .lite | 66,463 | 命令 playbook → 压缩为语义 |
| identity-register.md + .lite | 67,797 | 命令流程 → 压缩 |
| watch-core.md + .lite | 53,289 | 长轮询机制 → 改为 poll/daemon 说明 |
| chat-cli-reference.md + chat-comm-init.md | 6,981 | 删除 |
| watch-outdated-list / watch-wake-scheduling / watch-background-recovery | 9,316 | 归 gateway |
| **小计** | **262,067（52%）** | |

目标：`SKILL.md`(≤8KB) + `references/verbs.md`(≤6KB) + `references/decisions.md`(≤5KB) + `task-subscription-signal.md` 压缩版(≤6KB) + `task-core.md`(≤5KB) + `labels.zh-CN.md`(5.5KB) ≈ **≤40KB 全量（-92%）**；单次订阅事件加载 **77–90KB → ~20KB（-75%）**。

---

## 6. 量化提升评估（实测基线）

### 6.1 真实调用数据（`audit.jsonl` 2026-09-02..04，3,123 条）

| 指标 | 实测 |
|---|---|
| CLI 级调用延迟 | p50 **614ms** / p90 **3,154ms** |
| api 级调用延迟 | p50 365ms / p90 1,150ms |
| 最慢业务命令 | create-task p50 **6,993ms**、user-notify p50 **4,225ms**、service-match 1,076ms、next-action 1,038ms |
| 成功率 | **95.0%**（155 失败 → 触发恢复流程） |
| 身份/心跳开销 | heartbeat 808 + agent get 776 + get-my-agents 121 + get-agents 109 = **1,814 = 58% 的调用** |
| 冷启动（本次实跑） | `agent get-my-agents` **21,532ms**（DoH 冷路径） |
| 单次输出体量 | 85B（本地决策）～3,708B（身份列表）；subs 1,996B；status 196B |
| 隐藏成本 | CLI 在 stdout 里塞给 LLM 的渲染指令 |

### 6.2 提升预期

| 维度 | 现在 | 改后 | 量级 |
|---|---|---|---|
| skill 体量 | 499KB / 37 文件 | ≤40KB / ≤6 文件 | **-92%** |
| 单事件上下文 | 77–90KB（22–26k tokens） | ~20KB（~5.7k tokens） | **-75%** |
| 每流程 CLI 调用数 | 100%（58% 是开销） | 0 | **-55~60% 调用数** |
| 单调用延迟 | p50 614ms，冷启 21.5s | 单跳 HTTP / 进程内 | 去 spawn 与冷启 |
| 失败面 | 5% 失败 + 恢复流程 | 幂等 + 单跳重试 | 目标 <1%（P1 实测） |
| 运行依赖 | 2 个 CLI（二进制 + Node 包） | agent 侧 0；宿主侧 1 Python（+P2b 可选 Node） | 满足"不放二进制" |
| 并行期竞态 | — | 消除 JWT 刷新互相踢（§4.3） | 新增确定性收益 |
| 确定性执行 | policy 已 0 LLM（4/4） | 写操作同样 0 LLM | 保住数量级优势 |

不提升：链上/服务端延迟、LLM 内容质量、CN 网络。

### 6.3 验证方式

每阶段 `hermes -z + --usage-file` 跑 **v1.1 vs v1.0(CLI 路径)** 同任务 A/B（token/成功率/耗时），付费预算 ≤0.1 USDT/轮（已批）；回归断言以 golden fixtures 为准（§4.4）。

---

## 7. 分阶段计划与验收

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0.0**（已完成 2026-09-30） | **录制 golden fixtures + 二进制归档**（改造前的最后窗口） | ✅ 16/18 读命令已录；脱敏 manifest + 归档（sha 与 `binary_identity.json` 一致）已落盘，见 §4.4 |
| **P0**（✅ 完成 2026-09-30） | session（登录/续期/device-id）+ 16 读 verb + 契约测试骨架 | ✅ **16/16 PASS / 0 FAIL / 0 ERROR**（`gateway/tests/contract_golden.py`；脱敏报告 `tests/cli-golden/contract-p0-report.redacted.json`） |
| **P1**（3–5 天） | 12 写 verb + 签名链 + 幂等 + audit + 本地化模板接管 | 真实付费端到端 ≥1 轮；重复调用不二次出资；链上/服务端可核对；与 1 轮写操作 golden 对照 |
| **P2a**（2–3 天） | 轮询版 inbox → 事件 JSONL | 真实订阅收到 ≥1 真信号；watch-host / policy-engine 零改动 |
| **P2b**（2–4 天） | xmtp-bridge（Node）接管交互聊天 / 澄清 / user-notify | 同一条 `msg.send` 送达且 `session history` 与改造前一致 |
| **P3**（2–3 天） | skill 按 §5 瘦身 + A/B | 单事件 token 实测 -≥60%；成功率不低于 CLI 版 |

回退策略（v1.1 修订）：不再"切回 CLI 跑日常"，而是**回滚到上一阶段版本 + 必要时用归档二进制手工处置**。

---

## 8. 是否最优：对照与残差

### 8.1 对照

| 方案 | agent 侧无二进制 | 重实现成本 | 结论 |
|---|---|---|---|
| **本方案 v1.1**（Python gateway + Node bridge，无兼容层） | ✅ | 中（26 verb，无映射层） | **在约束下最优** |
| v1.0（带 CLI/MCP 并存形态 D） | ✅ | 中 + 双路径维护 + 并行期竞态 | 已被用户收窄取代 |
| 直接用官方 CLI（现状） | ❌ | 0 | 受限环境不可用 |
| 只做只读直连、写留 CLI | 部分 | 低 | 丢掉"确定性自主执行"卖点 |
| 全量重实现 107 命令 | ✅ | 高 | 浪费；已收敛到 26 verb |
| 重写 XMTP 客户端 | ✅ | 很高 | 无必要，Node bridge 已够 |

### 8.2 结论（诚实版）

1. **约束下接近最优**；v1.1 比 v1.0 更优——去掉并行期同时消除了 JWT 轮换竞态（§4.3），并省下映射与双路径测试。
2. **代价转移了**：从"兼容层维护"转移到"上游漂移 + 无同步修复通道"。没有 CLI 就没有"上游改接口我们跟着升一版就修好"的通道，所以 **golden fixtures（P0.0）是方案成立的前提，不是可选项**。
3. 因此 P0.0 必须**先做、且必须在停止使用 CLI 之前做**；否则基准数据永久丢失，验收会退化成"自证"。
4. 残差：P2b 的 bridge 仍依赖闭源 bundle 导出面（最坏退化为同进程 spawn `okx-a2a`，仍满足"agent 侧无二进制"）。

---

## 9. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 私有接口无契约 + 无上游修复通道 | golden fixtures 回归 + 版本探测 + 归档二进制应急 |
| 设备/订阅路由（receive-device 白名单） | 复现 `sha256(machine_id+"onchainos")` 派生；P2a 用真信号验证 |
| 会话与 seed 安全 | 落盘加密同现网（scrypt+AES-256-GCM），seed 用后 zeroize |
| CN 网络 | DoH 或 Clash 代理 |
| 闭源消息层导出面不足 | bridge 内退回 spawn `okx-a2a` |
| ToS/合规 | 仅自有账号、不绕风控、不改服务端 |
| 非产品路径能力缺口 | 明确不在范围内（OQ-1/OQ-8）；真需要时用归档二进制手工处置 |

---

## 10. 与现有资产的关系

- `scripts/watch-host.py`：输入源 → gateway 事件流（JSONL schema 不变）；
- `policy-engine.py` / `sub-collect.py` / `decision-loop.py` / `executor-lite.py`：零改动；
- `docs/design/01-10`：结论不变。

---

## 11. P0 实现记录（2026-09-30）

**产出**：`gateway/`（`okxai` 包，Python 3.11 + `cryptography`）+ 入口 `python -m okxai <verb>`（形态 A）
与 `python -m okxai serve --port 8788`（形态 B，stdlib http.server，仅监听回环）。

**验收**：`gateway/tests/contract_golden.py` → **16/16 PASS / 0 FAIL / 0 ERROR**
（对照 golden 的业务字段；忽略 CLI 自加工展示层、非确定性游标、CLI 自检器措辞）。
单 verb 热路径 **95–260ms**（对照 CLI 同命令 p50 614ms、冷启实测 21.5s）。

**验证到"不需要 CLI"这件事本身**：keyring 解密、HTTP 鉴权、token 续期、16 个读 verb
全部在无 CLI 进程参与的情况下跑通（仅复用 CLI 已写好的本地状态与 DoH 节点缓存）。

### 11.1 实现期发现的四个关键事实（都已写进代码注释与 gateway/README）

1. **凭据在 Windows 凭据管理器里，不在 `keyring.enc`**：CLI 优先读 OS keyring
   （target `agentic-wallet.onchainos`），`keyring.enc` 只是旧回退副本且内容已过期；
   只读文件会稳定得到 `10008 access token invalid`。gateway 按同序读取，写回时两处同步。
2. **token 续期要求"整套匿名头"**：refresh 虽声明为公开端点，但缺 `device-id` 等头会返回
   `HTTP 400 50113 Client signature public key missing`（服务端按 device-id 找回设备公钥）。
   补上 `ok-client-version` / `Ok-Access-Client-type` / `platform` / `device-id` / `device-name` 后成功。
3. **DoH 节点是"SNI 路由"的**：TCP 打节点 IP、TLS SNI = 节点 host（`web3.ynhf1jp.com`）、
   Host 头 = `web3.okx.com` 才通（curl 实测 HTTP 200；换 SNI 立即失败）。节点来自 CLI 已缓存的
   `doh-cache.json`；**再发现机制尚未实现**（P1 补），失效时提示可用 `OKXAI_HTTP_PROXY` 或让 CLI 刷新缓存。
4. **展示字段必须自建**：`statusName` / `statusDescription` / `approvalLabel` / `isThisDevice` /
   `lastOnlineLocal` / 订阅行的 `thisDeviceReceives` 都不在服务端响应里，是 CLI 本地算的。
   gateway 用 `labels.py` + 行级加工复刻（与 golden 逐字段一致）——这也再次印证 §3 的判断：
   "功能层面 API"本就在客户端，搬运行时即可，**后端零改动**。

### 11.2 P0 的偏差与已知缺口（如实记录）

- `agent.profile` / `agent.search`：CLI 4.5.2 的参数面与 dev 文档不同，未能录制基准
  （exit 2），P0 未实现对应 verb；需要时按服务端接口补。
- `task.active` 的非空分支（有未终态任务时的 `tasks[]` 标注）只实现了形状，**未经真实数据校验**。
- `gate.check` 的通信项固定为 not-ready（消息面 P2b 才接入），因此 `ready` 语义与 CLI 一致但成因不同。
- DoH 节点再发现、写路径（P1）与消息面（P2）未做。

---

## 12. v1.2 边界修订：签名/钱包/登录留在官方 CLI（OQ-19）

**用户决策（2026-09-30）**：私钥管理、钱包、登录、签名保持现状 CLI，不算改造范围；
本方案只管 **okx.AI 任务与订阅流程**。

### 12.1 执行体划分（谁干什么）

| 层 | 归属 | 说明 |
|---|---|---|
| 会话/钱包/私钥/签名/广播 | **官方 CLI（保留）** | 宿主侧内部调用；agent 不可见。链上写操作的真实执行者 |
| 任务/订阅**流程**：路由、状态机、nextAction、文案、幂等、事件、审计 | **gateway**（本仓库） | 原 CLI 的 L3，也是产品差异化的所在 |
| 查询（读） | **gateway 直连 HTTP** | P0 已完成：16 verb / 16 契约通过 |
| 无签名的后端写（设备路由、离线标记、可见性、serviceParam） | **gateway 直连 HTTP** | 本来就是纯 REST，无需 CLI |
| 需要签名的写（创建任务/订阅、接单、交付、验收、退款、评分、收益领取…） | **gateway 组参 → CLI 执行 → gateway 规范化** | 等价于早先对 okx-a2a 的"包装不改造" |

### 12.2 对早先结论的影响（如实修订）

- §4.1/§8 里"运行依赖 = 0 个 CLI"**仅对 agent 侧成立**；**宿主侧仍需官方 CLI**（写路径）。
  这一点此前写在 v1.1，现按新边界显式修正。
- **收益不变的部分**：agent 侧不再需要二进制/全局安装；skill 瘦身（-92%）不变；
  身份/心跳类调用（实测占 58%）被 gateway 的会话缓存消除这一项**完全不受影响**。
- **收益收窄的部分**：写路径不再验证"去 CLI"，而是"CLI 只做执行器"。
  上游漂移风险随之**下降**：接口变化时，被委托的命令跟着上游升级即可修复，
  我们在写路径上没有自建协议实现。
- **已投入且保留的资产**：`gateway/okxai/crypto.py`（HPKE/Ed25519/EIP-191，真机 sign-msg 验证通过）
  与 `tests/probe_signing.py`。当前**不作为主路径**，作为"将来若要彻底去 CLI"的现成地基保留。
- **契约测试反而更强**：CLI 仍在本地，写路径可以直接"gateway 结果 vs CLI 原生命令"双跑对照。

### 12.3 P1 写 verb 与执行体

| verb | 执行体 | 对应命令/端点 |
|---|---|---|
| `task.create` | CLI | `agent create-task` |
| `task.accept`（买家确认并付款） | CLI | `agent set-payment-mode` → `agent confirm-accept` |
| `task.apply` / `task.deliver`（ASP） | CLI | `agent apply` / `agent deliver` |
| `task.complete` / `task.reject` / `task.close` | CLI | `agent complete` / `reject` / `close` |
| `task.claimAutoRefund` | CLI | `agent claim-auto-refund` |
| `sub.create` | CLI | `agent create-subscribe` |
| `sub.cancel` / `sub.autorenew` | CLI | `agent subscribe-cancel` / `start-autorenew` |
| `sub.device` / `sub.offline` | gateway（HTTP） | `.../subscribe/{subId}/` 设备与离线标记端点 |
| `sub.aspClaim` / `sub.agreeRefund` / `sub.dispute` | CLI | `agent subscribe-asp-claim` / `subscribe-agree-refund` / `subscribe-dispute` |
| `refund.*` | CLI | `agent refund-prepare` / `refund-execute` / `refund-confirm` |
| `rating.submit` / `feedback.submit` | CLI | `agent feedback-submit` / `task-feedback` |
| `task.visibility` / `task.setAsp` / `task.userReject` | gateway（HTTP） | 对应的 off-chain 端点 |

### 12.4 P1 进展（2026-09-30）

**已实现**：`gateway/okxai/delegate.py`（CLI 委托执行器：argv 列表传参、命令白名单、
统一超时、UTF-8 解码、审计）+ `gateway/okxai/verbs_write.py`（19 个写 verb，见 §12.3 表）。

**已实测验证（不花钱、可逆）** — `gateway/tests/probe_writes.py` → PASS：
- `sub.offline` 真实往返：写 1 → 服务端读回 1；写 0 → 读回 0（真状态变更，非自述）
- `sub.device` 清空语义幂等
- 委托执行器：`agent get-my-agents` 经委托成功（154ms，executor=cli:*）；越权命令被拒绝
- 传输层补强：GET 自动重试 2 次 + 节点再发现（重读缓存 / 可选 `OKXAI_ALLOW_CLI_DOH_REFRESH=1` 让 CLI 刷缓存）
- P0 契约回归：**16/16 仍全过**

**踩坑（已固化为语义约定 + 测试）**：
- `deviceList: null` = **所有设备都收**；`deviceList: []` = **没有任何设备收**（会静默停掉该订阅的信号投递）。
  官方 CLI 把"空/省略"解释为清空；gateway 采用同一约定，并把"真正的空列表"改为显式 `explicit_empty=True` 才发出。
  ⚠️ 这条直接影响产品主线的信号交付，属于必须回归验证的字段。
- `setOfflineReceiveFlag` 的字段名是 `offlineReceiveFlag`（传 `flag` → `code=1001`）。

**待办**：需要签名的写操作（`task.create` / `sub.create` 等）尚未做**付费端到端**验证；
CLI 4.5.2 的 `create-task` 参数面与 dev 文档（4.6.x）不同——以实装 CLI 为准（`gateway/reference/cli-help.txt`）。

### 12.5 P1 验收受阻：版本闸门 + Windows 平台限制（2026-09-30 实测链）

一次真实付费 E2E 尝试，暴露了两道**上游强制的门槛**（都不是 gateway 自身问题）：

1. **写操作有客户端版本闸门**。CLI 4.5.2 的 `create-task` 在创建任何东西之前被后端拒绝：
   `code=1001 OnchainOS update required before task creation can continue`（读操作不受影响）。
   → 已按官方路径升级到 **4.6.3**（GitHub release + `checksums.txt` sha256 校验通过，
   `77d9fe87…`；安装器自带的 PowerShell 下载在本机失败，改用 curl 走官方 release）。
   **升级后读路径契约零漂移：golden（4.5.2 录制）仍 16/16 全过** —— 直连 HTTP 的读层是版本稳健的。
2. **4.6.x 的创建流程要求把任务绑定到本地 AI 运行时**：`okx-a2a job-provider bind-current`。
   CLI 内部给该调用约 5 秒，本机每次 `create-task` 都在这一步超时：
   `cannot bind task to the current AI runtime; creation was not broadcast`。
   根因是 **Hermes 的 okx-a2a 网关插件在 Windows 上无法安装**（官方明确：
   `okx-a2a setup hermes` → *"Hermes setup/update is not supported on Windows yet. The Hermes
   gateway installer requires bash. Use WSL, or run setup on macOS/Linux, or choose the
   codex/claude provider on Windows."*），没有插件 → 守护进程无法确认 provider readiness →
   绑定调用超时。手工执行同一条 `bind-current` 会立即返回 `already_bound`（但 readiness 仍为空）。

**未产生资金动作**：每次失败都在"broadcast 之前"中止。副作用是留下 3 条 `status=-1`（INIT）
未出资任务记录（`0x151334bc…`、`0x7dd798b4…`、`0x5ffe5d58…`）；V2 任务禁止直接 `close`
（`direct close is disabled for V2 tasks`），且无资金可退，属惰性记录。

**写路径 verb 已按 4.6.3 参数面更新**（`--provider-agent-id/--payment-token-*` 取代 4.5.2 的
`--provider/--budget`），参考面重抓至 `gateway/reference/cli-help.txt`。

**结论（对方案的影响）**：v1.2 的"签名类写操作委托 CLI"决策**被这次实测反向证实是必要的**：
后端用版本闸门强制客户端保持最新，任何自建客户端都得跟着服务端节奏走；把写路径留给官方 CLI，
等于把"跟随上游"这件事继续外包给上游自己。**Windows 上的写路径解除阻塞需要三选一**（见 OQ-20）。
