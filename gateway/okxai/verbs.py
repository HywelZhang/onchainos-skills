"""功能级 verb 层（P0 读路径；写路径见 P1）。

设计原则（docs/design/11 §4.2）:
    * 一个 verb = 一个业务动作，参数名用业务语言，不用底层 HTTP 参数名；
    * 返回体 = 服务端 `data` 的业务字段（结构与 CLI 一致，便于 golden 对照）；
    * 身份（agenticId / 钱包地址 / 设备）由 session 解析并**缓存**，
      不再像 CLI 那样每次调用都重新查询（audit 里 58% 的调用是这个开销）。

verbs:
    session.status           本地会话/密码学/身份诊断（无网络）
    session.devices          登录设备列表
    wallet.status            账号 + 策略
    agent.mine               我的 agent（保留 CLI 的账户分组嵌套）
    agent.flat               我的 agent（扁平）
    agent.get                按 ID 批量取 agent
    task.mine                我的任务列表
    task.active              跨 agent 的未终态任务
    task.status              单个任务详情
    task.deliverables        交付物（本地盘 + 任务详情）
    sub.list                 我的订阅列表
    sub.detail               单个订阅详情
    sub.cost                 订阅月成本合计
    service.match            市场服务检索
    flow.pending_decisions   本地待决策队列
    gate.check               就绪度自检（钱包/身份/通信）
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, Callable

from . import home as _home
from . import keystore, labels, wallets
from .session import ROLE_CODES, Session

AGENT_LIST_PATH = "/priapi/v5/wallet/agentic/agent/agent-list"
AGENT_BATCH_PATH = "/priapi/v5/wallet/agentic/agent/batch-list"
AGENT_DEVICE_LIST_PATH = "/priapi/v5/wallet/agentic/agent/device-list"
POLICY_PATH = "/priapi/v5/wallet/agentic/policy/query"
TASK_PREFIX = "/priapi/v1/aieco/task"
SUBSCRIBE_PREFIX = "/priapi/v1/aieco/task/subscribe"

CHAIN_INDEX = "196"


class VerbError(RuntimeError):
    pass


@dataclass
class Verb:
    name: str
    summary: str
    fn: Callable[..., Any]
    args: tuple[str, ...] = field(default_factory=tuple)


REGISTRY: dict[str, Verb] = {}


def verb(name: str, summary: str, args: tuple[str, ...] = ()):
    def deco(fn: Callable[..., Any]) -> Callable[..., Any]:
        REGISTRY[name] = Verb(name=name, summary=summary, fn=fn, args=args)
        return fn

    return deco


# ─────────────────────────── 会话 / 钱包 ───────────────────────────


@verb("session.status", "本地会话与身份诊断（不联网）")
def session_status(s: Session, **_: Any) -> dict[str, Any]:
    info = keystore.describe()
    info["transport"] = s.client.describe()
    info["email"] = wallets.email()
    info["logged_in"] = bool(s.tokens and s.tokens.access_token)
    try:
        rows = s.agents()
        info["agents"] = [
            {
                "agentId": r.get("agentId"),
                "role": r.get("role"),
                "name": r.get("name"),
                "status": r.get("status"),
            }
            for r in rows
        ]
    except Exception as exc:  # noqa: BLE001 - 诊断接口不该因网络失败而不可用
        info["agents"] = []
        info["agents_error"] = f"{type(exc).__name__}: {exc}"
    return info


@verb("wallet.status", "账号状态与交易策略")
def wallet_status(s: Session, **_: Any) -> dict[str, Any]:
    session_meta = keystore.read_session()
    expire = str(session_meta.get("sessionKeyExpireAt") or "")
    logged_in = bool(s.tokens and s.tokens.refresh_token) and (not expire.isdigit() or int(expire) > 0)
    account_id = wallets.selected_account_id()
    policy: Any = None
    if logged_in and account_id:
        try:
            data = s.get(POLICY_PATH, query={"accountId": account_id})
            if isinstance(data, list):
                data = data[0] if data else None
            policy = data
        except Exception:  # noqa: BLE001 - 与 CLI 一致：策略查询失败降级为 null
            policy = None
    return {
        "email": wallets.email(),
        "loggedIn": logged_in,
        "loginType": wallets.login_type() or None,
        "currentAccountId": account_id,
        "currentAccountName": wallets.account_name(),
        "accountCount": len(wallets.account_list()),
        "policy": policy,
    }


@verb("session.devices", "本 agent 的登录设备列表", args=("agent_id",))
def session_devices(s: Session, agent_id: str | None = None, page: int = 1, page_size: int = 20, **_: Any) -> dict[str, Any]:
    aid = s.agent_id("user", agent_id)
    data = s.get(f"{AGENT_DEVICE_LIST_PATH}?page={page}&pageSize={page_size}", agent_id=aid)
    # 与 agent-list 同款单元素数组归一（4.5.2 实测：data = [{list:[…],page,pageSize,total}]）
    if isinstance(data, list) and len(data) == 1 and isinstance(data[0], dict):
        data = data[0]
    if not isinstance(data, dict):
        data = {"list": data}
    data.setdefault("thisDeviceId", s.device_id)
    data.setdefault("page", page)
    data.setdefault("pageSize", page_size)
    rows = data.get("list")
    if isinstance(rows, list):
        for row in rows:
            if not isinstance(row, dict):
                continue
            row["isThisDevice"] = str(row.get("deviceId") or "") == s.device_id
            row["lastOnlineLocal"] = _fmt_local_ms(row.get("lastOnlineTime"))
        data.setdefault("total", len(rows))
    return data


# ─────────────────────────── 身份 ───────────────────────────


@verb("agent.mine", "我的 agent 列表（账户分组）", args=("role", "owner_address"))
def agent_mine(s: Session, role: str | None = None, owner_address: str | None = None,
               page: int = 1, page_size: int = 100, **_: Any) -> Any:
    query = {"chainIndex": CHAIN_INDEX, "page": str(page), "pageSize": str(page_size)}
    if role:
        query["role"] = ROLE_CODES.get(role, str(role))
    if owner_address:
        query["ownerAddress"] = owner_address
    data = s.get(AGENT_LIST_PATH, query=query)
    return _normalize_agent_page(data, page=page, page_size=page_size)


@verb("agent.flat", "我的 agent 列表（扁平）", args=("role",))
def agent_flat(s: Session, role: str | None = None, **_: Any) -> list[dict[str, Any]]:
    rows = s.agents(refresh=True)
    if role:
        code = ROLE_CODES.get(role, str(role))
        rows = [r for r in rows if str(r.get("role")) in (code, role)]
    return rows


@verb("agent.get", "按 ID 批量取 agent", args=("agent_ids",))
def agent_get(s: Session, agent_ids: str | None = None, **_: Any) -> Any:
    if not agent_ids:
        raise VerbError("agent.get 需要 agent_ids（逗号分隔）")
    ids = [i.strip() for i in agent_ids.split(",") if i.strip()]
    if not ids:
        raise VerbError("agent_ids 不能为空")
    # 官方 CLI 用重复的 agentIdList 参数 + 两个开关（queries.rs §get-agents）
    query: dict[str, Any] = {
        "agentIdList": ids,
        "needBlackStatus": "false",
        "needAgentService": "false",
    }
    return s.get(AGENT_BATCH_PATH, query=query)


# ─────────────────────────── 任务 ───────────────────────────


@verb("task.mine", "我的任务列表", args=("role", "page", "limit"))
def task_mine(s: Session, role: str = "user", page: int = 1, limit: int = 50, **_: Any) -> Any:
    aid = s.agent_id(role)
    return s.get(f"{TASK_PREFIX}/my?page={page}&page_size={limit}", agent_id=aid, with_cert=True)


@verb("task.active", "跨 agent 的未终态任务", args=("role",))
def task_active(s: Session, role: str = "user", **_: Any) -> Any:
    aid = s.agent_id(role)
    # 官方 CLI 用 body {"agentIds": [...]}（≤20 个；in_progress.rs §5.3），
    # 然后把三类结果合并成 {tasks,totalAgents,totalTasks}（aggregated 形状）。
    data = s.post(f"{TASK_PREFIX}/inProgress", body={"agentIds": [aid]}, agent_id=aid, inject_cert=True)
    return _aggregate_in_progress(data)


@verb("task.status", "单个任务详情", args=("job_id", "role"))
def task_status(s: Session, job_id: str | None = None, role: str = "user", **_: Any) -> Any:
    if not job_id:
        raise VerbError("task.status 需要 job_id")
    aid = s.agent_id(role)
    data = s.get(f"{TASK_PREFIX}/{job_id}", agent_id=aid, with_cert=True)
    return _decorate_task(data)


@verb("task.deliverables", "交付物列表（本地 manifest）", args=("job_id", "role"))
def task_deliverables(s: Session, job_id: str | None = None, role: str = "user", **_: Any) -> Any:
    """复刻 CLI 的 `task-deliverable-list`：读本地交付物 manifest，**不联网**。

    布局（deliverables.rs）: `<home>/deliverables/<role>/<jobId>[_<title>]/manifest.json`，
    manifest 内含 task 上下文与 entries[]（filename/originalName/type/size/savedAt）。
    """
    if not job_id:
        raise VerbError("task.deliverables 需要 job_id")
    if role not in ("user", "asp"):
        raise VerbError(f"role 必须是 'user' 或 'asp'，收到 {role!r}")
    root = _home.path("deliverables", role)
    target = None
    if root.exists():
        exact = root / job_id
        if exact.is_dir():
            target = exact
        else:
            for entry in sorted(root.iterdir()):
                if entry.is_dir() and entry.name.startswith(f"{job_id}_"):
                    target = entry
                    break
    if not target:
        return {"jobId": job_id, "deliverables": [], "note": f"本地无该任务的交付物目录（{root / job_id}）"}
    manifest_path = target / "manifest.json"
    if not manifest_path.exists():
        return {"jobId": job_id, "deliverables": [], "note": f"缺少 manifest.json（{manifest_path}）"}
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001
        raise VerbError(f"manifest.json 解析失败: {exc}") from exc

    task = manifest.get("task") or {}
    entries = []
    for item in manifest.get("entries") or []:
        if not isinstance(item, dict):
            continue
        entries.append({
            "path": str(target / str(item.get("filename") or "")),
            "originalName": item.get("originalName"),
            "deliverableType": item.get("deliverableType"),
            "sizeBytes": item.get("sizeBytes"),
            "savedAt": item.get("savedAt"),
        })
    return {
        "jobId": manifest.get("jobId") or job_id,
        "shortId": task.get("shortId"),
        "title": task.get("title"),
        "tokenAmount": task.get("tokenAmount"),
        "tokenSymbol": task.get("tokenSymbol"),
        "counterpartyAgentId": task.get("counterpartyAgentId"),
        "counterpartyName": task.get("counterpartyName"),
        "deliverables": entries,
        "dir": str(target),
    }


# ─────────────────────────── 订阅 ───────────────────────────


@verb("sub.list", "我的订阅列表", args=("role",))
def sub_list(s: Session, role: str = "user", **_: Any) -> Any:
    aid = s.agent_id(role)
    data = s.get(f"{SUBSCRIBE_PREFIX}/my", agent_id=aid, with_cert=True)
    if not isinstance(data, dict):
        data = {"list": data}
    data.setdefault("thisDeviceId", s.device_id)
    for row in data.get("list") or []:
        _decorate_subscription_row(row, s.device_id)
    return data


@verb("sub.detail", "单个订阅详情", args=("sub_id", "role"))
def sub_detail(s: Session, sub_id: str | None = None, role: str = "user", **_: Any) -> Any:
    if not sub_id:
        raise VerbError("sub.detail 需要 sub_id")
    aid = s.agent_id(role)
    return s.get(f"{SUBSCRIBE_PREFIX}/{sub_id}", agent_id=aid, with_cert=True)


@verb("sub.cost", "活跃订阅月成本合计")
def sub_cost(s: Session, **_: Any) -> dict[str, Any]:
    from . import tokens

    data = sub_list(s)
    rows = data.get("list") if isinstance(data, dict) else data
    total = 0.0
    symbol = ""
    token_address = ""
    active = 0
    for row in rows or []:
        if not isinstance(row, dict):
            continue
        status = row.get("status")
        if status not in (1, "1"):
            continue
        active += 1
        try:
            total += float(row.get("paymentCurrencyAmount") or 0)
        except (TypeError, ValueError):
            pass
        token_address = token_address or str(row.get("paymentTokenAddress") or "")
        symbol = symbol or str(row.get("paymentTokenSymbol") or "") or tokens.symbol_for(
            str(row.get("chainIndex") or CHAIN_INDEX), token_address
        )
    return {
        "tokenAddress": token_address,
        "tokenSymbol": symbol,
        "totalAmount": "%g" % total,
        "activeCount": active,
    }


# ─────────────────────────── 市场 / 本地队列 ───────────────────────────


@verb("service.match", "市场服务检索", args=("keywords", "page_size"))
def service_match(s: Session, keywords: str | None = None, page_size: int = 10, **_: Any) -> Any:
    body: dict[str, Any] = {"pageSize": page_size}
    if keywords:
        body["keywords"] = [k for k in keywords.split(",") if k.strip()]
    aid = s.agent_id("user")
    data = s.post(f"{TASK_PREFIX}/asp/service/search", body=body, agent_id=aid)
    for service in (data.get("services") if isinstance(data, dict) else None) or []:
        asp = service.get("asp") if isinstance(service, dict) else None
        if isinstance(asp, dict):
            asp["rating"] = labels.format_rate(asp.get("rating"))
    return data


@verb("flow.pending_decisions", "本地待决策队列")
def flow_pending_decisions(s: Session, **_: Any) -> Any:
    path = _home.path("task", "pending-decisions-new.json")
    if not path.exists():
        return {"pending": [], "source": str(path)}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001
        raise VerbError(f"pending-decisions-new.json 解析失败: {exc}") from exc
    return {"pending": data, "source": str(path)}


@verb("gate.check", "就绪度自检（钱包/身份/通信）")
def gate_check(s: Session, **_: Any) -> dict[str, Any]:
    w = wallet_status(s)
    identity: dict[str, Any] = {"ok": False}
    try:
        aid = s.agent_id("user")
        for row in s.agents():
            if str(row.get("agentId")) == aid:
                identity = {"ok": bool(row.get("status") == 1), "agentId": aid,
                            "name": row.get("name"), "role": "user", "status": row.get("status")}
                break
    except Exception as exc:  # noqa: BLE001
        identity = {"ok": False, "hint": f"{type(exc).__name__}: {exc}"}
    ready = bool(w.get("loggedIn")) and bool(identity.get("ok")) and False  # 通信面未接入 → 不 ready
    return {
        "ready": ready,
        "wallet": {"ok": bool(w.get("loggedIn")), "accountId": w.get("currentAccountId"),
                   "accountName": w.get("currentAccountName"), "email": w.get("email")},
        "identity": identity,
        "communication": {
            "ok": False,
            "hint": "消息面（XMTP）尚未接入（P2b 经 Node bridge）；因此 ready=false",
        },
    }


def run(name: str, session: Session | None = None, **kwargs: Any) -> Any:
    entry = REGISTRY.get(name)
    if not entry:
        raise VerbError(f"unknown verb {name!r}; available: {', '.join(sorted(REGISTRY))}")
    s = session or Session.load()
    return entry.fn(s, **kwargs)


def listing() -> list[dict[str, Any]]:
    return [
        {"name": v.name, "summary": v.summary, "args": list(v.args)}
        for v in sorted(REGISTRY.values(), key=lambda v: v.name)
    ]


# ─────────────────────────── 形状复刻辅助 ───────────────────────────


def _normalize_agent_page(data: Any, page: int, page_size: int) -> Any:
    """复刻 CLI 的 `normalize_singleton_object`：单元素数组 → 该元素本身。

    agent-list 的原始 `data` 是 `[{"list":[…],"page":…,"total":…}]`，CLI 之后只取
    第一个元素（并补上自己算出的分页元数据），这里保持一致以便与 golden 对齐。
    """
    if isinstance(data, list):
        if len(data) == 1 and isinstance(data[0], dict):
            data = data[0]
        else:
            data = {"list": data}
    if isinstance(data, dict):
        data.setdefault("page", page)
        data.setdefault("pageSize", page_size)
        if "total" not in data:
            rows = data.get("list")
            data["total"] = len(rows) if isinstance(rows, list) else 0
    return data


def _aggregate_in_progress(data: Any) -> dict[str, Any]:
    """`task/inProgress` 的三类结果 → CLI 的统一形状（`tasks`/`totalAgents`/`totalTasks`）。

    注意: golden 录制时无未终态任务，非空分支尚未用真实数据校验（P1 待补）。
    """
    data = data if isinstance(data, dict) else {}
    tasks: list[dict[str, Any]] = []
    for key, role in (("buyerTasks", "user"), ("providerTasks", "asp"), ("evaluatorDisputes", "evaluator")):
        for item in data.get(key) or []:
            if isinstance(item, dict):
                tasks.append({**item, "myRole": role})
    agents = {t.get("buyerAgentId") for t in tasks} | {t.get("providerAgentId") for t in tasks}
    return {
        "tasks": tasks,
        "totalAgents": len([a for a in agents if a]),
        "totalTasks": len(tasks),
        "hasInProgress": bool(tasks),
        "inProgressNum": len(tasks),
    }


def _decorate_task(data: Any) -> Any:
    """给任务详情补上 statusName / statusDescription（CLI 文本渲染的等价字段）。"""
    if not isinstance(data, dict):
        return data
    name = labels.task_status_name(data.get("status"))
    data.setdefault("statusName", name)
    data.setdefault("statusDescription", labels.task_status_description(name))
    return data


def _decorate_subscription_row(row: Any, device_id: str) -> None:
    """复刻 CLI 对订阅行的加工（device_routing.rs / subscription_ops.rs）。"""
    if not isinstance(row, dict):
        return
    row["statusName"] = labels.sub_status_name(row.get("status"))
    devices = row.get("deviceList")
    if devices is None:
        row["thisDeviceReceives"] = True          # 未设置 → 所有设备都收
    elif isinstance(devices, list):
        row["thisDeviceReceives"] = device_id in [str(d) for d in devices]
    row.setdefault("description", "")
    row.setdefault("serviceParams", "")
    row.setdefault("trailStartTime", None)         # CLI 字段名就是 trail（非 trial）
    row.setdefault("trailEndTime", None)


def _fmt_local_ms(value: Any) -> str:
    """epoch 毫秒 → 本地时间字符串（CLI 的 `lastOnlineLocal` 格式: 'YYYY-mm-dd HH:MM:SS +08:00'）。"""
    try:
        ms = int(value)
    except (TypeError, ValueError):
        return ""
    import datetime as _dt

    local = _dt.datetime.fromtimestamp(ms / 1000.0).astimezone()
    offset = local.strftime("%z")
    if len(offset) == 5:
        offset = f"{offset[:3]}:{offset[3:]}"
    return local.strftime("%Y-%m-%d %H:%M:%S ") + offset
