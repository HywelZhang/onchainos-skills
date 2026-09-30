# cli-golden — P0-11 回归基准（录制型）

> 来源方案: `docs/design/11-cli-to-api-refactor.md` §4.4 / §7 P0.0
> 录制脚本: `scripts/record-golden.py`（可重复执行）

## 为什么要录

P0-11 让 okx-ai 不再运行官方 CLI。原验收是"gateway 输出 vs `onchainos` 输出逐字段 diff = 0"，
一旦停用 CLI 就无法再产生对照数据。因此**在停用之前**录一份真实输出作为不可再生的回归裁判。

## 基准事实

| 项 | 值 |
|---|---|
| CLI | `onchainos.exe` 4.5.2（`C:\Users\10518\.local\bin\`） |
| 二进制 sha256 | `b97dd05d06c343a23f1dfa7def2b88b5c3d27bc5891a42065fb98dde63d16a6d` |
| 该 sha 来源 | 与 CLI 自记的 `~/.onchainos/binary_identity.json`（2026-09-02）**一致**，即与产出 `audit.jsonl` 的那次运行是同一二进制 |
| 录制时间 | 2026-09-30 18:04 (+08:00) |
| 覆盖 | 18 条读路径命令，**16 条成功**（2 条因 4.5.2 参数形状不同失败，见下） |
| 原始输出位置 | `$ONCHAINOS_HOME/golden/cli/20260930-180440/`（**含账号数据，不进 git**） |
| 仓库内 | 本 README + `manifest.redacted.json`（命令、argv 脱敏、exit、耗时、字节数、stdout sha256） |
| 应急二进制 | `$ONCHAINOS_HOME/archive/20260930/`（onchainos 4.5.2 + a2a-node 0.2.10 dist + SHA256SUMS） |

> 版本提示: 本仓 `cli/Cargo.toml` 为 4.6.3，本机实装为 **4.5.2**（落后一版）。
> 基准以**本机实装**为准——它才是产出 `audit.jsonl` 与已验收流程的那个版本；gateway 只需对齐服务端接口，不必对齐 CLI 版本。

## 实测观察（录制时）

- 多数读命令热路径 **78–220ms**；`gate.check` 15.4s、`task.active` 10.1s（网络/冷路径）。
- 同一命令冷启动曾达 **21.5s**（`agent get-my-agents`），热路径 173ms —— spawn + DoH 冷路径成本，是 P0-11 的改善目标之一。
- 输出体量 63B–3.7KB；`flow.pending_decisions` 的输出里带**给 LLM 的渲染指令**（gateway 必须接管这部分职责）。

## 已知失效项（4.5.2 参数面）

| 命令 | exit | 原因 |
|---|---|---|
| `agent profile --agent-id 2366` | 2 | 4.5.2 的 `profile` 参数形状不同（usage error） |
| `agent search --keywords signal` | 2 | 同上，参数名不同 |

两项都是 CLI 参数面差异、非服务端问题；P0 实现这两个 verb 时以**服务端接口**为准，并在此处补记最终参数。

## 怎么用（回归裁判）

```bash
# 1) 复录（人工/临时，慎用；会覆盖新目录，不影响旧基准）
python scripts/record-golden.py --job-id <id> --sub-id <id> --agent-id <id>

# 2) P0 起用: gateway 的同一 verb 输出与 golden 逐字段比对
#    比对口径: 只比业务字段（业务字段集见 11 §4.2），忽略 CLI 的渲染文本与本地路径
#    断言: 业务字段 diff = 0；不一致项必须解释为"服务端已变更"并同步更新基准
```

比对口径由 P0 的契约测试骨架实现（`tests/`），必须脚本化、产出 JSON 报告，禁止目测。
