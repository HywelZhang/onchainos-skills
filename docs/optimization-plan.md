# onchainos-skills fork 优化方案（dev 分支）

> 状态: 草案 v0.1（2026-09-03，待用户确认后定稿）
> 基线: fork main == upstream main @ 17daea5（2026-09-02 sync）；本地 Hermes skills/CLI = v4.5.2（最新 stable）
> 目标: 让 OKX.AI / onchainos 任务流程在本地（Hermes + CN 网络 + 中文）更顺畅
> 非目标: 不做后端/合约层改动；不破坏官方安装渠道兼容性（除非决策点确认放弃）

## 1. 架构速览

```
┌─────────────────────────────────────────────────────┐
│ 宿主层: Hermes / Claude Code / OpenClaw / ...        │  ← 我们在这
│   - skills/  = markdown 说明书（路由表/gates/模板）    │
│   - workflows/ = 编排文档（INDEX.md 路由 → 分步文档）  │
├─────────────────────────────────────────────────────┤
│ CLI 层: onchainos (Rust, 单一二进制)                  │
│   - 真实执行: token/wallet/task/agent/defi/payment   │
│   - gate-check / next-action: 生成 playbook 文本      │
│     硬编码引用 skills/.../references/*.md 路径        │
│   - 内嵌 MCP server；内置 doh/ (DNS 解析辅助)         │
├─────────────────────────────────────────────────────┤
│ 后端: OKX 黑盒 API + XLayer 链上合约（不可改）         │
└─────────────────────────────────────────────────────┘
```

