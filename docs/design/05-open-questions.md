# P0-05 开放问题清单（待用户确认）

> 维护: 自主开发期间遇到需确认的点只记录于此，不阻塞其它明确任务。状态随用户回复更新。

## ✅ 已解决

### OQ-1 使用场景范围 — [已答 2026-09-03]
只优化 okx.ai 功能域（单次任务 + 订阅任务场景）。不扩展到 wallet/defi/dex-market 流程优化。
含义: P1 文档层剩余 okx-ai 主 SKILL 瘦身完成后即收尾；其它 skill 的 lite 化/i18n 不做；市场数据配额问题随之出范围。

### OQ-2 OKX API key — [已答 2026-09-03]
不需要 API Key；账户后续通过邮箱登录 onchainos。身份/任务/订阅为链上(XLayer)操作，不走市场数据配额。邮箱登录的具体命令/流程待需要时验证（当前 CLI 登录是 wallet QR/浏览器流）。

### OQ-3 watch 常驻 — [已答 2026-09-03]
评估结论: okx-a2a(@okxweb3/a2a-node) 为闭源 npm 包（本仓库只有调用代码），内部改造不可行 → 采用"包装不改造"自建 watch-host（docs/design/06 + scripts/watch-host.py 骨架已落，离线自测 8/8 PASS）。待办: 真机验证需 okx-a2a 安装 + 登录态；supervisor 选型（Hermes 后台 / cron 心跳 / 系统服务）未定。

### OQ-4 通知通道 — [已答 2026-09-03]
console（v1）。telegram 等留适配器扩展点（06 §5 --notify）。

### OQ-5 L2(Rust) 改动意愿 — [已答 2026-09-03]
允许改 Rust。⚠ 本机暂无 cargo/rustup —— L2 开工前置: 安装 Rust 工具链（Windows 默认 MSVC target，需 VS Build Tools 或选 GNU 工具链），约 1-2GB，动手前确认安装方式。

### OQ-6 GitHub 推送凭据 — [已完成 2026-09-03]
GCM 2.9.1 配置 + 浏览器授权完成；dev 已推送 origin/dev（0b72a175 起同步）。

### OQ-10 实时耗时基线 — [已解决·部分]
静态基线完成；实时耗时需邮箱/钱包登录态 + 一次真实订阅信号流，待登录态就绪补测（与 watch-host 真机验证同批）。

## ❓ 仍待确认

### OQ-7 hook 接口形态（产品化关键）
pre/post hook 对外暴露约定: A) 目录约定+YAML（scripts/ 白名单，当前设计） B) npm/pip 插件包 C) 本地 daemon API。影响 docs/design/03。

### OQ-8 评审员(evaluator) v1 范围
默认保持 v1 不做（01 附录 A）。若做仲裁托管需重新规划。

### OQ-9 上游文件分歧策略
默认: SKILL.md/references 允许最小分歧（fork-default 段落 + lite 新文件），每 sync 手动解冲突；其余文件零改动。已按此执行。确认或收紧？

### OQ-11 中文 labels 覆盖范围
labels.zh-CN.md 挂在 okx-ai/references/。OQ-1 收窄后其它 skill 大概率不做。

### OQ-12 watch-host supervisor 选型（OQ-3 衍生）
watch-host 以什么方式常驻: A) Hermes 后台进程(本会话 terminal background) B) cron 心跳(--once 每 N 秒) C) Windows 计划任务/服务。v1 建议 B（最简单可控，事件延迟 ≤ N 秒可接受）。
