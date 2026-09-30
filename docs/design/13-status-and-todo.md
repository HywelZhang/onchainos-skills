# P0-13 状态 / 执行计划 / TODO（可交接）

> **给接手的 agent**：这是 okx-ai「去 CLI 化」重构的**唯一权威进度文档**。先读 §1 摘要与 §4 环境事实，
> 再按 §7 的优先级挑活。配套细节在 `docs/design/11-cli-to-api-refactor.md`（方案与实测）与
> `docs/design/12-linux-write-path.md`（写路径迁移）。
> 更新日期: 2026-09-30。工作分支: `dev`（`main` 只吃上游 merge，不要往 main 提交）。

---

## 1. 一句话摘要

**读路径已完成并验证（16/16 契约通过，agent 侧不需要二进制）；写路径代码已完成但端到端未验收
（Windows 撞上游平台限制，需在 Linux/WSL 收口）；消息面（P2）与 skill 重写（P3）尚未开始 ——
因此"装 dev 分支就能跑新流程"目前**不成立**，现在装上去跑的仍是 CLI 驱动的旧流程。**

## 2. 目标与边界（v1.2 决策，勿擅自扩大）

- **做什么**：把 okx-ai 域（单次任务 + 订阅 + 身份）的**流程层**从官方 CLI 迁到本仓库的
  `gateway/`（Python），使 agent 侧只需 HTTP / 解释器，不需要任何二进制。
- **不做什么（OQ-21 已定）**：私钥管理、钱包、登录、**签名/广播**保持官方 CLI，不在改造范围；
  gateway 只管 okx-ai 任务与订阅流程。需要签名的写操作 = gateway 组参 → **CLI 执行** → gateway 规范化。
- **不动**：wallet / defi / dex-market 的业务域；上游 skill 目录结构（最小分歧 + `<!-- FORK -->` 标记）。

## 3. 当前状态

| 组件 | 状态 | 证据（可复跑） |
|---|---|---|
| P0 读路径（16 个 verb） | ✅ 完成 | `python gateway/tests/contract_golden.py` → **16 PASS / 0 FAIL / 0 ERROR**（对照 4.5.2 录制的 golden；升级到 4.6.3 后**零漂移**） |
| 无签名写（`sub.device` / `sub.offline`） | ✅ 完成 | `python gateway/tests/probe_writes.py` → PASS（写 1→服务端读回 1；写 0→读回 0） |
| 委托执行器（CLI 作执行体） | ✅ 完成 | probe_writes 内含：`agent get-my-agents` 委托成功 154ms；越权命令（wallet transfer）被拒 |
| 签名链（HPKE/Ed25519/EIP-191） | ✅ 已实现且真机验证 | `python gateway/tests/probe_signing.py` → PASS（服务端接受我们的 personalSign）**注：当前非主路径**，因签名按 v1.2 留给 CLI |
| P1 签名类写（`task.create`/`sub.create`/验收/退款/评分…共 19 verb） | ⚠️ 代码完成、**端到端未验收** | 见 §6 阻塞；每轮失败都在广播前中止（**未花钱**） |
| P2a 轮询 inbox（订阅信号交付） | ❌ 未开始 | 产品主线，优先级最高未完成项之一 |
| P2b XMTP bridge（交互聊天/通知） | ❌ 未开始 | 依赖 @okxweb3/a2a-node（闭源 bundle）导出面 |
| P3 skill 重写（指向 gateway） | ❌ 未开始 | 现在 `skills/okx-ai` 仍是 37 文件 / 499KB 的 CLI 版，**没有任何一处提到 gateway** |
| 新流程安装/升级文档 | ❌ 未开始 | `docs/INSTALL.md` 讲的是"装 CLI + 用 CLI skill" |
| 网关分发形态（仓库内运行 / 可安装包） | ❌ 未定 | 目前只能 `cd gateway && python -m okxai …` |

## 4. 环境事实（别重复踩）

- **CLI**：已升级 **4.6.3**（2026-09-30；4.5.2 被后端版本闸门拒写：`code=1001 OnchainOS update required`）。
  原二进制归档：`$ONCHAINOS_HOME/archive/20260930/`（sha256 与 CLI 自记 `binary_identity.json` 一致）。
- **okx-a2a**：0.2.16（原 0.2.10）；daemon 曾被 `doctor --fix` 拉起（daemon ready、agent_refresh pass）。
- **Python 解释器**：需 `cryptography`（Windows 上本机用
  `%LOCALAPPDATA%\hermes\hermes-agent\venv\Scripts\python.exe`，3.11 + cryptography 50.x）。
  其它环境 `uv pip install cryptography` 或 pip 即可；gateway 只用标准库 + cryptography。
