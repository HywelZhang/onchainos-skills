"""P1 写路径 verb（v1.2 边界：签名类委托官方 CLI，纯后端类直连 HTTP）。

归口规则（docs/design/11 §12.3）:
    * 需要**链上签名**的动作 → `delegate.run(["agent", ...])`：
      创建任务/订阅、接单、交付、验收/拒绝/关闭、自动退款、收益领取、退款、评分。
    * **纯后端**动作（无签名）→ 直连 HTTP：
      订阅设备列表、离线接收标记（以及后续的可见性/serviceParam）。
"""

from __future__ import annotations

from typing import Any

from . import delegate
from .verbs import REGISTRY, Session, VerbError, verb  # noqa: F401 - REGISTRY 供 cli/server 复用

SUBSCRIBE_API_PREFIX = "/priapi/v1/aieco/task/subscribe"


def _delegated(command: list[str], flags: dict[str, Any], *, timeout: int = 240,
               agent_id: str | None = None) -> dict[str, Any]:
    """执行委托并补上流程元数据（agent 侧只看 ok/data/rendered + nextAction）。"""
    flags = {k: v for k, v in (flags or {}).items() if v is not None}
    if agent_id and "agent_id" not in flags:
        flags["agent_id"] = agent_id
    result = delegate.run(command, flags=flags, timeout=timeout)
    result.setdefault("phase", "done" if result.get("ok") else "error")
    result.setdefault("decision", "ready" if result.get("ok") else "blocked")
    result["executor"] = "cli:" + " ".join(command[:2])
    return result


# ─────────────────────────── 任务（委托） ───────────────────────────


@verb("task.create", "创建任务（含出资，链上写）", args=("description", "budget", "provider", "service_id"))
def task_create(s: Session, description: str | None = None, budget: str | None = None,
                provider: str | None = None, service_id: str | None = None,
                max_budget: str | None = None, currency: str = "USDT", title: str | None = None,
                payment_mode: str = "escrow", service_params: str | None = None,
                service_token_amount: str | None = None, service_token_address: str | None = None,
                endpoint: str | None = None, files: Any = None, **_: Any) -> dict[str, Any]:
    missing = [k for k, v in (("description", description), ("budget", budget),
                              ("provider", provider), ("service_id", service_id)) if not v]
    if missing:
        raise VerbError(f"task.create 缺少必填参数: {', '.join(missing)}")
    return _delegated(["agent", "create-task"], {
        "description": description,
        "budget": budget,
        "max_budget": max_budget or budget,
        "currency": currency,
        "provider": provider,
        "payment_mode": payment_mode,
        "service_id": service_id,
        "title": title,
        "service_params": service_params,
        "service_token_amount": service_token_amount,
        "service_token_address": service_token_address,
        "endpoint": endpoint,
        "file": files,
    })


@verb("task.setPaymentMode", "设置任务的支付方式（链上写）", args=("job_id",))
def task_set_payment_mode(s: Session, job_id: str | None = None, payment_mode: str = "escrow",
                          token_symbol: str | None = None, token_amount: str | None = None,
                          endpoint: str | None = None, **_: Any) -> dict[str, Any]:
    if not job_id:
        raise VerbError("task.setPaymentMode 需要 job_id")
    return _delegate_with_positional(["agent", "set-payment-mode"], job_id, {
        "payment_mode": payment_mode, "token_symbol": token_symbol,
        "token_amount": token_amount, "endpoint": endpoint,
    })


def _delegate_with_positional(command: list[str], positional: str | None, flags: dict[str, Any],
                              timeout: int = 240) -> dict[str, Any]:
    """带位置参数（如 <JOB_ID>）的委托。"""
    result = delegate.run(command, flags={k: v for k, v in flags.items() if v is not None},
                          positionals=[positional] if positional else [], timeout=timeout)
    result.setdefault("phase", "done" if result.get("ok") else "error")
    result.setdefault("decision", "ready" if result.get("ok") else "blocked")
    result["executor"] = "cli:" + " ".join(command[:2])
    return result


