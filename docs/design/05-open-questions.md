# P0-05 开放问题清单（待用户确认）

> 维护: 自主开发期间遇到需确认的点只记录于此，不阻塞其它明确任务。状态随用户回复更新。

## ✅ 已解决

### OQ-1 使用场景范围 — [已答 2026-09-03]
只优化 okx.ai 功能域（单次任务 + 订阅任务场景）。不扩展到 wallet/defi/dex-market 流程优化。
含义: P1 文档层剩余 okx-ai 主 SKILL 瘦身完成后即收尾；其它 skill 的 lite 化/i18n 不做；市场数据配额（OQ-2）问题随之出范围。

### OQ-2 OKX API key — [已答 2026-09-03]
不需要 API Key；账户后续通过邮箱登录 onchainos。
含义: 身份注册/任务/订阅为链上(XLayer)操作，不走市场数据配额；研究类(配额敏感)非目标。onchainos 邮箱登录路径待验证（当前 CLI 登录是 wallet QR/浏览器流，邮箱登录的具体命令/流程在需要时再查）。

### OQ-5 L2(Rust) 改动意愿 — [已答 2026-09-03]
允许改 Rust。⚠ 本机暂无 cargo/rustup —— L2 开工前置: 安装 Rust 工具链（Windows 默认 MSVC target，需 Visual Studio Build Tools 或选 GNU 工具链），约 1-2GB，动手前确认安装方式。

### OQ-6 GitHub 推送凭据 — [进行中 2026-09-03]
Git Credential Manager 2.9.1 已配置为全局 credential.helper；等首次 `git push` 触发浏览器授权（OAuth/device 流）后即完成。完成后 dev 分支（当前领先 origin/main 7 个 commit）可推送。

### OQ-10 实时耗时基线 — [已解决·部分]
静态基线完成；实时耗时需钱包/邮箱登录态 + 一次真实订阅信号流，待有登录态时补测（OQ-2 的邮箱登录路径可用后即可做）。

## ❓ 仍待确认

### OQ-3 watch 常驻实现范围
问题已讲解（见会话）: 官方现状 = agent 会话内长轮询（无头常驻需自建 watch-host）。是否要我实现 watch-host 封装（后台 okx-a2a user watch --json + 自动重进 + 事件落盘，供 policy 引擎消费）？还是先保持官方 agent-in-loop 用法？

### OQ-4 通知通道
渲染/提醒的 notifyTo: console / telegram / 其它。Hermes cronjob deliver 只能到 gateway 平台；CLI 只发事件。宿主适配器形态待定。

### OQ-7 hook 接口形态（产品化关键）
pre/post hook 对外暴露约定: A) 目录约定+YAML（scripts/ 白名单，当前设计） B) npm/pip 插件包 C) 本地 daemon API。影响 docs/design/03。

### OQ-8 评审员(evaluator) v1 范围
默认保持 v1 不做（01 附录 A）。若做仲裁托管需重新规划。

### OQ-9 上游文件分歧策略
默认: SKILL.md/references 允许最小分歧（fork-default 段落），每 sync 手动解冲突；其余文件零改动。已按此执行（okx-ai/SKILL.md 两处小补丁 + 4 个 lite 新文件）。确认或收紧？

### OQ-11 中文 labels 覆盖范围
labels.zh-CN.md 挂在 okx-ai/references/。默认只做任务/订阅域；其余 skill 按使用频率跟进（OQ-1 收窄后: 大概率不做）。
