# onchainos-skills fork 优化方案（dev 分支）

> 状态: v1.1（2026-09-03 深夜刷新）— P0/P1 完成，P2 买家侧核心完成，产品化打包未开始
> 基线: fork main == upstream main @ 17daea5（2026-09-02；仅 v4.8.x-beta tags，等 stable）
> 范围（OQ-1）: 只优化 okx.ai 功能域（单次任务 + 订阅任务场景）
> 目标: 确定性 policy 引擎（事件直触发）替代 LLM 逐步驱动；用户侧除需求理解外全自动

## 1. 交付状态

### P0 设计+工具（完成）
docs/design/01-09（节点清单/信号 schema/policy 配置/用户旅程/开放问题/watch-host/policy 引擎/A-B/ASP 执行器设计）；scripts/ 全套（sync-to-hermes、audit-events、watch-host、policy-engine、decision-loop、executor-lite、signal-envelope、install-watch-task）；docs/cn-quickstart.md；examples/policy/

### P1 文档层（完成）
4 张 lite 卡（signal 13KB/playbook 29.2KB/watch 21.9KB/register 合并 36.8KB）零规则丢失；labels.zh-CN.md；SKILL.md 瘦身 22.8→20.9KB + FORK 标记

### P2 买家侧引擎（完成-核心）
- [x] watch-host + supervisor 计划任务 okx-watch-host 实装（2026-09-03，每 5 分钟 --once，日志 %LOCALAPPDATA%\okx-watch-host\watch.log；修复任务环境 PATH/gbk 解码/emoji 编码三坑，实测收到事件）
- [x] policy-engine（校验+scope 合并+classify+decide，8/8）；decision-loop（闭环+pre/post hook veto/fallback，6/6）
- [x] executor-lite 真机 n=4（escrow 0.1×4 全释放 ASP，4/4 完成）：run1 文件(修下载正则)/run2 文件全自动/run3 text 型发现→补 text 提取+resume+历史补抓/run4 文件全自动零介入；生命周期 0 LLM token
- [x] signal-envelope（9/9）+ 信封闭环 gate（strict 无信封/无效→强制 ask，6/6）

### P2 买家侧订阅流（第一阶段, 2026-09-03）
- [x] 订阅市场勘察: ASP 8136『1M·斯巴达』高波动主流币跟单信号(serviceId abff5dbe…, 20 USDT/月, 72h 试用, sid 36563) = 真实流验证候选
- [x] 信号内容分流: policy-engine 新 kind signal_order/signal_analysis + decision-loop contentTags 确定性打标(0 LLM); examples/policy/sub-36563.json 真 ASP 模板(analysis→notify 不打扰 / order→ask 资金确认 / 未知→ask 兜底)
- [x] scripts/sub-sim.py 离线场景模拟 7/7 PASS(同管线可回放录制事件做校准 OQ-10)
- [x] docs/design/10-buyer-subscription.md 一阶段手册(订阅日常命令表+分流设计+风险)
- [ ] 真机上链订阅(72h 试用) → watch 捕获真实 analysis/order → 分流正确性校准（OQ-14, 需确认）

### 未做（如实）
- ASP 侧执行器（09 设计已定，需 ASP 身份真机采样，见 OQ-13）
- 真实订阅流端到端（无活跃订阅；事件分类校准 OQ-10 未做）
- 执行桥护栏接口化（limits/grants 与真实资金执行绑定验证）
- watch-host ≥24h 长跑、CN 代理 fallback、上游 stable sync、打包分发

## 2. A/B 实测汇总（docs/design/08，全真实付费，方法见文档）

| 流 | fork vs 官方（token/调用/成本/墙钟） | n | 成功率 |
|---|---|---|---|
| 只读任务（文档层） | input −4.8%、成本 −1%、墙钟 −3%（噪声级） | 1 | 1/1 |
| A2MCP HTTP paywall | input −24%、cache −77%、调用 −67%、成本 −41%、墙钟快 7.6× | 1 | 1/1 vs 1/1 |
| A2MCP MCP 传输 | input −19%、cache −55%、调用 −30%、成本 −23% | 1 | 1/1 vs 1/1 |
| A2A 全生命周期（文档层） | input −3.6%、output −24%、调用 −9%、成本 −13%；墙钟 +24% 慢 | 3 | 3/3 vs 2/3 |
| executor 确定性 vs 官方 LLM | 0 LLM token/$0 vs 59.6K+1.59M cache/33 调用/$0.0171/轮 | 4 | 4/4 vs 2/3 |

结论: 文档层优势在有界确定性流（A2MCP 类，成本 −23~−41%）；A2A 长流程文档层仅小幅占优（方差大），数量级优势在确定性执行（executor：0 LLM token、失败模式可复现可修复）。

## 3. 待办（按优先级）

1. [P1] 真实订阅流端到端（OQ-14 待确认: 8136 跟单信号 72h 免费试用, 候选服务 sub-36563; 离线链路 sub-sim 7/7 已通）——买家侧最后一块未验证面
2. [P1] 打包产品化: 安装器/初始化向导/默认 policy 模板包/README-as-product（对象=会用终端的开发者先行）
3. [P2] ASP 侧执行器（OQ-13 定身份）
4. [P2] 护栏接口化 + 内容评审规则扩展（超出食谱类任务时）
5. [P2] watch ≥24h 长跑；上游 stable sync 实战；CN 代理 fallback

## 4. 红线（不变）

references 路径稳定 / frontmatter description 不动 / 协议·资金·争议规则不改 / _shared 整目录同步 / main 只吃上游 / 小步提交可回滚
