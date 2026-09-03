# P0-07 policy 引擎（03 schema 的校验器 + 决策器）

> 状态: v0.1（2026-09-03）
> 配套: docs/design/03-policy-config.md（schema 与默认值表）；输入事件 = watch-host 归一化事件（06 §4）。
> 语言: python（stdlib；YAML 可用则支持，否则 JSON 配置）。v1 不做 Rust（OQ-5 工具链未装，接口与未来 Rust 版一致）。

## 1. 职责（v1 骨架）

1. validate(cfg): 03 schema 静态校验 —— mode 枚举域、events."*" 兜底、limits 数值/格式、script/hook 白名单目录存在性、auto 模式要求的配套(limits/consent 标记)为 warn 级提示。
2. load_chain(scope): 按 订阅级 > 任务级 > 角色级 > 全局 合并配置（浅合并，数组覆盖）。
3. classify(event): watch-host 事件(kind/jobId/markers/raw) → 候选节点 id（markers→节点映射表）+ 若 raw 含 wire 事件名(job_created/sub_user_reject/...) 则取 events.<wire>。
4. decide(ev): 输出决策 {mode, hooks(pre/post 命中), limits 命中, reason}。events.<wire> > nodes.<id> > events."*" > 域默认(ask)。
5. 输出: 决策 JSON；--dryrun 供测试。

## 2. 决策优先级（03 §1 落地）

```
事件 → events.<wire>(raw 含 wire 名) > nodes.<classify 节点> > events."*" > 角色/全局默认(ask)
scope 合并: sub-<id> > job-<id> > role-buyer|role-asp > global
```

## 3. classify 映射（v1 起表，后续可配）

| watch-host kind | markers/线索 | 默认节点 | 默认 mode |
|---|---|---|---|
| decision_request | — | buyer.sub.decide(或保留 decision 待人工) | ask |
| signal | active_subscription / signal | buyer.sub.signal_received | 解析后走 decide |
| task_event | 终态标记 | buyer.task.terminal / asp 侧按角色 | direct+notify |
| task_event | 中间态标记 | 对应流程节点 | ask/llm |
| notification | — | notify 节点 | direct |
| raw | 含 wire 事件名 → events.<wire> | 按 wire | events 表 |

## 4. 文件

- scripts/policy-engine.py: 引擎（--validate/--decide/--selftest）
- examples/policy/global.json + sub-EXAMPLE.json: 03 §5 三个场景的示例配置（buyer strict auto / 安全告警 script / ASP hooks）

## 5. 验证状态

- [x] selftest: schema 校验(非法 mode/越界金额/非白名单脚本报错)、scope 合并、wire 优先、dryrun 决策样例
- [ ] 接入 watch-host 事件流 + 真实订阅信号（待活跃订阅；watch-host 长跑同批）
- [ ] consent/grants 联动（后续接执行桥时实现，非 v1）
