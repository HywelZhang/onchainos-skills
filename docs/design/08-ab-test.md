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