- **凭据顺序**：**先 OS keyring，再文件回退**。Windows 上是凭据管理器 target `agentic-wallet.onchainos`，
  `keyring.enc` 是**过期副本**（只读它必然 `10008 access token invalid`）。Linux 上建议
  `export ONCHAINOS_FORCE_FILE_KEYRING=1` 让 CLI 用文件库，gateway 侧同一份。
- **网络**：本机 `web3.okx.com` DNS 被污染（169.254.0.2）；gateway 复刻 DoH 节点策略
  （TCP 打节点 IP + SNI=节点 host + Host=web3.okx.com）。GitHub 访问**间歇性**（`ls-remote` 成功、
  `push` 失败交替出现）；npm registry 可达。环境变量：`OKXAI_HTTP_PROXY`、`OKXAI_ALLOW_CLI_DOH_REFRESH=1`、`OKXAI_ONCHAINOS_CLI`。
- **golden 基准**：原始录制（含账号数据，**不入库**）在 `$ONCHAINOS_HOME/golden/cli/20260930-180440/`；
  仓库内脱敏版 `tests/cli-golden/{manifest.redacted.json,README.md,contract-p0-report.redacted.json}`；
  契约报告落 `$ONCHAINOS_HOME/golden/reports/contract-*.json`。

## 5. 命令速查

```bash
HPY="<带 cryptography 的解释器>"        # Windows 见 §4；Linux 用 python3
cd gateway

$HPY -m okxai list                      # 35 个 verb（16 读 + 19 写）
$HPY -m okxai task.mine                 # 形态 A：命令行
$HPY -m okxai serve --port 8788         # 形态 B：本地 HTTP（agent 只需 fetch）
#   GET  http://127.0.0.1:8788/v1/<verb>?arg=value
#   POST http://127.0.0.1:8788/v1/<verb>  -d '{"kwargs":{...}}'

# 三项回归（改动后必须全过）
$HPY tests/contract_golden.py           # 期望 16 PASS / 0 FAIL
$HPY tests/probe_writes.py              # 期望 PASS
$HPY tests/probe_signing.py             # 期望 PASS

# 升级 CLI（后端版本闸门要求跟上游；GitHub 间歇，脚本带重试）
bash scripts/setup-linux-writepath.sh --version v4.6.3
```

## 6. 当前阻塞与已知问题

1. **Windows 上签名类写操作不可用（上游限制）**：
   4.6.x 的 `create-task`/`create-subscribe` 要求 `okx-a2a job-provider bind-current`，
   而 Hermes 的 okx-a2a 网关插件官方**不支持 Windows**（`okx-a2a setup hermes` →
   *"Hermes gateway installer requires bash … or choose the codex/claude provider on Windows"*），
   绑定在 CLI 的 ~5s 超时内无法完成 → `creation was not broadcast`（**不花钱**）。
   **决策（OQ-20）**：写路径搬到 **Linux/WSL**（手册 `docs/design/12` + 脚本 `scripts/setup-linux-writepath.sh`）。
   本机 WSL **未安装**，需要管理员执行 `wsl --install -d Ubuntu` 并重启。
2. **副作用记录**：3 条 `status=-1`（INIT）未出资任务（`0x151334bc…`、`0x7dd798b4…`、`0x5ffe5d58…`）；
   V2 任务禁止直接 `close`（`direct close is disabled for V2 tasks`），且无资金可退 → 惰性记录。
3. **DoH 节点再发现**：目前只用 CLI 写好的 `doh-cache.json`；节点失效时靠 GET 重试 + 可选经 CLI 刷缓存
   （`OKXAI_ALLOW_CLI_DOH_REFRESH=1`）。真正的自主再发现未实现。
4. **版本闸门是长期风险**：后端会随上游推进最低客户端版本；写路径因委托 CLI 而自动跟随，
   但**读路径**若遇接口变更只能靠 golden 回归发现（现阶段读接口在 4.5.2→4.6.3 之间稳定）。
5. **并发协作**：同一仓库有另一条工作线在持续提交（Janus 告警取证 `evidence: …`）。
   改文档编号前先 `grep` 一下 05 的 OQ 序号（我已被撞过一次：v1.2 边界决策从 OQ-19 改成 OQ-21）。

## 7. TODO（按优先级；每项给出验收口径）

