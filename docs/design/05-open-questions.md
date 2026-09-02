# P0-05 开放问题清单（待用户确认）

> 维护: 自主开发期间遇到需确认的点只记录于此，不阻塞其它明确任务。用户在方便时逐条回复。

## OQ-1 使用场景优先级（影响后续流程精简顺序）
P0 时未收到回复。已按假设执行: 订阅信号处理(copy-trade 主线)优先。假设: a 任务市场/身份 > b 订阅信号 > c 只读研究 > d wallet。若实际不同请纠正——影响 P1 剩余流顺序（当前: identity-register → user-playbook → watch-core）。

## OQ-2 OKX API key
是否申请（https://web3.okx.com/onchain-os/dev-portal）？免费市场数据配额撞墙（MARKET_API_*_OVER_QUOTA）后没有 key 只能付费或用沙箱内置 key。影响 P8 处理与真实环境测试能力。

## OQ-3 watch/常驻实现
订阅信号/watch 守护需要宿主（Hermes 后台进程/cronjob）配合保活与事件分发。是否需要我实现 watch-host 封装（P2）？还是仅文档？

## OQ-4 通知通道
渲染与提醒的 notifyTo 目标: console / telegram / 其它。Hermes cronjob 的 deliver 只能到 gateway 平台（如 telegram），CLI 本身只发事件。需要定宿主适配器形态。

## OQ-5 L2(Rust) 改动意愿
是否允许改 Rust CLI（复合命令 / next-action --json / watch 行为 / policy 命令）？本机 Rust 工具链是否可用？决定 P2 深度。

## OQ-6 fork push 凭据
本地 git 身份已设: HywelZhang / HywelZhang@users.noreply.github.com（仓库级，可改）。推送 origin 需要 GitHub 凭据（gh auth / PAT / SSH）。dev 已有 3 个 commit 未推送。是否配置推送？

## OQ-7 hook 接口形态（产品化关键决策）
目标2 的 pre/post hook 用哪种约定对外暴露: A) 目录约定 + YAML 配置（scripts/ 白名单，当前设计，最简单） B) npm/pip 插件包 C) 本地 daemon API。影响 docs/design/03 与后续 SDK/文档。

## OQ-8 评审员(evaluator)角色 v1 范围
已按 v1 不做处理（01 附录 A: 14 事件不建节点，官方 CLI 兜底）。确认保持？若要做仲裁托管需重新规划。

## OQ-9 上游文件分歧策略
为挂载 lite 默认路径，已在 okx-ai/SKILL.md 做小补丁（fork-default 段落）。策略: SKILL.md/references 允许最小分歧（每 sync 冲突手动解），其余文件零改动。确认该策略或要求更严格（全部 overlay，不动上游文件）？

## OQ-10 实时耗时基线
静态基线已完成；实时耗时需钱包登录会话（QR 扫码需人在场）+ 一次真实订阅信号流。等有登录态时补测。

## OQ-11 中文 labels 覆盖范围
labels.zh-CN.md 目前挂在 okx-ai/references/。是否需要推广到其它 skill（wallet/defi/dex-market 的用户可见文案同样有翻译负担）？默认: 先只做任务/订阅域（okx-ai），其余按使用频率跟进。
