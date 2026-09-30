# P0-11 CLI → 直连 API 重构方案（okx-ai 去 CLI 化）

> 状态: **v1.0 已定稿**（2026-09-30，OQ-13..17 已答，见 §0 决策表）。
> 范围: okx-ai 域（单次任务 + 订阅任务 + 身份），不动 wallet/defi/dex-market 的日常流程。
> 依据: 本仓 `cli/src/**`（Rust 源码可读）+ 本机 `@okxweb3/a2a-node@0.2.10` 实测 + `skills/okx-ai` 全量统计 + 本机 `~/.onchainos/audit.jsonl`（3,123 条真实调用记录）+ 本次实机跑的只读命令。

---

## 0. 决策表（本轮用户确认）

| OQ | 决策 | 影响 |
|---|---|---|
| OQ-13 消息面 | **C + A**：轮询优先覆盖订阅信号；a2a/XMTP 保留兜底交互聊天 | P2 两个子任务；`okx-a2a` 不再是日常路径依赖 |
| OQ-14 运行形态 | **B 主 + A 轻量**：本地 HTTP 服务（宿主侧守护）+ 仓库内库脚本 | agent 侧只需 `fetch`；无全局安装 |
| OQ-15 语言栈 | **Python 主 + Node 仅 XMTP** | 业务层与 scripts/*.py 同栈；Node 退化为一个 bridge 进程 |
| OQ-16 CLI shim | **保留** `onchainos`（回退 + 契约测试基准） | 每阶段可回退；diff 回归有基准 |
| OQ-17 P2 范围 | **交互式 + 信号都做** | P2 = 轮询信号通道 + XMTP 交互（经 Node bridge） |

---

## 1. 现状拆解：okx-ai 今天依赖什么

skill 文本是指挥层，执行体是**两个 CLI**：

```
skills/okx-ai (dev 分支: 37 文件 / 499,220 字符 ≈ 142k tokens)
 ├─ onchainos           Rust CLI（本仓 cli/ 源码可读，agent 命名空间 107 子命令）
 │   ├─ L1 HTTP    web3.okx.com  /priapi/v1/aieco/*（约 30 端点）
 │   │                    + /priapi/v5/wallet/agentic/*（约 20 端点：会话/签名/广播）
 │   ├─ L2 会话     JWT + refreshToken、sessionCert、agenticId 头、device-id 头
 │   │              （/auth/refresh 续期；社交登录 = 本地生成 authSessionId + X25519 tempPubKey
 │   │               → 打开 /account/sociallogin → 2s 轮询 /auth/session/result）
 │   ├─ L2 密码学   HPKE(DHKEM(X25519,HKDF-SHA256)+HKDF-SHA256+AES-256-GCM, info="okx-tee-sign")
 │   │              解出 32B Ed25519 seed → 本地签名 → gen-msg-hash → sign-msg → broadcast
 │   └─ L3 业务/状态 nextAction 信封、statusLabel 文案、状态机、幂等、audit.jsonl
 └─ okx-a2a            @okxweb3/a2a-node（Node，闭源 dist bundle，@xmtp/node-sdk 6.1.0）
     ├─ XMTP 消息面   user watch / session send / xmtp-send / user check
     └─ 反向依赖      shell 调用 onchainos（agent list / session / version）
```

skill 实际用到的命令：**onchainos 57 个 + okx-a2a 12 个**（其余是上游备用面）。

### 1.1 决定后续判断的实测证据

| 事实 | 证据 | 含义 |
|---|---|---|
| 服务端不返回业务文案 | `statusLabel/statusDescription` 在 `identity/utils.rs` 本地映射 | 渲染层在客户端，搬走不丢语义 |
| 服务端不返回流程信封 | `nextAction` 在 `a2mcp_probe/flow.rs`、`receive.rs` 本地组装 | "功能层面流程"本就在客户端 |
| 交付物读本地盘 | `deliverables.rs` → `~/.onchainos/deliverables/`（本机 262KB） | 迁移期保持目录布局 |
| 无本地私钥 | `crypto.rs:20-98`、`sign.rs:119-154`：TEE 托管，客户端只签 msgHash | 自建层不需要 KMS/HSM |
| 附件面已是 HTTP | `chat/mod.rs:10-14` `im/attachments/xmtp/encrypted/{upload,download}` | 只有"消息收发"在 XMTP |
| 消息面无 HTTP 端点 | aieco 端点清单只有 eligible / system-config / risk | 交互聊天必须 XMTP |
| 设备身份是本地派生 | `device/id.rs`：`sha256(machine_id+"onchainos")`；订阅有 receive-device 白名单 | 自建层须复现同一派生，否则收不到信号 |
| 上游带 DoH 故障转移 | `cli/src/doh/**` | CN 环境需 DoH 或代理 |
| **CLI 输出里带 LLM 指令** | 实跑 `agent pending-decisions-v2 list` 输出含 `"Render the line above to the user as your assistant response."` | 提示词工程目前在命令行输出里；gateway 必须显式接管这一职责 |

---

## 2. 可行性矩阵

| 能力 | 今天靠 | 纯 HTTP | 成本 | 备注 |
|---|---|---|---|---|
| 登录/续期/设备身份 | onchainos | ✅ | 低 | 全 HTTP，authSessionId 本地生成 |
| 查询（任务/订阅/服务/身份/评价） | onchainos | ✅ | 低 | ~15 个读端点 |
| 身份注册/更新/激活 | onchainos | ✅ | 中 | 需广播（签名链同下） |
| 创建任务/订阅并出资 | onchainos | ✅ | 中高 | createAndFund → 签名 → broadcast |
| 接单/交付/验收/拒绝/退款/评分 | onchainos | ✅ | 中高 | 幂等 + 双签（complete/reject） |
| nextAction / 文案 / 状态机 | onchainos 本地 | ✅（移植） | 中 | 开源可读，逻辑迁移非重写 |
| 附件上传/下载 | onchainos HTTP | ✅ | 低中 | 会话密钥加密须一致 |
| 订阅信号交付 | okx-a2a 长轮询 | ✅（轮询） | 中 | `task/inProgress` + `subscribe/{id}` + 本地 deliverables |
| 交互式聊天/澄清/peer | okx-a2a + XMTP | ❌ | 高 | 保留 Node bridge |

**比例**：约 70% 可 100% 直连 HTTP；约 20% 需本地移植（逻辑可读）；10% 依赖 XMTP（本方案降级为"保留 Node bridge"，不再要求 agent 侧 CLI）。

---

## 3. 核心判断：要不要"后端包装更多功能层 API"？

**不需要改后端。** "功能层面 API"今天就是客户端逻辑（§1.1 前两条），把它搬到本地转换层是**同架构换运行时**；服务端本来只提供资源级端点。反向证据：若这些逻辑在后端，CLI 不必有 107 个子命令和 13k 行 agent_commerce 代码。

---

## 4. 目标架构（v1.0 定稿）

```
        ┌──── agent 环境（任意 harness，只需能发 HTTP 或跑 python）────┐
        │  skill: skills/okx-ai/  →  路由 + 信封契约 + verb 表（≤800 行）│
        └──────────────┬────────────────────────────┬─────────────────┘
            形态 B: HTTP 127.0.0.1:8788        形态 A: python scripts/okxai/cli.py <verb>
                       ▼                            ▼
   ┌───────────────────────── okxai gateway（Python，宿主侧常驻）─────────────────────────┐
   │ session  登录/续期/sessionCert/agenticId/device-id（复用 ~/.onchainos 会话与加密布局）  │
   │ crypto   X25519+HPKE 解 seed、Ed25519 签名、附件密钥（std lib 级：cryptography/pyhpke）  │
   │ domain   task / subscription / identity / refund / rating 状态机（移植 cli/src 语义）   │
   │ envelope nextAction + statusLabel + 本地化模板 + 幂等键 + audit.jsonl                   │
   │ transport HTTP → web3.okx.com/priapi/*（DoH 或 Clash 代理）                             │
   │ inbox    P2a 轮询（inProgress/subscribe/deliverables）→ 事件 JSONL（06 §4 契约不变）    │
   │          P2b 交互：内部调用 ▼                                                          │
   └───────────────────────────────┬───────────────────────────────────────────────────────┘
                                    ▼
              xmtp-bridge（Node 单进程，宿主侧）: 优先 import @okxweb3/a2a-node 的 dist/index.js
              （导出面不足时退回同进程内 spawn `okx-a2a`）→ 对 gateway 暴露 stdio JSON-RPC
```

边界原则：**Python 负责一切业务与状态；Node 只在"必须 XMTP"时被调用**，且对 agent 不可见。

### 4.1 形态（含新增形态 D）

| 形态 | 交付 | agent 需要 | 用途 |
|---|---|---|---|
| **B**（主） | `okxai serve --port 8788`，宿主侧守护 | 只 `fetch` localhost | 只允许 HTTP 工具的 agent |
| **A** | `python scripts/okxai/cli.py <verb>` | 能跑 python，无全局安装 | 沙箱可用解释器 |
| **D**（新增，非受限环境） | 现有 `onchainos`（必要时走 MCP：repo 已有 `mcp::serve()`，2980 行） | 允许装一次 CLI/MCP 的环境 | Hermes 等自控环境**直接用官方 CLI**，不重复实现 |
| C（回退） | `onchainos` / `okx-a2a` 原样保留 | 同今天 | 迁移期、契约测试基准、非产品路径命令 |

形态 D 是本轮新增的收敛点：**重实现只为真正受限的环境服务**，把上游漂移风险限制在最小 scope（见 §8）。

### 4.2 功能级 verb 面（26 个，只覆盖产品路径）

读（P0）：`session.status` · `agent.mine/get/search` · `service.match/list` · `task.mine/detail/inProgress/deliverables` · `sub.list/detail/cost` · `feedback.list` · `flow.next`

写（P1）：`task.create` · `task.accept/decline` · `task.deliver` · `task.complete/reject`（双签） · `task.claim` · `sub.create/cancel/autorenew/device/offline` · `refund.prepare/execute/confirm` · `rating.submit` · `service.paramUpdate`

消息（P2）：`watch.poll` · `msg.send/session` · `user.notify` · `file.up/down`

**非产品路径命令**（evaluator 质押/仲裁/投票、x402、跨链等 ~30 个）：**不进 gateway**，需要时经形态 D/C 走官方 CLI。这把重实现面从"107 命令"压到"26 verb"。

统一信封与今天一致（`phase/decision/reason/nextAction/payload`），skill 渲染规则与本地化规则不变（含"只呈现 actionLabel、不暴露 action id"）。

---

## 5. skill 瘦身（实测基线 → 目标）

dev 分支现状：**37 文件 / 499,220 字符**（≈142k tokens）。其中纯 CLI 机械面：

| 文件 | 字符 | 判定 |
|---|---|---|
| task-cli-reference.md | 58,221 | 纯 CLI 语法参考 → **删除** |
| task-user-playbook.md + .lite | 66,463 | 命令序列 playbook → 压缩为语义 |
| identity-register.md + .lite | 67,797 | 注册流程 + 命令 → 压缩 |
| watch-core.md + .lite | 53,289 | 长轮询机制 → 改为 poll/daemon 说明 |
| chat-cli-reference.md + chat-comm-init.md | 6,981 | 命令面 → 删除 |
| watch-outdated-list / watch-wake-scheduling / watch-background-recovery | 9,316 | 运行时排障 → 归 gateway |
| **小计** | **262,067（52%）** | |

目标：`SKILL.md`(≤8KB) + `references/verbs.md`(≤6KB) + `references/decisions.md`(≤5KB) + `task-subscription-signal.md` 压缩版(≤6KB) + `task-core.md`(≤5KB) + `labels.zh-CN.md`(5.5KB) ≈ **≤40KB 全量**（-92% 体量）。

单次订阅事件的加载量（关键指标）：今天 = SKILL.md + intent-routing + subscription-signal + watch-core.lite + labels ≈ **77–90KB（≈22–26k tokens）**；改后 = SKILL.md + verbs + 信号语义 ≈ **20KB（≈5.7k tokens）→ -75%**。

---

## 6. 量化提升评估（实测基线）

### 6.1 真实调用数据（`~/.onchainos/audit.jsonl`，2026-09-02..04，3,123 条）

| 指标 | 实测 |
|---|---|
| 每次 CLI 级调用延迟 | p50 **614ms** / p90 **3,154ms** |
| 每次 api 级调用延迟 | p50 **365ms** / p90 1,150ms |
| 最慢业务命令 | create-task p50 **6,993ms**、user-notify p50 **4,225ms**、service-match 1,076ms、next-action 1,038ms |
| 成功率 | **95.0%**（155 条失败 → 触发重试/恢复流程） |
| **身份/心跳类开销** | heartbeat 808 + agent get 776 + get-my-agents 121 + get-agents 109 = **1,814 次 = 58% 的总调用** |
| 冷启动（本次实跑） | `agent get-my-agents` **21,532ms**（DoH 冷路径）；同命令热路径 p50 535ms |
| 单次输出体量（本次实跑） | 85B（本地决策列表）～ 3,708B（身份列表）；`my-subscriptions` 1,996B；`status` 196B |
| 输出里的隐藏成本 | CLI 在 stdout 里塞 LLM 指令（"Render the line above to the user…"） |

### 6.2 提升预期（按维度）

| 维度 | 现在 | 改后 | 量级 |
|---|---|---|---|
| skill 体量 | 499KB / 37 文件 | ≤40KB / ≤6 文件 | **-92%** |
| 单事件上下文 | 77–90KB（22–26k tokens） | ~20KB（~5.7k tokens） | **-75%** |
| 每流程 CLI 调用数 | 100% 走 CLI，其中 58% 是身份/心跳 | 0（读/写直连）；心跳由 daemon 承担，身份本地缓存 | **-55~60% 调用数** |
| 单调用延迟 | p50 614ms，冷启 21.5s | 单跳 HTTP/进程内；无 spawn、无 DoH 冷启重复 | 去 spawn 与冷启路径 |
| 失败面 | 5% 调用失败并触发恢复 | 幂等键 + 单跳重试（无 spawn/解析失败面） | 目标 <1%（P1 实测） |
| 部署要求 | 2 个 CLI（Rust 二进制 + Node 包） | agent 侧 0 依赖；宿主侧 1 Python 守护 + 可选 Node bridge | 满足"不放二进制" |
| 确定性执行 | policy 引擎已 0 LLM（实测 4/4） | 写操作也进确定性层，auto 路径同样 0 LLM | 保住既有数量级优势 |

**不提升的部分（如实标注）**：链上/服务端延迟、LLM 内容质量、LLM 交付物生成、CN 网络抖动。这些不在本方案范围。

### 6.3 验证方式（沿用既有纪律）

P0/P1 每阶段用 `hermes -z + --usage-file` 跑 **CLI 版 vs gateway 版同任务 A/B**（token/成功率/耗时三列出表），真实付费预算 ≤0.1 USDT/轮（已批）。

---

## 7. 分阶段计划与验收

| 阶段 | 内容 | 验收（必须真实执行） |
|---|---|---|
| **P0**（1–2 天） | session + 14 读 verb + **契约测试骨架** | gateway 输出与 `onchainos` 输出逐字段 diff = 0，报告落盘 |
| **P1**（3–5 天） | 12 写 verb + 签名链 + 幂等 + audit + 本地化模板接管 | 真实付费端到端 ≥1 轮；重复调用不二次出资；状态可在链上/服务端核对 |
| **P2a**（2–3 天） | 轮询版 inbox（inProgress/subscribe/deliverables）→ 事件 JSONL | 真实订阅收到 ≥1 真信号，watch-host/policy-engine 零改动可用 |
| **P2b**（2–4 天） | xmtp-bridge（Node）接管交互聊天/澄清/user-notify | 与 CLI 版行为对照：同一条 `msg.send` 送达且 `session history` 一致 |
| **P3**（2–3 天） | skill 按 §5 瘦身 + A/B 对照 | 单事件 token 实测下降 ≥60%；同任务成功率不低于 CLI 版 |

回退：任一阶段失败 → 形态 C 原路径继续可用（shim 保留）。

---

## 8. 是否最优：对照方案与残差

### 8.1 对照

| 方案 | 满足"agent 侧无二进制" | 重实现成本 | 结论 |
|---|---|---|---|
| 本方案（Python gateway + Node bridge） | ✅ | 中（26 verb） | **在约束下最优** |
| 直接用官方 CLI（现状） | ❌ | 0 | 受限环境不可用 |
| **MCP 模式**（repo 已有 `mcp::serve()`） | ❌（仍需装二进制） | 0 | **在允许装的环境里性价比最高 → 定为形态 D** |
| 只做只读直连、写操作留 CLI | 部分 | 低 | 保留不了"确定性自主执行"这一核心卖点 |
| 全量重实现 107 命令 | ✅ | 高 | 浪费；已收敛到 26 verb |
| 重写 XMTP 客户端 | ✅ | 很高 | 无必要；Node bridge 已满足约束 |

### 8.2 结论（诚实版）

- **在"agent 侧不能出现二进制 + 要确定性自主执行 + 要 token 下降"这三条约束下，本方案接近最优。**
- **严格说不是全局最优**：全局最优是**按环境分派**——允许装的环境（Hermes 自己）继续用官方 CLI/MCP（形态 D），只有受限环境走 gateway。v1.0 已这样收敛，代价是维护两套路径（用契约测试兜住）。
- 真正的成本中心不是代码量，而是**上游漂移**：`/priapi/*` 无契约承诺，服务端改接口只同步官方 CLI。所以 **契约测试（P0 交付物）+ 保留 shim（OQ-16）不是可选项，是方案成立的前提**。没有这两样，本方案的长期收益会从"提升"退化成"负债"。
- 第二个残差：P2b 的 Node bridge 仍依赖闭源 bundle 的导出面（可能只有 CLI 面可用），最坏情况退化成"同进程 spawn okx-a2a"——仍满足"agent 侧无二进制"，但不是最优雅形态。

---

## 9. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 私有接口无契约 | 契约测试 + 版本探测 + 形态 C/D 回退 |
| 设备/订阅路由（receive-device 白名单） | 复现 `sha256(machine_id+"onchainos")` 派生并持久化；P2a 用真信号验证 |
| 会话与 seed 安全 | 与现网同构落盘加密（scrypt+AES-256-GCM），seed 用完 zeroize |
| CN 网络 | DoH 或 Clash 代理 |
| 闭源消息层导出面不足 | bridge 内退回 spawn `okx-a2a`（Node 进程内，agent 不可见） |
| ToS/合规 | 仅自有账号、不绕风控、不改服务端；边界写入文档 |
| 双路径维护成本 | 形态 D 承担非产品路径；gateway 只维护 26 verb |

---

## 10. 与现有资产的关系

- `scripts/watch-host.py`：输入源从 okx-a2a stdout → gateway 事件流（JSONL schema 不变）；
- `policy-engine.py` / `sub-collect.py` / `decision-loop.py` / `executor-lite.py`：**零改动**；
- `docs/design/01-10` 结论不变（本方案是执行层重构）。
