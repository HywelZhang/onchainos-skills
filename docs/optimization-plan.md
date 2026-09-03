# onchainos-skills fork 优化方案（dev 分支）

> 状态: v1.0（2026-09-03 夜刷新）— P0/P1 完成，P2 进行中
> 基线: fork main == upstream main @ 17daea5（2026-09-02，本次检查无新 commit；仅 v4.8.x-beta tags，等 stable）
> 范围（OQ-1）: 只优化 okx.ai 功能域（单次任务 + 订阅任务场景）；不做 wallet/defi/dex-market 流程优化
> 目标: 让 OKX.AI 任务流程顺畅，最终形态 = 确定性 policy 引擎（事件直触发）替代 LLM 逐步驱动

## 1. 已交付（全部在 dev，已推送 origin/dev）

### P0 — 设计 + 工具（完成）
- docs/design/01-08: 节点清单(55 事件+审计)、信号 schema、policy 配置、用户旅程、开放问题、watch-host、policy 引擎、A/B 测试记录
- scripts/: sync-to-hermes(.sh/.ps1)、audit-events.py、watch-host.py、policy-engine.py、install-watch-task.ps1
- docs/cn-quickstart.md；examples/policy/

### P1 — 文档层（完成）
- okx-ai references 4 张 lite 卡（signal 13KB/playbook 29.2KB/watch 21.9KB/register+invariants 合并 36.8KB），零规则丢失
- labels.zh-CN.md i18n 渲染表；SKILL.md 瘦身 22.8→20.9KB（零规则丢失下限实测 ~20KB，路由 25 行零改动）
- SKILL.md FORK 标记补丁（lite 默认路由）；英文为主决策（OQ 已答）

### P2 — 引擎层（进行中）
- watch-host: 骨架+端到端+循环稳定性(16min/8 周期)验证通过（06）
- policy-engine: 校验器+scope 合并+classify+decide，selftest 8/8（07）
- supervisor 脚本就绪（OQ-12=B），计划任务未实装（需密码/管理员）

## 2. A/B 实测结论（docs/design/08）

| 任务型 | fork vs 官方 (input/cache/调用/成本) | 备注 |
|---|---|---|
| HTTP paywall A2MCP | -24%/-77%/-67%/-41% | fork 全面占优, 墙钟快 7.6× |
| MCP 传输 A2MCP | -19%/-55%/-30%/-23% | fork 占优, 墙钟方差 |
| A2A 全生命周期 | -11%/+138%/+69%/+68% | n=1 被 run 方差淹没; fork=文档层未含确定性引擎 |

结论: fork 文档层优势在有界确定性流程(实测); A2A 的预期优势在"确定性执行"(policy 引擎)而非文档——引擎未接线前 A2A 对比无意义。

## 3. 待办（剩余优化项）

### 核心（产品价值所在）
- [ ] executor-lite: 确定性买家驱动(01 buyer 节点直连 CLI, LLM 只在需求理解+内容评判缝) —— 含幂等/journal/竞态处理
- [ ] policy 引擎接 watch-host 事件流(事件 JSONL → decide → 动作), 形成闭环
- [ ] hook 运行时: 执行 03 配置的 pre/post(白名单脚本, veto 语义)
- [ ] 信号信封实现(02): ASP 模板生成器 + buyer strict/loose 解析
- [ ] executor-lite vs 官方 LLM A/B(A2A 用户侧全自动验证, 需 ~0.2 USDT escrow, 待确认)

### 验证/运维
- [ ] 真实订阅信号端到端(需活跃订阅; 同批补 OQ-10 实时基线)
- [ ] watch-host ≥24h 长跑; supervisor 计划任务实装
- [ ] 上游 sync 流程实战(upstream main 未动, 暂无需; 发 stable 后: merge+FORK 冲突解+audit-events 重跑)
- [ ] CN 代理 fallback 小工具(web3.okx.com 间歇超时, 三次实验均撞到)
- [ ] A2A n≥3 带约束复测(若要对 A2A 定论)

### 待确认项（详见 docs/design/05, 统一答复）
- OQ-8 evaluator v1 范围; OQ-11 labels 扩散
- 防呆护栏放宽试点(P1 遗留; 产品化方向建议保守保留)
- Rust 工具链安装(L2 开工前置, ~1-2GB)
- executor-lite 真实 escrow 测试预算(0.1×N USDT/轮)

## 4. 红线（不变）
references 路径稳定 / frontmatter description 不动 / 协议·资金·争议规则不改 / _shared 整目录同步 / main 只吃上游 / 小步提交可回滚
