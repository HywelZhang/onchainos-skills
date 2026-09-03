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