### P3-A. 重写 okx-ai skill 指向 gateway（最高优先，解锁"装 dev 即用新流程"）
- 做什么：`skills/okx-ai/SKILL.md` + verb 表 + 决策语义（保留语义判断，删除 CLI 参数 playbook）。
  目标体量 ≤800 行（现状 6220 行 / 499KB），单事件加载 77–90KB → ~20KB。
- 验收：新 agent 仅凭 dev 分支的 skill + 一份安装文档，能完成"查任务/查订阅/改设备/改离线标记"；
  被删掉的 CLI 细节（`task-cli-reference.md` 58KB 等）有明确替代入口。
- 依赖：P3-B（安装文档）与 §8 的形态决策。

### P3-B. 新增"新流程"安装/升级文档
- 做什么：`docs/INSTALL-GATEWAY.md`（或 INSTALL.md 增章）：依赖（解释器+cryptography、CLI 已登录）、
  网关启动方式、自检命令、常见故障（凭据顺序、DoH、版本闸门）；`AGENTS.md` 加入口。
- 验收：在一台干净机器上照文档执行，三条回归命令全绿。

### P3-C. 分发形态落地
- 选项 A：仓库内运行 + 宿主守护（现状即可，文档写清 systemd/计划任务/后台启动）；
- 选项 B：打成可安装包（`uv`/`pip` 一行装），skill 里直接 `okxai …`。
- 验收：目标机器上"一条命令起服务 + 一条命令自检"。

### P2a. 轮询版 inbox（产品主线：订阅信号交付）
- 做什么：轮询 `task/inProgress` + `subscribe/{id}` + 本地 `deliverables/`，归一化成
  `docs/design/06` §4 的 JSONL 事件契约，喂给既有 `scripts/watch-host.py` / `policy-engine.py`（**这两个脚本零改动**）。
- 验收：真实订阅收到 ≥1 条真信号，归一化落盘且 policy 判定通过。
- 备注：这条同时解决另一工作线记录的"试用取消 cron 未触发 → 6 USDT 真实扣费"与"信号陈旧度"问题（OQ-19）。

### P2b. XMTP bridge（交互聊天/澄清/`user.notify`）
- 做什么：Node 单进程 bridge（优先 `import @okxweb3/a2a-node` 的 `dist/index.js`，导出面不足则同进程 spawn `okx-a2a`），
  对 gateway 暴露 stdio JSON-RPC；agent 不可见。
- 验收：同一条 `msg.send` 送达，且 `session history` 与旧流程一致。

### P1-E2E. 签名写路径端到端验收（等 Linux/WSL）
- 验收四条（缺一不可）：① 返回 `jobId` 且 `status` init→created（**可核对 txHash**）；
  ② 重复调用不二次出资；③ 接单/交付路径可观察；④ 与"写操作 golden"业务字段一致（该 golden 需在 Linux 上补录）。
- 预算：0.1 USDT/轮（已批）。

### P1-Test. 写路径契约基准
- 做什么：补录一轮**真实写操作**（Linux 侧，CLI 4.6.3）作为写路径 golden，并把断言加进 `tests/`。
- 验收：写 verb 的输出可与该基准逐字段对照。

### 其它（低优先）
- DoH 节点自主再发现（不再依赖 CLI 刷缓存）；
- `status=-1` 惰性记录的清理策略（或等上游放开 V2 close）；
- 监控后端最低客户端版本（写路径失败时优先怀疑版本闸门）。

## 8. 待决策（OQ-22，需用户拍板）

1. **agent 面向的调用形态**：本地 HTTP 服务（`127.0.0.1:8788`，宿主守护）为主 /
   `python -m okxai` 为主 / 两者都写进 skill。*倾向：HTTP 为主，命令行作兜底。*
2. **重写后的 skill 是否保留 CLI 回退路径**：保留（稳，但 skill 更厚，且旧流程在 4.6.x 上写路径本就断）/
   不保留（skill 最薄，但没 gateway 就完全不能用）。*倾向：不保留（回退交给"gateway 不可用时报错并指向安装文档"）。*

## 9. 接手第一步（照做即可）

1. 跑 §5 的三条回归命令 —— 期望 16/16 + 2×PASS（这是"没破坏既有能力"的判据）。
2. 读 `docs/design/11` §12（v1.2 边界 + P1 实测链）与 `docs/design/12`（写路径迁移）。
3. 选一条 TODO：**推荐先做 P3-A/P3-B**（不依赖 Linux/WSL，做完"装 dev 即用新流程"在读+无签名写范围成真），
   然后在 Linux/WSL 上做 **P1-E2E**，再做 **P2a**（产品主线）。
