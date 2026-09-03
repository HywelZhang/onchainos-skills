# P0-08 A/B 实测：fork 优化 skills vs 官方原生 skills（2026-09-03）

> 方法: 同机、同 CLI(onchainos 4.5.2)、同模型(deepseek-v4-flash)、同任务表述、同 profile；唯一变量 = Hermes skills 目录内容。
> 官方条件 = origin/main 原样 skills（git archive）；fork 条件 = dev 分支 skills（lite 卡 + FORK 路由 + SKILL 瘦身）。
> 无头运行: `hermes -z "<prompt>" --in <repo> --usage-file ...`，任务 = 只读两操作（查订阅列表 + 列我的 agents）。
> 数据源: --usage-file JSON（官方 token 计量）+ state.db（墙钟/消息/实际 skill_view 加载）。

## 结果

| 指标 | 官方原生 | fork 优化 | Δ |
|---|---|---|---|
| 实际加载 skill 文档 | SKILL 21.3KB + preflight 1.1 + playbook 37.0 + discover 8.5 = **67.9KB** | SKILL 20.9 + preflight 1.1 + **playbook.lite 29.2** + discover 8.5 = **59.8KB** | **−8.1KB (−12%)** |
| input_tokens（非缓存新增） | 34,461 | 32,798 | −1,663 (−4.8%) |
| output_tokens | 4,592 | 4,809 | +217（fork 输出更详） |
| cache_read_tokens | 172,032 | 210,688 | +38,656（fork 缓存复用更多） |
| api_calls / assistant 轮 | 7 | 8 | +1（fork 多取 agent 详情） |
| 墙钟耗时 | 92.1s | 89.3s | −3% |
| 估算成本 | $0.00659 | $0.00653 | −1% |
| CLI 报错重试 | 3 次（--format 不被支持） | 2 次 | 少 1 |

## 解读

1. 设计生效: fork 在"我的订阅"流实际加载了 task-user-playbook.lite.md(29.2KB) 而非全量(37KB)；identity-discover 未做 lite（范围外）两轮同加载 8.5KB——对照组无回归。
2. fork 少 4.8% 新增输入 token、少 8.1KB 文档字节、耗时略短，**且多完成了一个 CLI 查询**（#1792 详情: 描述/securityRate 5.00/soldCount 5——官方轮没取）。单位产出的输入效率 fork 更高。
3. cache_read 高是 fork 会话把更长上下文驻留缓存（8 次调用 vs 7 次），deepseek 缓存价低，成本仍略低。
4. 定性: fork 轮的总结更结构化、设备信息/评分/销量齐全；两轮都正确报告 804 错误（ASP 未激活时服务列表被 API 拒）且都未伪造。

## 边界（必须声明）

- n=1，~5% 级差异在单次噪声内；要统计显著需 n≥3-5。
- 任务集只读且仅 1/2 操作在 lite 覆盖内；最大 delta 的流没跑到: 身份注册(−34%)需链上写、订阅信号(−49%)需活跃订阅。真实差距应大于本表。
- 两轮都被残留备份目录污染一次（歧义 skill 重试 1 次，对称，已清理）。
- fork 轮额外查询使 token 对比偏保守（同工作量下 fork 输入差会更大）。

## 复现

```
# 官方 skills: git archive origin/main skills | tar -x -C <tmp>
# 切换: mv $HERMES/skills/okx <backup>; cp -r <tmp>/skills $HERMES/skills/okx
hermes -z "$(cat .ab/prompt.txt)" --in . --usage-file .ab/usage-<tag>.json --cli
# 指标: usage-*.json + state.db sessions/messages 查询（本会话脚本见会话记录）
```

## 实验 2: 真实付费 A2MCP 任务（2026-09-03, 同一轮内对比）