体量事实（上下文成本的第一手证据）:
- skills/*/SKILL.md: payments 36KB, dapp-discovery 28KB, okx-ai 21KB, wallet 10KB, dex-market 7KB, defi 6KB, guide 1.5KB
- okx-ai references/ 合计 428KB；单文件最大: task-cli-reference 58KB, task-user-playbook 37KB, watch-core 31KB, identity-register 30KB
- 中文 = 仅 7 个 keyword-glossary（关键词映射），无任何中文模板/规则

## 2. 问题清单

| # | 问题 | 证据 | 影响 | 拟定方案 | 层 |
|---|------|------|------|---------|----|
| P1 | 单流程上下文超重: 主 SKILL(21KB) + 30KB+ 参考文件/次 | du 数据见上 | 慢、贵、易截断 | 量化基线后瘦身/分层；高频流给 lite 入口 | L1 |
| P2 | 中文是外挂: 全英文模板 + 语言锁现场翻译 + 禁混语 | okx-ai SKILL.md §Language Lock | 每轮翻译心智 + 出错源 | 中文变体覆盖高频流；CLI Label 中英对照小表 | L1 |
| P3 | 路由三处漂移: SKILL 表格 / workflows INDEX / CLI playbook 硬编码路径 | pending_v2.rs 输出 watch-core.md 路径 | 文件移动即断链 | 路径稳定性=红线；收敛文档路由到一处 | L1 |
| P4 | watch/常驻流程在 Hermes 会话难维持: 超时重进/stale-wake/重启恢复全靠 agent 自觉 | watch-core 31KB + wake-scheduling/background-recovery | 漏事件、重进失败 | 宿主封装: watch-host 脚本 + cron/后台保活 | L1+宿主 |
| P5 | 版本三件套耦合: skill frontmatter ↔ CLI ↔ 服务端(taskMinVersion/version_notice) | preflight --skill-version; version_notice.rs | fork 改 skill 后版本线混乱 | fork 独立版本线(如 4.5.3-zh.1)，记录配套 CLI | 流程 |
| P6 | 本地安装/同步链路坏: hermes skills install 丢 _shared/；手拷易漏；现装 4.5.2 落后 fork | china-network ref（已验证） | 断链 skill 静默失效 | sync-to-hermes 脚本(整目录 cp + 文件数校验 + 备份) | 工具 |
| P7 | CN 网络无文档: GitHub raw 不稳、okx 域名污染、每新终端重设代理 | 记忆+setup ref | 安装/升级卡死 | CN 快速上手文档；脚本内 export；验证 CLI 是否遵守 HTTPS_PROXY | L1+工具 |
| P8 | 配额撞墙无引导: 免费市场数据配额(MARKET_API_*_OVER_QUOTA) | setup ref（实测过） | 流程卡死无下一步 | 错误码→行动项映射文档；可选 dex-market 错误处理补丁 | L1 |
| P9 | 防呆护栏过重(自用场景): 卡片确认/one-call rule/禁 jq/禁 poll | okx-ai SKILL.md §Gates | 每写操作多 1-2 轮往返 | 产出可放宽清单 vs 不可动清单；自用 lite profile 试点 | L1 |
| P10 | 事件流每轮注入大段英文 playbook，中文场景还要翻译 | asp/flow.rs 内嵌 KB 级模板 | 高频事件链上下文膨胀 | 待量化后: 模板压缩或 --format 精简；宿主缓存 jobId 上下文 | L2 |

## 3. 红线与约束（改动前必读）

1. skills/**/references/*.md 的相对路径被 CLI playbook/gate 硬编码引用——改名/移动必须同步改 Rust 源码，否则提示断链。默认：只增不改路径。
2. SKILL.md frontmatter 的 name/description 是 skill 路由与"何时加载"的依据，description 改动会影响所有宿主的选择逻辑——改 description 要全量回归。
3. 协议/资金/评审员/争议/订阅信号相关规则不改（对外履约与链上状态机耦合）。
4. _shared/ 目录必须整目录随 skill 安装（本地同步脚本强制校验文件数）。
5. 上游 sync 策略: main 分支只吃 upstream merge；改动全部走独立分支合入 dev；与上游同名文件的冲突要小步解、带 diff 记录。
6. 所有改动可回滚: commit 粒度小 + 本地 Hermes skills 同步前先备份。

## 4. 分阶段路线图

### P0 — 基线与工具（0.5~1 天，零产品逻辑改动）
- [x] 设计文档五件（docs/design/ 01-04 + docs/optimization-plan.md）: 节点清单(含 55 事件全名单+使用审计)、信号 schema、policy 配置、用户旅程
- [x] sync-to-hermes 脚本（scripts/sync-to-hermes.sh + .ps1）: 整目录 cp + 文件数校验 + 备份(备份目录在 skills 根之外)；注意: git-bash 下 LOCALAPPDATA 反斜杠会破坏 compgen，已用 ${VAR//\\//} 归一化 + find 探测修复
- [x] CN 快速上手文档（docs/cn-quickstart.md）: 代理 env、安装/升级、同步、配额
- [x] scripts/audit-events.py: 事件使用审计工具(上游 sync 后重跑)，结果 54 变体: 36 active / 18 rust-only / 0 dead
- [x] 本地切换: Hermes skills 4.5.2 → fork 4.5.3 产物（diff 校验 IDENTICAL）；原 4.5.2 可随时从官方 v4.5.2 tag 恢复
- [x] 行为验证: ① preflight 4.5.3 skill + 4.5.2 CLI → action:null（版本配对宽容, 领先一个 patch 无碍）② 无代理 preflight 通过 → CLI 内置 DoH, API 流量免代理（代理只用于安装/更新/网页）③ preflight 会在后台清理官方废弃 skill（okx-how-to-play/okx-ai-guide/okx-x402-payment 等名单已捕获）
- [~] 量化基线: 静态完成(单流程文档加载 47-104KB vs 优化后 1-2KB; 回合数见 04)；实时耗时需钱包登录会话, 待有登录态时补测

### P1 — 文档层优化（L1，方向修订: 主体保持英文 + 精简/分层 + i18n 渲染层，不做全文中文变体）
- [x] 决策: skill 主体保持英文（指令精度/多模型鲁棒/sync 零冲突）；中文只用于 keyword 触发、示例、用户可见文案 i18n
- [x] 试点流(订阅信号处理): task-subscription-signal.lite.md 协议卡(13KB, 原 25.7KB, 硬规则零丢失: 非可信输入边界/流程/终态/桥/consent/排队续跑) + labels.zh-CN.md 渲染表 + SKILL.md 挂载(lite 默认, 歧义升全量)
- [x] 其余高频流同法精简(委派 3 并行子任务, 均零规则丢失审计):
  - identity-register.lite.md 36.8KB(合并 register+invariants 两文件 56.1KB→单次加载, 省 34.5%; 卡骨架/#id ladder/词表/A2MCP·A2A 规则/不可信字段全部保留)
  - task-user-playbook.lite.md 29.2KB(原 37KB, 省 21%; 18 个 §锚点原样保留, SKILL 路由 §链接不受影响)
  - watch-core.lite.md 21.9KB(原 31.2KB, 省 30%; wake prompt/时序守卫/6 条反模式/停止条件全保留)
  - 经验: 规则密集型文档精简到 55% 会丢规则——实际 65-79%, 保规则优先; 合并多文件单次加载是更有效的省法
- [x] labels.zh-CN.md 扩展: §8 身份域 + §9 订阅管理/设备域
- [ ] 主 SKILL 瘦身试点: okx-ai 21KB → 目标 ~10KB（可折叠内容下沉 references）
- [ ] 防呆护栏可放宽项试点（自用 lite profile，不动协议规则）
- 验收: 对照 P0 基线，所选流程「加载 KB / 往返次数 / 出错率」下降，中文输出零混语

### P2 — 宿主与 CLI 层（按需，L1+L2）
- [x] watch-host 骨架（docs/design/06-watch-host.md + scripts/watch-host.py）: 包装 okx-a2a CLI（OQ-3 结论: a2a-node 闭源不可改，包装不改造）；停止条件/中间态/去重按 watch-core 规则编码；离线自测 8/8 PASS；OQ-4: 通知=console
- [ ] watch-host 真机验证: 需 okx-a2a 安装(npm i -g @okxweb3/a2a-node) + 邮箱/钱包登录态 + 活跃订阅事件流
- [ ] supervisor 选型（OQ-12: 建议 cron 心跳 --once）
- [ ] policy 引擎: 事件队列 → events.<wire>/nodes.<id> 决策(03 schema) + 执行桥接入
- [ ] 任务高频链: next-action 模板压缩评估 / 宿主缓存 jobId 上下文
- [ ] MCP 通道评估: 类型化工具 vs 读 md 敲 CLI 的收益实测
- 验收: watch 连续 24h 无漏事件；高频链 token 减半（实测）

## 5. 开放决策点（请 1~6 作答）

1. 常用场景排序（决定 P0/P1 先做谁）: a) OKX.AI 身份/任务市场（发任务/接单/评审） b) watch/订阅信号 c) 链上只读研究(dex-market) d) wallet/swap/defi
2. 中文版策略: A) 新增 zh 变体文件（保住上游 sync，推荐） B) 直接改上游同名文件为中文（sync 每轮手动解冲突） C) 先只做对照小表+文档
3. 是否必须保持官方渠道可安装（npx skills / claude plugin / marketplace）？若只 Hermes 自用，结构自由度更大
4. L2(Rust) 可接受度: 0) 不碰 Rust 1) 只加脚本级工具 2) 可改 Rust（需本机 Rust 工具链）
5. 是否申请 OKX API key（https://web3.okx.com/onchain-os/dev-portal）？影响 P8 优先级
6. watch/常驻需求: 需要 Hermes cron/后台保活实现吗？

## 6. 附录: 常用验证命令

```
onchainos --version                        # CLI 版本
onchainos preflight --skill-version 4.5.3  # preflight 门（data.action=null 为通过）
diff -rq <hermes skills>/okx <fork>/skills # 同步完整性
find skills/okx-ai/references -type f | wc -l   # 文件数校验
git fetch upstream && git merge upstream/main    # 上游同步
```
