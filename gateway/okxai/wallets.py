"""本地wallet 元数据读取（wallets.json）——跟账号选择/展示有关，无网络。"""

from __future__ import annotations

import json
from typing import Any

from . import home as _home


def load() -> dict[str, Any]:
    path = _home.path("wallets.json")
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:  # noqa: BLE001
        return {}


def account_list() -> list[dict[str, Any]]:
    data = load()
    accounts = data.get("accounts")
    if isinstance(accounts, list):
        return [a for a in accounts if isinstance(a, dict)]
    m = data.get("accountsMap")
    if isinstance(m, dict):
        return [v for v in m.values() if isinstance(v, dict)]
    return []


def selected_account_id() -> str:
    return str(load().get("selectedAccountId") or "")


def account_name(account_id: str | None = None) -> str:
    target = account_id or selected_account_id()
    for a in account_list():
        if str(a.get("accountId")) == target:
            return str(a.get("accountName") or "")
    return ""


def email() -> str:
    return str(load().get("email") or "")


def login_type() -> str:
    return str(load().get("loginType") or "")