@verb("task.confirmAccept", "买家确认接单并付款（链上写；需先 setPaymentMode）", args=("job_id",))
def task_confirm_accept(s: Session, job_id: str | None = None, **_: Any) -> dict[str, Any]:
    if not job_id:
        raise VerbError("task.confirmAccept 需要 job_id")
    return _delegate_with_positional(["agent", "confirm-accept"], job_id, {})


@verb("task.accept", "买家验收接单（= setPaymentMode + confirmAccept 组合）", args=("job_id",))
def task_accept(s: Session, job_id: str | None = None, payment_mode: str = "escrow",
                token_symbol: str | None = None, token_amount: str | None = None,
                **_: Any) -> dict[str, Any]:
    if not job_id:
        raise VerbError("task.accept 需要 job_id")
    steps: list[dict[str, Any]] = []
    mode = _delegate_with_positional(["agent", "set-payment-mode"], job_id, {
        "payment_mode": payment_mode, "token_symbol": token_symbol, "token_amount": token_amount,
    })
    steps.append({"step": "set-payment-mode", **mode})
    if not mode.get("ok"):
        return {"ok": False, "phase": "set_payment_mode", "decision": "blocked",
                "error": mode.get("error"), "steps": steps}
    accept = _delegate_with_positional(["agent", "confirm-accept"], job_id, {})
    steps.append({"step": "confirm-accept", **accept})
    return {"ok": bool(accept.get("ok")), "phase": "accept" if accept.get("ok") else "confirm_accept",
            "decision": "ready" if accept.get("ok") else "blocked",
            "data": accept.get("data"), "error": accept.get("error"), "steps": steps,
            "executor": "cli:agent confirm-accept"}


@verb("task.apply", "ASP 接单（链上写）", args=("job_id", "agent_id", "token_amount"))
def task_apply(s: Session, job_id: str | None = None, agent_id: str | None = None,
               token_amount: str | None = None, token_symbol: str = "USDT", **_: Any) -> dict[str, Any]:
    if not (job_id and agent_id and token_amount):
        raise VerbError("task.apply 需要 job_id/agent_id/token_amount")
    return _delegate_with_positional(["agent", "apply"], job_id, {
        "agent_id": agent_id, "token_amount": token_amount, "token_symbol": token_symbol,
    })


@verb("task.deliver", "ASP 交付（链上写）", args=("job_id", "agent_id"))
def task_deliver(s: Session, job_id: str | None = None, agent_id: str | None = None,
                 message: str | None = None, deliverable_text: str | None = None,
                 files: Any = None, **_: Any) -> dict[str, Any]:
    if not (job_id and agent_id):
        raise VerbError("task.deliver 需要 job_id/agent_id")
    return _delegate_with_positional(["agent", "deliver"], job_id, {
        "agent_id": agent_id, "message": message, "deliverable_text": deliverable_text, "file": files,
    })


@verb("task.complete", "买家验收通过并放款（链上写·双签）", args=("job_id",))
def task_complete(s: Session, job_id: str | None = None, **_: Any) -> dict[str, Any]:
    if not job_id:
        raise VerbError("task.complete 需要 job_id")
    return _delegate_with_positional(["agent", "complete"], job_id, {})


@verb("task.reject", "买家拒收交付物（链上写·双签）", args=("job_id", "reason"))
def task_reject(s: Session, job_id: str | None = None, reason: str | None = None, **_: Any) -> dict[str, Any]:
    if not (job_id and reason):
        raise VerbError("task.reject 需要 job_id/reason")
    return _delegate_with_positional(["agent", "reject"], job_id, {"reason": reason})


@verb("task.close", "买家关闭任务（仅 Open 态合法；链上写）", args=("job_id",))
def task_close(s: Session, job_id: str | None = None, agent_id: str | None = None, **_: Any) -> dict[str, Any]:
    if not job_id:
        raise VerbError("task.close 需要 job_id")
    return _delegate_with_positional(["agent", "close"], job_id, {"agent_id": agent_id})


