# P0-12 写路径迁移到 Linux/WSL（OQ-20 选 2）

> 状态: v0.1（2026-09-30）。决策: 用户拍板 OQ-20 选项 **2 —— 写路径搬到 Linux/macOS/WSL 执行**。
> 依据: 同一台 Windows 上 `okx-a2a setup hermes` 被官方拒绝（"Hermes setup/update is not supported
> on Windows yet. The Hermes gateway installer requires bash."），导致 4.6.x 的
> `job-provider bind-current` 超时 → `create-task` 无法完成（未广播、未花钱）。
> 关联: docs/design/11 §12.5（实测链）、05 OQ-20。

## 1. 现状侦察（本机实测）

| 项 | 结果 |
|---|---|
| WSL | **未安装**（`wsl.exe --status` → 未安装用于 Linux 的 Windows 子系统） |
| Docker | 未安装 |
| 虚拟化固件 | 已启用（True） |
| 系统 | Windows 10 Home China（支持 WSL2） |
| 结论 | 用选项 2，需要**管理员权限 + 重启**装 WSL；或改用一台真正的 Linux 机器 |

## 2. 目标架构（谁在哪儿跑）

```
Windows 侧（现状，继续可用）                     Linux/WSL 侧（新增，写路径）
  agent + skill（okx-ai）                           okxai gateway（会话/流程/委托）
  读路径 gateway（16 读 verb，已验证）      ──────▶  onchainos CLI（签名/广播/钱包）
  契约测试 / golden 基准                            okx-a2a runtime（Linux 官方支持 setup）
  golden/reports 归档                               事件与投递（P2a 轮询可在任一侧）
```

两种落地方式，**推荐 A**：

| 方式 | 做法 | 优点 | 代价 |
|---|---|---|---|
| **A. gateway 主体搬进 WSL**（推荐） | WSL 里跑 `python -m okxai serve`，agent 通过 `http://127.0.0.1:8788` 访问（WSL2 的 localhost 转发） | 一个登录态、一个 device-id、读写同一套会话；Windows 侧不再需要 gateway | 需要在 WSL 里重新 `wallet login`（凭据不跨 OS 迁移） |
| B. 双跑（Windows 读 + WSL 写） | Windows gateway 保留读，写 verb 通过 `wsl.exe onchainos ...` 转发 | Windows 现有会话不用动 | 两套登录态/设备、跨边界进程调用、写路径延迟+故障面变大 |

方式 A 的额外好处：Windows 上那条"版本闸门 + 插件不支持"的坑在 Linux 侧不存在（官方支持 bash → Hermes 插件可装）。

## 3. 需要你执行（管理员 + 重启）

```powershell
# 管理员 PowerShell（会提示重启；Win10 Home 支持 WSL2）
wsl --install -d Ubuntu
# 重启后首次进入 Ubuntu 设置用户名/密码
```

若机器策略不允许装 WSL，则改用一台 Linux 主机（步骤 4 同样适用；把仓库 clone 过去即可）。

## 4. 自动化部分（已就绪，脚本已提交）

在 Linux/WSL 里、仓库根目录执行：

```bash
bash scripts/setup-linux-writepath.sh            # 自动：Node 检查 → 下载 CLI(GitHub release + sha256 校验) → 装 okx-a2a → 自检
```

脚本刻意**不**代做两件事（脚本结尾会打印）：

1. `onchainos wallet login`（社交登录，需浏览器；WSL 不会弹窗 → 把 URL 复制到 Windows 浏览器打开）
   建议同时 `export ONCHAINOS_FORCE_FILE_KEYRING=1`，摆脱对桌面 keyring 服务的依赖（headless 更可预测）。
2. `okx-a2a setup hermes --json` + `okx-a2a doctor --fix --json`（Linux 上支持 bash → 插件可装，期望 `ready:true`）。

然后跑三项既有验证（gateway 与 CLI 必须同机同用户）：

```bash
PY=$(command -v python3)
$PY gateway/tests/probe_signing.py     # HPKE/Ed25519 签名链（真机 sign-msg）
$PY gateway/tests/probe_writes.py      # 无签名写路径往返（离线标记/设备列表）
$PY gateway/tests/contract_golden.py   # 读路径契约（对照 4.5.2 录制的 golden，期望 16/16）
```

## 5. 花钱的验收（P1 收口）

```bash
# 0.1 USDT/轮（已批预算）。先用 service.match 取一个 ≤0.1 USDT 的服务与其 aspAgentId。
$PY -m okxai task.create \
  --title="Gateway P1 端到端验证" \
  --description="自动化链路验证：150 字以内的健康晚餐建议（含 3 个要点）" \
  --provider_agent_id=<ASP> --service_id=<SERVICE_ID> \
  --service_token_address=0x779ded0c9e1022225f8e0630b35a9b54be713736 \
  --service_token_amount=0.1 --payment_token_symbol=USDT --payment_token_amount=0.1 \
  --service_params='{"topic":"健康晚餐建议"}'
```

验收口径（缺一不可）：
1. 返回带 `jobId` 且 `task.status` 从 `init` → `created`（**出资已在链上广播**，可核对 txHash）；
2. 重复调用**不产生第二次出资**（幂等）；
3. ASP 接单/交付路径可被观察（`task.active` / `sub.*` / 事件流）；
4. 对照一个"写操作 golden"（改造前 CLI 单轮写操作的输出形状），业务字段一致。

## 6. 迁移注意点（照抄，别踩）

- **新设备 = 新 device-id**：`device/id.rs` 的 id 由 `sha256(machine_id + "onchainos")` 派生，WSL 是另一台"机器" → 需要重新 `wallet login`，且会在订阅的接收设备表里多出一台设备。
  订阅的 `deviceList` 语义（Windows 上已实测）：**`null` = 所有设备都收；`[]` = 没有任何设备收**。
  迁移后若要"只在 WSL 收"，写 `deviceList=["<wsl-device-id>"]`，并用 `sub.list` 的 `thisDeviceReceives` 复核。
- **凭据不跨 OS**：Windows 的 token 在凭据管理器里，WSL 读不到；不要试图复制 `keyring.enc`（其 KDF 身份文件是各机独立的 `machine-identity`）。
- **golden 基准仍是 4.5.2 录的**：Linux 侧升级到 4.6.3 后读路径契约已实测零漂移（16/16），但若再出现偏差，先判断是"服务端变更"还是"版本差异"，再决定是否补录基准。
- **网络**：WSL 访问 GitHub/npm 可能与 Windows 不同（CN 下视 Clash 配置）；若 WSL 里不通，用 mirrored 网络模式或让镜像走宿主机代理。GitHub 访问在本机是间歇性的，脚本已内置 3 次重试。
- **不要在 Windows 侧再跑写 verb**：4.6.x 的写路径在这台 Windows 上必然卡在 `bind-current`，只会在服务端留下 `status=-1` 的未出资记录（当前已有 3 条）。
