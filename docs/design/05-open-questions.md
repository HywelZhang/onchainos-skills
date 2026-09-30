# P0-05 开放问题清单（待用户确认）

> 维护: 自主开发期间遇到需确认的点只记录于此，不阻塞其它明确任务。状态随用户回复更新。

## ✅ 已解决

### OQ-1 使用场景范围 — [已答 2026-09-03]
只优化 okx.ai 功能域（单次任务 + 订阅任务场景）。不扩展到 wallet/defi/dex-market 流程优化。
含义: P1 文档层剩余 okx-ai 主 SKILL 瘦身完成后即收尾；其它 skill 的 lite 化/i18n 不做；市场数据配额问题随之出范围。

### OQ-2 OKX API key — [已答 2026-09-03]
不需要 API Key；账户已邮箱登录 onchainos（1051892427@qq.com，loggedIn）。身份/任务/订阅为链上(XLayer)操作，不走市场数据配额。

### OQ-3 watch 常驻 — [已答 2026-09-03]
okx-a2a(@okxweb3/a2a-node) 闭源不可改 → 包装不改造自建 watch-host（docs/design/06 + scripts/watch-host.py）。真机端到端已验证（2026-09-03，收到真实事件并归一化落盘）。待办: 长跑 ≥24h 验证（需活跃订阅/任务流）；supervisor 选型见 OQ-12。

### OQ-4 通知通道 — [已答 2026-09-03]
console（v1）。telegram 等留适配器扩展点（06 §5 --notify）。

### OQ-5 L2(Rust) 改动意愿 — [已答 2026-09-03 + 工具链已装]
允许改 Rust。✅ 工具链已装（2026-09-03）: rustup + stable 1.98.0 minimal profile, cargo/rustc 在 ~/.cargo/bin（PATH 未自动改, 用 ~/.cargo/bin/cargo 或自行加 PATH）。⚠ 首次 cargo build 需 MSVC link.exe（VS Build Tools C++ workload）——缺则装 vs_buildtools 或改用 GNU target。

### OQ-6 GitHub 推送凭据 — [已完成 2026-09-03]
GCM 2.9.1 + 浏览器授权完成；dev 与 origin/dev 同步。

### OQ-7 hook 形态 — [已答 2026-09-03: A]
采用 A: 目录约定 + YAML（scripts/ 白名单目录放可执行脚本，policy YAML 引用 pre: [scripts/xxx.py]；上下文走环境变量，退出码 0=通过/非零=veto）。理由: 用户多为小白，配置由 Agent 按自然语言代写——A 的约定（白名单目录 + 固定参数契约）对 LLM 生成/校验最不易错，无 SDK/依赖/版本负担；hook 本身小白不直接碰，Agent 代管。B/C 留作未来同一 YAML 接口下的 loader 扩展。

### OQ-9 上游文件分歧策略 — [已答 2026-09-03: 策略1]
最小分歧: 只新增文件 + 上游文件最小补丁；所有改动上游文件处打 `<!-- FORK -->` 标记，sync 冲突一眼可辨。已对 okx-ai/SKILL.md 补丁区补标记。

### OQ-10 实时耗时基线 — [部分]
邮箱登录态已就绪（OQ-2）。实时耗时/真实订阅信号流采样待有活跃订阅后补（与 watch-host 长跑验证同批）。

## ❓ 仍待确认

### OQ-8 评审员(evaluator) v1 范围
默认保持 v1 不做（01 附录 A）。若做仲裁托管需重新规划。

### OQ-11 中文 labels 覆盖范围
labels.zh-CN.md 挂在 okx-ai/references/。OQ-1 收窄后其它 skill 大概率不做。

### OQ-12 watch-host supervisor 选型 — [已答 2026-09-03: B]
cron 心跳: Windows 计划任务每 N 分钟跑 `watch-host.py --once`（scripts/install-watch-task.ps1）；会话级可用循环进程 `python scripts/watch-host.py`。事件延迟 ≤ 间隔；决策及时性要求高时间隔可 1 min。

### OQ-13..17 来源: P0-11 CLI → 直连 API 重构（docs/design/11-cli-to-api-refactor.md, 2026-09-30）

#### ✅ 已答 2026-09-30（P0-11 v1.0 定稿）
- OQ-13 = **C + A**（轮询优先覆盖订阅信号；okx-a2a/XMTP 保留为交互聊天兜底）
- OQ-14 = **B 主 + A 轻量**（本地 HTTP 服务 127.0.0.1:8788 宿主侧守护；仓库内库脚本作为轻量形态）
- OQ-15 = **Python 主 + Node 仅 XMTP**（Node 退化为宿主侧 xmtp-bridge 单进程，对 agent 不可见）
- OQ-16 = **保留** `onchainos`（迁移期回退 + 契约测试基准）
- OQ-17 = **交互式 + 信号都做**（P2 拆 P2a 轮询信号通道 / P2b Node bridge 交互聊天）
- 追加决策: 新增**形态 D**（非受限环境直接用官方 CLI / 已存在的 MCP serve），gateway 重实现面收敛到 **26 verb**，非产品路径命令不进 gateway。

### OQ-13 消息面（XMTP）选项
A 保留 okx-a2a（收窄为唯一 CLI 依赖）/ B 本地 Node 内嵌 a2a-node / C 轮询优先覆盖订阅信号 + A 兜底交互聊天。
方案倾向: C + A（P2 先做轮询版 inbox，因为订阅信号本质是交付物落地，HTTP+本地盘即可），B 作长期演进。