@verb("task.claimAutoRefund", "买家在 ASP 超时后领取自动退款（链上写）", args=("job_id",))
def task_claim_auto_refund(s: Session, job_id: str | None = None, **_: Any) -> dict[str, Any]:
    if not job_id:
        raise VerbError("task.claimAutoRefund 需要 job_id")
    return _delegate_with_positional(["agent", "claim-auto-refund"], job_id, {})


@verb("task.setAsp", "指定/替换 ASP 与服务（off-chain 触发 job_asp_selected）", args=("job_id",))
def task_set_asp(s: Session, job_id: str | None = None, provider_agent_id: str | None = None,
                 service_id: str | None = None, **_: Any) -> dict[str, Any]:
    if not (job_id and provider_agent_id and service_id):
        raise VerbError("task.setAsp 需要 job_id/provider_agent_id/service_id")
    return _delegate_with_positional(["agent", "set-asp"], job_id, {
        "provider_agent_id": provider_agent_id, "service_id": service_id,
    })


@verb("task.userReject", "买家拒绝当前 ASP（off-chain）", args=("job_id",))
def task_user_reject(s: Session, job_id: str | None = None, agent_id: str | None = None, **_: Any) -> dict[str, Any]:
    if not job_id:
        raise VerbError("task.userReject 需要 job_id")
    return _delegate_with_positional(["agent", "user-reject"], job_id, {"agent_id": agent_id})


# ─────────────────────────── 订阅（委托） ───────────────────────────


@verb("sub.create", "创建订阅（链上写）", args=("service_id", "service_token_amount", "title"))
def sub_create(s: Session, service_id: str | None = None, service_token_amount: str | None = None,
               service_token_address: str | None = None, auto_renew: str = "false",
               title: str | None = None, description: str | None = None,
               use_trial: Any = None, service_params: str | None = None,
               provider_agent_id: str | None = None, service_interval: str | None = None,
               autotrade_mode: str | None = None, autotrade_amount: str | None = None,
               autotrade_cap: str | None = None, autotrade_quote: str | None = None,
               **_: Any) -> dict[str, Any]:
    missing = [k for k, v in (("service_id", service_id), ("service_token_amount", service_token_amount),
                              ("title", title)) if not v]
    if missing:
        raise VerbError(f"sub.create 缺少必填参数: {', '.join(missing)}")
    if not service_token_address:
        raise VerbError("sub.create 需要 service_token_address（服务计价代币合约地址）")
    return _delegated(["agent", "create-subscribe"], {
        "service_id": service_id,
        "service_token_amount": service_token_amount,
        "service_token_address": service_token_address,
        "auto_renew": auto_renew,
        "title": title,
        "description": description,
        "use_trial": use_trial,
        "service_params": service_params,
        "provider_agent_id": provider_agent_id,
        "service_interval": service_interval,
        "autotrade_mode": autotrade_mode,
        "autotrade_amount": autotrade_amount,
        "autotrade_cap": autotrade_cap,
        "autotrade_quote": autotrade_quote,
    })


@verb("sub.cancel", "取消订阅（试用取消 + 关闭自动续费）", args=("sub_id",))
def sub_cancel(s: Session, sub_id: str | None = None, **_: Any) -> dict[str, Any]:
    if not sub_id:
        raise VerbError("sub.cancel 需要 sub_id")
    return _delegate_with_positional(["agent", "subscribe-cancel"], sub_id, {})


@verb("sub.autorenew", "开启自动续费（需 EIP-712 条款签名）", args=("sub_id",))
def sub_autorenew(s: Session, sub_id: str | None = None, **_: Any) -> dict[str, Any]:
    if not sub_id:
        raise VerbError("sub.autorenew 需要 sub_id")
    return _delegate_with_positional(["agent", "start-autorenew"], sub_id, {})


@verb("rating.submit", "提交对 ASP 的评价（链上写）", args=("task_id", "creator_id", "score"))
def rating_submit(s: Session, task_id: str | None = None, creator_id: str | None = None,
                  score: Any = None, description: str | None = None, agent_id: str | None = None,
                  **_: Any) -> dict[str, Any]:
    if not (task_id and creator_id and score is not None):
        raise VerbError("rating.submit 需要 task_id/creator_id/score")
    return _delegated(["agent", "feedback-submit"], {
        "task_id": task_id, "creator_id": creator_id, "score": score,
        "description": description, "agent_id": agent_id,
    })


