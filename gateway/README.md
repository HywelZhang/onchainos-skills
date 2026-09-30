# okxai gateway — 本地转换层

P0-11（`docs/design/11-cli-to-api-refactor.md` v1.1）的实现。把 okx-ai 域需要的
**会话 / 密码学 / HTTP / 状态 / 文案**从官方 CLI 迁到可移植运行时，
使 agent 侧**不再需要任何二进制或全局安装**。

```
agent（任意 harness）
  ├─ 形态 A: python -m okxai <verb> [--k=v]        ← 沙箱里能跑 python 就够
  └─ 形态 B: HTTP 127.0.0.1:8788/v1/<verb>          ← 只需 fetch/curl
         ▼
  okxai gateway（宿主侧，本包）
    home/keystore  ~/.onchainos 状态复用（OS 凭据管理器优先，keyring.enc 回退）
    transport      HTTPS：DoH 节点 IP 固定 + SNI=节点 host + Host=web3.okx.com
    session        JWT 生命周期 / refresh / agenticId 缓存 / device-id
    verbs          功能级 verb（读 P0 已完成；写 P1 / 消息 P2 待做）
    labels         状态→文案映射（skill 只呈现文案，不呈现 code）
```

## 运行

需要一个**带 `cryptography` 的解释器**。本机现成的：

```bash
HPY="C:/Users/10518/AppData/Local/hermes/hermes-agent/venv/Scripts/python"   # Python 3.11 + cryptography 50.x

# 形态 A
$HPY -m okxai list
$HPY -m okxai agent.flat
$HPY -m okxai task.status --job_id 0x...
$HPY -m okxai sub.cost

# 形态 B（宿主侧守护；agent 只需 HTTP）
$HPY -m okxai serve --port 8788
curl -s http://127.0.0.1:8788/healthz
curl -s "http://127.0.0.1:8788/v1/sub.cost"
curl -s -X POST http://127.0.0.1:8788/v1/task.status -d '{"kwargs":{"job_id":"0x…"}}'
```

> 若目标机器没有 `cryptography`：`uv pip install cryptography`（或 pip）。
> 包本身只用标准库 + `cryptography`，无其它依赖。

## 契约回归（P0 验收口径）

```bash
$HPY tests/contract_golden.py                    # 自动取最新 golden
$HPY tests/contract_golden.py --only task.status
```

对照基准是改造前录制的 CLI 输出（`tests/cli-golden/`，原始数据在
`$ONCHAINOS_HOME/golden/cli/<ts>/`）。口径：**golden 的业务字段必须全部在
gateway 输出中存在且相等**；忽略 CLI 自加工的展示层（`card`/`cells`/`statusLabel`…）、
非确定性字段（`searchAfter`）与 CLI 自检器措辞（`hint`）。
报告落在 `$ONCHAINOS_HOME/golden/reports/contract-*.json`。

**P0 结果（2026-09-30）**：16 条可比对 golden → **16 PASS / 0 FAIL / 0 ERROR**
（`agent.profile`、`agent.search` 因 CLI 4.5.2 参数面差异不可录，另行标注）。

## 已实现 verb（P0 读路径）

| verb | 说明 | 底层 |
|---|---|---|
| `session.status` | 本地会话/密码学/身份诊断（不联网） | 本地 |
| `session.devices` | 登录设备列表（含 `isThisDevice`/`local` 时间） | `GET /agent/device-list` |
| `wallet.status` | 账号 + 交易策略 | 本地 + `GET /policy/query` |
| `agent.mine` / `agent.flat` / `agent.get` | 身份查询 | `GET /agent/agent-list` · `/agent/batch-list` |
| `task.mine` / `task.active` / `task.status` | 任务查询 | `GET /task/my` · `POST /task/inProgress` · `GET /task/{jobId}` |
| `task.deliverables` | 交付物（读本地 manifest） | 本地 |
| `sub.list` / `sub.detail` / `sub.cost` | 订阅 | `GET /task/subscribe/my` · `/{subId}` |
| `service.match` | 市场服务检索 | `POST /task/asp/service/search` |
| `flow.pending_decisions` | 本地待决策队列 | 本地 |
| `gate.check` | 就绪度自检 | 组合 |

## 实现期踩到的坑（都已在代码里注释）

| 坑 | 现象 | 处理 |
|---|---|---|
| 凭据双份 | `keyring.enc` 里的 token 早已过期，CLI 却能用 | CLI 在 Windows 优先用**凭据管理器**（target `agentic-wallet.onchainos`）；gateway 同序读取，写回时两处都写（`wincreds.py`） |
| refresh 被拒 | `HTTP 400 50113 Client signature public key missing` | refresh 虽不要求 JWT，但要求**整套匿名头**（`device-id` 等）；服务端按 device-id 找回设备公钥 |
| DNS 污染 | `web3.okx.com` → 169.254.0.2，直连必失败 | 复刻 DoH 节点策略：TCP 打节点 IP、SNI=节点 host（`web3.ynhf1jp.com`）、Host=真实域名（curl 实测 200） |
| 单元素数组 | 服务端 `data` 常是 `[{...}]` | 复刻 CLI 的 `normalize_singleton_object` |
| 展示字段 | `statusName`/`approvalLabel`/`isThisDevice` 等不在服务端 | 在 gateway 内用 `labels.py` + 行级加工补齐（与 golden 逐字段一致） |
| 假通过 | 首版契约测试只读 manifest（无 stdout）→ 全 PASS 但 0 字段 | 改为读 `NN_<name>.json` 原始录制，并输出 matched 字段数 |

## 未做（后续阶段）

- **P1 写路径**：`task.create`（createAndFund → gen-msg-hash → sign-msg → broadcast）、
  `task.accept/deliver/complete/reject/claim`、`sub.create/cancel/autorenew/device`、
  `refund.*`、`rating.submit`。需要 `crypto.py`（HPKE 解 seed、Ed25519、keccak256+EIP-191）。
- **P2a 轮询 inbox** → 事件 JSONL（沿用 `docs/design/06` 的 schema）。
- **P2b xmtp-bridge**（Node 单进程）接管交互聊天 / `user.notify`。
- **DoH 节点再发现**：当前仅用 CLI 已缓存的节点；节点失效时的重新发现机制待实现（P1 期间补）。

## 签名链验证（P1 前置，已完成）

```bash
$HPY tests/probe_signing.py      # 不广播、不花钱；退出码 0 = 全通
```

覆盖：keccak256 向量 → HPKE 解 seed（库实现与手写实现互验）→
`pre-transaction/sign-msg` 接受我们的 `personalSign` 会话签名。

> HPKE 用 `cryptography.hazmat.primitives.hpke`（RFC 9180 原生，需 ≥45），
> 同时保留手写 Base 模式实现作为回退；两条路径已互验一致。
> 踩坑: HKDF-**Extract** 必须单独用 HMAC-SHA256（误用 `HKDF().derive()` = Extract+Expand 会 InvalidTag）。
