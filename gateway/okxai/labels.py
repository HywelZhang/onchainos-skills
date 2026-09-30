"""文案映射层（CLI 的显示语义，供 skill 渲染直接使用）。

来源: `cli/src/commands/agent_commerce/` 内的 label/desc 表，逐一照搬：
    * 任务状态 → 短名: task/common/query.rs `status_name`
    * 任务状态 → 描述: task/common/mod.rs `status_desc`
    * 订阅状态 → 短名: task/user/subscription_ops.rs `status_name`
    * 评分格式:      identity/utils.rs `format_search_rate`

设计意图: skill 只呈现 `statusName` / `statusDescription`，绝不呈现原始 code
（与官方 skill 的渲染契约一致）；把这张表放在 gateway 里，是为了让"文案"和
"状态机"待在一起，避免 skill 里再出现一份需要跟着服务端变的映射。
"""

from __future__ import annotations

TASK_STATUS: dict[int, str] = {
    0: "created",
    1: "accepted",
    2: "submitted",
    3: "rejected",
    4: "disputed",
    5: "admin_stopped",
    6: "complete",
    7: "close",
    8: "expired",
    9: "failed",
}

TASK_STATUS_DESC: dict[str, str] = {
    "init": "Initializing (awaiting on-chain confirmation)",
    "created": "Awaiting acceptance (Created)",
    "accepted": "Accepted; ASP executing (Accepted)",
    "submitted": "ASP submitted deliverable; awaiting User Agent review (Submitted)",
    "rejected": "User Agent rejected deliverable; evaluation possible within freeze period (Rejected)",
    "disputed": "Evaluation in progress (Disputed)",
    "admin_stopped": "Admin stopped the task (AdminStopped)",
    "completed": "Task completed; funds released (Complete)",
    "complete": "Task completed; funds released (Complete)",
    "failed": "Evaluation concluded; task closed (Failed)",
    "close": "User Agent closed the task (Close)",
    "expired": "Task expired (Expired)",
}

SUB_STATUS: dict[int, str] = {
    -1: "INIT",
    1: "ACTIVE",
    3: "REJECTED",
    4: "DISPUTED",
    6: "COMPLETED",
    7: "CLOSED",
    9: "FAILED",
}

ROLE_LABEL: dict[str, str] = {"1": "User", "2": "ASP", "3": "Evaluator", "user": "User", "asp": "ASP", "evaluator": "Evaluator"}

RATING_PLACEHOLDER = "—"


def task_status_name(code: object) -> str:
    try:
        return TASK_STATUS.get(int(code), "unknown")  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return "unknown"


def task_status_description(name: str) -> str:
    return TASK_STATUS_DESC.get(name, "Unknown status")


def sub_status_name(status: object) -> str:
    try:
        code = int(status)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return "UNKNOWN"
    return SUB_STATUS.get(code, f"UNKNOWN_{code}")


def format_rate(rate: object) -> str:
    """与 CLI 的 `format_search_rate` 一致: 两位小数后去尾零；无评分 → '—'。"""
    if rate in (None, ""):
        return RATING_PLACEHOLDER
    try:
        value = float(rate)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return RATING_PLACEHOLDER
    text = f"{value:.2f}"
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text