# ─────────────────────────── 订阅（直连 HTTP，无签名） ───────────────────────────


@verb("sub.device", "覆盖订阅的接收设备列表（纯后端；默认按 CLI 语义'空=清空=所有设备收'）",
      args=("job_id", "device_list"))
def sub_device(s: Session, job_id: str | None = None, device_list: Any = None,
               role: str = "user", explicit_empty: Any = False, **_: Any) -> dict[str, Any]:
    """POST /task/subscribe/device/batchUpdate，body {"items":[{"jobId","deviceList"}]}。

    ⚠️ 实测语义陷阱（2026-09-30，我俩都踩过）:
        * `deviceList: null`（字段缺失）→ **所有设备都收**（thisDeviceReceives = true）
        * `deviceList: []`        → **没有任何设备收**（thisDeviceReceives = false，会静默停掉信号投递）
    官方 CLI 把"空/省略"解释为**清空**（→ 所有设备收）。本 verb 采用同一约定：
        * None / "" / []  → 省略该字段 = 清空（恢复"所有设备收"）
        * 显式传 `explicit_empty=True` 才会发出真正的空列表（"谁都别收"）
    """
    if not job_id:
        raise VerbError("sub.device 需要 job_id")

    devices: list[str] = []
    provided = device_list is not None and device_list != ""
    if isinstance(device_list, (list, tuple)):
        devices = [str(d) for d in device_list]
    elif isinstance(device_list, str) and device_list.strip():
        devices = [d.strip() for d in device_list.split(",") if d.strip()]

    item: dict[str, Any] = {"jobId": job_id}
    if explicit_empty:
        item["deviceList"] = devices
    elif provided and devices:
        item["deviceList"] = devices
    # 否则：不带 deviceList 字段 = 清空（所有设备收）

    aid = s.agent_id(role)
    return s.post(f"{SUBSCRIBE_API_PREFIX}/device/batchUpdate", body={"items": [item]},
                  agent_id=aid, inject_cert=True, retry=True)


@verb("sub.offline", "设置订阅的离线接收标记（0=保留积压, 1=丢弃积压）", args=("sub_id", "flag"))
def sub_offline(s: Session, sub_id: str | None = None, flag: Any = 0, role: str = "user",
                **_: Any) -> dict[str, Any]:
    if not sub_id:
        raise VerbError("sub.offline 需要 sub_id")
    try:
        value = int(flag)
    except (TypeError, ValueError):
        raise VerbError(f"flag 必须是 0 或 1，收到 {flag!r}") from None
    aid = s.agent_id(role)
    # 字段名是服务端的 `offlineReceiveFlag`（实测：传 `flag` 会被拒 code=1001）
    return s.post(f"{SUBSCRIBE_API_PREFIX}/{sub_id}/setOfflineReceiveFlag",
                  body={"offlineReceiveFlag": value}, agent_id=aid, inject_cert=True, retry=True)


# ─────────────────────────── 兜底：受控原始委托 ───────────────────────────


@verb("cli.raw", "受控原始委托（白名单内 agent 子命令，迁移期兜底）", args=("command", "args"))
def cli_raw(s: Session, command: str | None = None, args: str | None = None, **_: Any) -> dict[str, Any]:
    """`command` 形如 "agent subscription-route-set"，`args` 为空格分隔的原始 flag 串。

    仅用于尚未映射的 okx-ai 子命令；越权命令会被 delegate 拒绝。
    """
    if not command:
        raise VerbError("cli.raw 需要 command（例如 'agent status'）")
    argv = command.split()
    if args:
        argv.extend(args.split())
    result = delegate.run(argv)
    result.setdefault("phase", "done" if result.get("ok") else "error")
    result.setdefault("decision", "ready" if result.get("ok") else "blocked")
    result["executor"] = "cli:" + " ".join(argv[:2])
    return result