### OQ-14 gateway 运行形态
B 本地 HTTP 服务（127.0.0.1:8788，由宿主侧守护，agent 只需 fetch）/ A 仓库内库脚本（`node scripts/okxai/*.mjs`，无全局安装）/ A+B 双形态。
方案倾向: B 主用（对"只允许 HTTP 工具"的 agent 最友好），A 作为沙箱内解释器可用时的轻量形态。

### OQ-15 语言栈
Node（唯一能内嵌 XMTP 生态）/ Python（与 scripts/*.py 一致）/ 混合。
方案倾向: 混合——Python 做业务层/状态机/policy 对接，Node 仅在需要消息面时启用。

### OQ-16 是否保留 CLI shim
保留（迁移期回退路径 + 契约测试基准）/ 立即移除。
方案倾向: 保留（形态 C），契约测试直接以 `onchainos` 输出为基准比对，是最便宜的回归防线。

### OQ-17 P2 是否纳入交互式聊天
否，P2 只做"订阅信号交付 + 任务事件"（主线产品）；交互式澄清/peer chat 延后 / 是，一并做。
方案倾向: 否。
**已答 2026-09-30: 是——交互式 + 信号都做（P2 拆 P2a 轮询信号 / P2b Node bridge 交互）。**

### OQ-18 是否保留官方 CLI/MCP 兼容层 — [已答 2026-09-30: 不做兼容层]
决策: 不实现官方 CLI/MCP 并存形态（v1.0 的形态 C/D 取消），只实现自身方案（形态 A/B）；运行依赖 = 0 个 CLI。
必补两个替代物（否则验收退化为自证）:
1. **录制型 golden 基准**: `scripts/record-golden.py` 已录 16/18 条读路径命令 → 原始输出在 `$ONCHAINOS_HOME/golden/cli/20260930-180440/`（不入 git）；仓库内 `tests/cli-golden/manifest.redacted.json` + README（比对口径）。
2. **应急二进制归档**: `$ONCHAINOS_HOME/archive/20260930/`（onchainos 4.5.2 + a2a-node 0.2.10 dist + SHA256SUMS；二进制 sha256 与 CLI 自记 `binary_identity.json` 一致）。
收益: 无并行期 ⇒ 消除"同账号两客户端同时刷新 JWT 互相踢"的竞态；无映射层与双路径测试。
代价: 失去"上游改接口→升级 CLI 即修复"的通道 ⇒ golden 回归 + 版本探测升级为方案成立前提（docs/design/11 §8.2）。

### OQ-19 试用到期保护机制 + 信号陈旧度 — [待确认]
实测(2026-09-30 复盘): 09-06 两条试用 cancel cron 均未触发(host/调度器越过 120s 宽限被判"不会触发"移除), 两试用自动转付费共扣 5 + 1 = **6 USDT**(详见 10-buyer-subscription.md §3); 另有一份 09-03T17:10Z 快照直到 09-30 才补投(滞后 27 天, 客户端未打陈旧标记)。
待定: (a) 保护机制选型——周期轮询+补跑(catch-up) / 常驻 supervisor / 后端侧取消(与 OQ-13/14、P0-11 方向一致, 倾向轮询+补跑); (b) 时限余量(建议扣款前 ≥30min 触发且失败重试, 本例第二笔 18:20 本身就晚于扣款 18:17); (c) "信号陈旧度标记"是否列为客户端必备字段(建议是, content 时间戳 vs 本地时间)。
现状: 两订阅 autoRenew=0, 付费期至 2026-10-06; 是否现在显式 subscribe-cancel 由用户决定。


### OQ-20 Windows 上写路径解除阻塞（三选一）— 待用户决策
背景: CLI 4.6.3 的创建流程要求 `okx-a2a job-provider bind-current`，而 Hermes 的 okx-a2a 网关插件
在 Windows 上无法安装（官方: Hermes setup/update not supported on Windows yet, requires bash）→
每次 create-task 在绑定步超时（未广播、未花钱）。读路径与所有无签名写路径不受影响。
选项:
  A. 在 Windows 上把 okx-a2a 的 AI provider 切到 **codex 或 claude**（官方推荐的 Windows 路径）；
     需先安装对应 CLI（本机 codex/claude/gemini 均未安装）——会改变现有 A2A 会话由 Hermes 托管的现状。
  B. 写路径改到 **Linux/macOS（或 WSL）** 环境执行（技能/仓库可共用，宿主侧跑 gateway + CLI）。
  C. Windows 上暂时**只做读 + 无签名写**（当前已可用的 19 个写 verb 中，CLI 委托类暂不可用），
     等官方补齐 Windows 支持。
方案倾向: 需用户拍板（涉及运行时托管方式与机器，不宜自行决定）。
**已答 2026-09-30: 选项 2 —— 写路径搬到 Linux/macOS/WSL 执行。**
落地: docs/design/12-linux-write-path.md + scripts/setup-linux-writepath.sh（本机 WSL 未安装，需管理员+重启）。

### OQ-22 新流程的 agent 面向形态（待确认；详见 docs/design/13 §8）
1. 调用形态: 本地 HTTP 服务(127.0.0.1:8788, 宿主守护)为主 / `python -m okxai` 为主 / 两者都写进 skill。
   倾向: HTTP 为主, 命令行作兜底。
2. 重写后的 skill 是否保留 CLI 回退路径: 保留(稳但更厚; 旧流程在 4.6.x 上写路径本就断) / 不保留(最薄, 无 gateway 则不可用)。
   倾向: 不保留, 由"gateway 不可用时报错并指向安装文档"替代。