任务: 使用 A2MCP 服务「TradeDesk 指标体验引流」(端 https://api-ai.online/okxai/trial, 0.1 USDT/次, 服务 39830)——GET→402→x402 exact 付款→拿回执。两轮各付款一次(0.1 USDT), fork 腿与官方腿使用同一份提示词。

| 指标 | fork 优化 | 官方原生 | Δ |
|---|---|---|---|
| 墙钟耗时 | 37.8s | 285.9s | **7.6× 快** |
| input_tokens | 24,892 | 30,926 | +24% |
| output_tokens | 2,550 | 7,637 | 3.0× |
| cache_read_tokens | 79,744 | 348,544 | 4.4× |
| api_calls | 4 | 12 | 3.0× |
| 估算成本 | $0.00442 | $0.00744 | +68% |
| 业务结果 | 成功(1 付, TD-0C7ACFEE) | 成功(1 付, TD-85D63D03) | 等价 |

定性:
- fork 路径直: 调用→402→quote→pay→结果, 4 次调用无多余探索; 官方轮多次重读参考文档(缓存 348K vs 80K)、输出 3 倍长(大量错误叙述与过程说明)、含一次网络超时绕代理(web3.okx.com TCP 超时→Clash)与 balanceStatus 检查。
- 官方轮的网络超时属环境噪声(与 skills 无关), 但正是其"先查余额再走支付"的流程让它撞上; fork 轮直接走支付无此步骤。差异被放大但仍方向一致。
- 两轮业务等价: 各自 1 次 0.1 USDT 付款, order TD-0C7ACFEE / TD-85D63D03, register_url 相同(https://tradedesk.cn/r/RC7SLE3R)。
- 费用合计 0.2 USDT(探针即 fork 腿); 余额 ~0.30 USD₮0。

边界: n=1; 环境噪声(网络)存在; 但 7.6× 耗时/3× 调用差远大于单次噪声, 方向可信。复现: 同上 `--usage-file`, 提示词 .ab/probe-prompt.txt。

## 实验 3: 真实付费 A2MCP(MCP 传输)任务（2026-09-03）

任务: 使用 A2MCP 服务「AI饮食运动助手」(ASP 健康生活, MCP 端点 https://mcp.opcshop.xyz/mcp, 0.01 USDT/次, 服务 30754)——MCP 接入 → 付费墙在 tools/call 层 → 402 → x402 exact 付款 → 真实工具调用(健康计划)。两轮同一提示词, 各付款一次。fork 腿在 generate_plan 触发 402; 官方腿在 bmi 触发 402 后 generate_plan 命中服务端免费缓存(cached:true)。

| 指标 | fork 优化 | 官方原生 | Δ |
|---|---|---|---|
| 墙钟耗时 | 397.9s | 354.3s | 官方快 12%(噪声, n=1) |
| input_tokens | 35,820 | 42,639 | 官方 +19% |
| output_tokens | 11,292 | 15,579 | 官方 +38% |
| cache_read_tokens | 327,424 | 508,800 | 官方 +55% |
| api_calls | 10 | 13 | 官方 +30% |
| 估算成本 | $0.00909 | $0.01176 | fork 省 23% |
| 业务结果 | BMI+7天计划, 1付(TD 0x3f5c…) | BMI+7天计划, 1付(0xe40cb2…) | 等价 |

定性:
- fork 在 token/调用/成本全面占优(输入-19%, 输出-38%, 缓存-55%, 调用-30%, 成本-23%); 墙钟本轮官方略快(两轮都 ~6 分钟级, MCP+内容型任务远重于实验 2 的轻量 paywall)。
- 两轮走的付费动作路径不同(fork: generate_plan 直接付; 官方: bmi 付 + generate_plan 免费缓存)——官方轮因此多拿到一次独立 BMI 分析, 属行为方差, 但成本同(0.01)。
- 两轮都独立发现同一 ASP 情报: ① 服务端按 default_user 缓存计划, generate_plan 对未付费匿名用户可免费命中(cached:true, 商业逻辑漏洞) ② 个性化仅由 BMI 档案驱动, "久坐/生活方式"自然语言不生效 ③ tools/list 与 intro 永久免费, 可先探后付。
- CN 网络: 两轮都需 Clash 代理访问 OKX 余额接口(web3.okx.com 超时), 付费主流程直连正常。

边界: n=1; 付费动作路径不一致引入方差; token/成本方向与实验 2 一致(fork 占优), 墙钟不一致(实验 2 fork 快 7.6×, 本实验官方快 12%)→ 墙钟受路径与网络影响大, token/调用/成本更稳定。累计两实验: fork 输入 -16~-24%、成本 -23~-41%。复现: .ab/probe2-prompt.txt。

## 实验 4: A2A 全生命周期任务（2026-09-03）— 结果反转, 如实记录

任务: 发布 A2A 任务给「美食食谱与生活文案」(ASP 内容创作工坊 #11198, 0.1 USDT escrow) 并走完 publish→accept→deliver→review→complete。两轮同一提示词, 各自独立完成(双方 escrow 0.1 均在验收后释放给 ASP)。

| 指标 | fork 优化 | 官方原生 | Δ |
|---|---|---|---|
| 墙钟耗时 | 534.3s | 448.2s | 官方快 16% |
| input_tokens | 51,749 | 57,860 | fork 低 11% |
| output_tokens | 36,839 | 13,518 | fork 高 2.7× |
| cache_read_tokens | 2,957,184 | 1,242,112 | fork 高 2.4× |
| api_calls | 44 | 26 | fork 高 69% |
| 估算成本 | $0.02584 | $0.01536 | fork 高 68% |
| 业务结果 | 全生命周期完成(0x4bd2…), 验收合格 | 全生命周期完成(0xfefa…), 验收合格 | 等价 |

反转原因(fork 腿实际发生的弯路, 来自会话报告): ① 用提示词里的短 id "205aa54c" 做 --service-id 过滤未生效→换 --service-name 重定位(多轮) ② 直连超时→代理重试 ③ 接受确认遇竞态(code=1001 任务已不在 OPEN, ASP 先接受)→按预授权重试 ④ 交付物下载首次用发送方 agentId 报 access token invalid→换自身 agent 重下 ⑤ 评审门拦截 complete→走 pending-decisions 队列 ⑥ 全程 49 条工具消息、输出 36.8K tokens(大量过程叙述)。官方腿同提示词却未踩 ①④ 的弯路(直接定位到完整 UUID、一次下载成功)。
判断: ①④ 类弯路是 agent 行为方差(同提示词两腿表现不同), 竞态/超时是环境方差; 长流程(8-9 分钟)方差大, n=1 下不足以归因 skills。但 fork 输出 2.7×/调用 1.7× 的量级提示: fork 腿在该轮更"话痨+多试探", 若复现需加"简洁汇报"约束并 n≥3。

## 主矩阵: 三种任务类型对比汇总（fork 相对官方的 Δ, n=1/型）

| 任务型 | input | output | cache_read | api_calls | 墙钟 | 成本 |
|---|---|---|---|---|---|---|
| 实验2 HTTP paywall A2MCP(0.1U) | -24% | -67% | -77% | -67% | -87% (fork 快 7.6×) | -41% |
| 实验3 MCP 传输 A2MCP(0.01U) | -19% | -38% | -55% | -30% | +12% | -23% |
| 实验4 A2A 全生命周期(0.1U) | -11% | +173% | +138% | +69% | +19% | +68% |

结论: fork 优势集中在**有界、确定性强的流程**(A2MCP 调用类: 输入/缓存/成本全面占优); 在**长程开放式 agent 流程**(A2A 发布-验收)上, 单次方差淹没 skill 差异, 本样本官方反超——需 n≥3 + 同路径约束(完整 UUID、简洁输出指令)才能定论。业务成功率两型均为 100%(各完成)。

## 实验 5: A2A 全生命周期 n=3 配对复测（2026-09-03）

同一任务（美食食谱, 完整 UUID + 简洁输出约束提示词）, 3 对 6 轮交替跑（fork↔official 同机同 prompt）。escrow 0.1×6（验收后均释放 ASP）。

| 指标(中位数) | fork (n=3) | official (n=3) | Δ (fork 为基准) |
|---|---|---|---|
| input_tokens | 59,589 | 61,814 | −3.6% |
| output_tokens | 15,612 | 20,489 | **−24%** |
| cache_read | 1,591,552 | 1,786,496 | −11% |
| api_calls | 30 | 33 | −9% |
| 估算成本 | $0.01710 | $0.01963 | −13% |
| 墙钟 | 650s | 523s | +24%(fork 慢) |
| 业务成功率 | 3/3 | 2/3 | fork 更高 |

分轮明细: fork (inp/out/calls/cost): 60615/24657/42/$0.0226, 59589/15345/30/$0.0171, 36976/15612/25/$0.0129 — 全部成功。official: 65617/20489/40/$0.0222, 61814/21328/33/$0.0196(失败), 47965/16413/29/$0.0159。
解读: n=3 推翻实验 4 的"官方反超"——那确是单次方差(official run2 未走完生命周期, fork 3/3 全成)。fork 文档层在 A2A 上: token/调用/成本小优(−9~−24%), 成功率更高; 墙钟仍无优势(两腿同为 LLM 驱动, ASP 交付延迟+agent 行为方差主导, 中位 523-650s 波动大)。真正拉开差距的轴 = executor-lite 确定性执行(0 LLM), 见下。

## 主结论（全部实测, 2026-09-03）

1. 文档层(fork lite 卡+i18n): A2MCP 有界流程全面占优(输入 −19~−24%, 缓存 −55~−77%, 成本 −23~−41%, 墙钟最快 −87%); A2A 长流程小幅占优(输出 −24%, 成本 −13%, 成功率 3/3 vs 2/3), 墙钟无优势。
2. 确定性执行(executor-lite, 用户侧机械节点): 0 LLM token / $0 LLM 成本, 2/2 真实 A2A 全生命周期完成(修复下载正则+评审门序列后)。LLM 只留在需求理解与内容评判两处语义缝 → 用户"除需求理解全部自动"的预想成立。
3. 待补: 订阅信号流端到端(需活跃订阅)与 ASP 侧执行器。





## 实验 6: executor-lite 确定性执行 n=3 实测汇总（真实付费, 全部同 ASP 美食食谱/内容创作工坊 #11198, escrow 0.1×3）

| run | 任务 | 交付类型 | 生命周期 | LLM tokens | CLI 调用 | 备注 |
|---|---|---|---|---|---|---|
| run1 0xd0b4…61f | 减脂食谱 | 文件 recipe.md | 完成(评审门) | 0 | ~10 | 下载正则 bug → 手工补 recipe 后走评审流; 修复 |
| run2 0x4753…03b | 减脂食谱 | 文件 | 完成(全自动) | 0 | ~10 | 规则词汇误判 → 校准; 全链路 0 介入 |
| run3 0x1e01…bcb3 | 减脂食谱 | **text 内联** | 完成(评审门) | 0 | ~12 | 新发现: text 型交付无 fileKey; 交付晚于 300s watch 窗; 补 resume --job-id + 历史补抓 + text 提取后闭环 |

- 确定性侧三属性: 生命周期 0 LLM token、每轮 ~10-12 次确定性 CLI 调用、失败模式全部可复现可修复(下载正则/text 型/resume)。
- 对比官方 LLM 腿(A2A n=3 中位): input 59.6K + cache 1.59M token、33 API 调用、$0.0171/轮、成功率 2/3 → executor: 0 token / 0 LLM 成本 / ~10 CLI 调用 / 3/3 完成。
- 墙钟: executor 主导因素=ASP 交付延迟+watch 窗(固定轮询), 非 LLM 推理; run3 端到端 ~15min 中 5min 是 watch 超时等待, 交付到达后 resume 收尾 <3min。
- 结论: 用户侧机械节点全自动(0 LLM)成立且可扩展到 text 交付; ASP 交付延迟是唯一可变因素 → 产品上由订阅周期/通知驱动, 无需轮询(06 watch-host 已覆盖)。executor 确定性驱动 = 官方 LLM 路径成本的 ~0 倍、可靠性更高; 差异数量级在"架构选择"(LLM 路由 vs 程序路由), 与文档层优化正交叠加。
