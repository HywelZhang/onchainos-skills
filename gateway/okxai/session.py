"""会话层 —— JWT 生命周期、agenticId 解析、请求头组装（替代 CLI 的 L2）。

复刻自 `cli/src/client.rs`（匿名/JWT 头）、`cli/src/commands/agentic_wallet/auth/mod.rs`
（refresh）、`cli/src/commands/agent_commerce/identity/queries.rs`（agent-list 查询）。

与原 CLI 的差异（有意为之）:
    * token 轮换后**立即写回** keyring.enc，避免 refresh token 单次有效导致的掉线；
    * 身份（agenticId / 钱包地址）**进程内缓存**，这正是原 CLI 每次调用都重新
      查询身份（audit 里 58% 调用是身份/心跳）的根因，gateway 一次性解析后复用。
"""

from __future__ import annotations

import base64
import json
import time
from dataclasses import dataclass, field
from typing import Any

from . import keystore
from .transport import Client, HttpError, TransportError

CLIENT_VERSION = "4.5.2"  # 与测得的实装版本一致；仅作为 ok-client-version 上报
XLAYER_CHAIN_INDEX = "196"
ROLE_CODES = {"user": "1", "asp": "2", "evaluator": "3"}
TOKEN_EXPIRY_MARGIN_S = 60


class NotLoggedIn(RuntimeError):
    pass


class ApiError(RuntimeError):
    def __init__(self, code: str, msg: str, path: str = ""):
        self.code = code
        self.msg = msg
        self.path = path
        super().__init__(f"API error (code={code}): {msg}")


@dataclass
class TokenPair:
    access_token: str
    refresh_token: str


def _jwt_exp(token: str) -> int | None:
    parts = token.split(".")
    if len(parts) != 3:
        return None
    payload = parts[1]
    payload += "=" * (-len(payload) % 4)
    try:
        data = json.loads(base64.urlsafe_b64decode(payload).decode("utf-8"))
    except Exception:  # noqa: BLE001
        return None
    exp = data.get("exp")
    return int(exp) if isinstance(exp, (int, float)) else None


def _expired(token: str) -> bool:
    exp = _jwt_exp(token)
    if exp is None:
        return True
    return time.time() >= exp - TOKEN_EXPIRY_MARGIN_S


_DEVICE_NAME: str | None = None


def device_name() -> str:
    """OS 设备显示名（`device-name` 头）。官方 CLI 读系统名，这里用等价来源，只读一次。"""
    global _DEVICE_NAME
    if _DEVICE_NAME is None:
        import os
        import platform

        raw = os.environ.get("COMPUTERNAME") or platform.node() or ""
        _DEVICE_NAME = raw.strip()[:128] or "unknown-device"
    return _DEVICE_NAME


def unwrap(body: Any, path: str = "") -> Any:
    """统一解析服务端信封：{code,msg,data} / {ok,data} / 裸 JSON。

    官方 CLI 只把 `data` 交给上层；这里保持一致，非 0 code 抛 ApiError。
    """
    if isinstance(body, dict):
        if "ok" in body and isinstance(body.get("ok"), bool):
            if body["ok"]:
                return body.get("data")
            raise ApiError(str(body.get("code", "?")), str(body.get("error") or body.get("msg") or "unknown"), path)
        if "code" in body:
            code = str(body.get("code"))
            if code not in ("0", "200", ""):
                msg = body.get("msg") or body.get("message") or body.get("errorMessage") or "unknown error"
                raise ApiError(code, str(msg), path)
            return body.get("data")
    return body


@dataclass
class Session:
    """登录态 + 身份缓存。一个进程内只解析一次身份。"""

    client: Client = field(default_factory=Client)
    tokens: TokenPair | None = None
    session_cert: str = ""
    device_id: str = ""
    _agents: list[dict[str, Any]] | None = None

    # ── 基础 ──────────────────────────────────────────────────────────
    @classmethod
    def load(cls, client: Client | None = None) -> "Session":
        s = cls(client=client or Client())
        blob = keystore.read_blob()
        access = (blob.get("access_token") or "").strip()
        refresh = (blob.get("refresh_token") or "").strip()
        if not access and not refresh:
            raise NotLoggedIn("未登录：keyring.enc 中没有 token（官方 CLI 里用 `onchainos wallet login`）")
        s.tokens = TokenPair(access_token=access, refresh_token=refresh)
        meta = keystore.read_session()
        s.session_cert = str(meta.get("sessionCert") or "")
        s.device_id = str(meta.get("deviceId") or "")
        session_exp = str(meta.get("sessionKeyExpireAt") or "")
        if session_exp.isdigit() and int(session_exp) <= time.time():
            raise NotLoggedIn("会话已过期（sessionKeyExpireAt 已到）：需要重新登录社交账号")
        return s

    # ── token ─────────────────────────────────────────────────────────
    def ensure_token(self) -> str:
        assert self.tokens is not None
        if self.tokens.access_token and not _expired(self.tokens.access_token):
            return self.tokens.access_token
        return self.refresh()

    def refresh(self) -> str:
        """POST /priapi/v5/wallet/agentic/auth/refresh（公开端点，无需 JWT）。

        实测教训（2026-09-30）: 该端点虽然不要求 JWT，但要求**整套匿名头**
        （`device-id` / `ok-client-version` / `Ok-Access-Client-type` / `platform`）。
        只发 `{"refreshToken"}` 会被拒：HTTP 400 `50113 Client signature public key missing`
        —— 服务端按 device-id 找回该设备注册的客户端公钥，缺头就找不到。
        """
        assert self.tokens is not None
        if not self.tokens.refresh_token:
            raise NotLoggedIn("access token 已过期且没有 refresh token，需要重新登录")
        if _expired(self.tokens.refresh_token):
            raise NotLoggedIn("refresh token 已过期，需要重新登录")
        body = self.client.request(
            "POST",
            "/priapi/v5/wallet/agentic/auth/refresh",
            body={"refreshToken": self.tokens.refresh_token},
            headers=self.anon_headers(),
        )
        item = unwrap(body, "auth/refresh")
        if isinstance(item, list):
            item = item[0] if item else {}
        if not isinstance(item, dict) or not item.get("accessToken"):
            raise ApiError("?", f"auth/refresh 响应缺少 accessToken: {item!r}", "auth/refresh")
        self.tokens = TokenPair(
            access_token=str(item["accessToken"]),
            refresh_token=str(item.get("refreshToken") or self.tokens.refresh_token),
        )
        self._persist_tokens()
        return self.tokens.access_token

    def _persist_tokens(self) -> None:
        """token 轮换后立即落盘（refresh token 可能单次有效）。"""
        assert self.tokens is not None
        blob = keystore.read_blob()
        blob["access_token"] = self.tokens.access_token
        blob["refresh_token"] = self.tokens.refresh_token
        keystore.write_blob(blob)

    # ── 头 ────────────────────────────────────────────────────────────
    def anon_headers(self) -> dict[str, str]:
        """官方 `ApiClient::anonymous_headers()` 的等价物（refresh 等公开端点也要求）。"""
        h = {
            "ok-client-version": CLIENT_VERSION,
            "Ok-Access-Client-type": "agent-cli",
            "platform": "agent-cli",
        }
        if self.device_id:
            h["device-id"] = self.device_id
        name = device_name()
        if name:
            h["device-name"] = name
        return h

    def headers(self, agent_id: str | None = None) -> dict[str, str]:
        token = self.ensure_token()
        h = self.anon_headers()
        h["Authorization"] = f"Bearer {token}"
        if agent_id:
            h["agenticId"] = str(agent_id)
        return h

    # ── 请求封装 ──────────────────────────────────────────────────────
    def get(self, path: str, *, query: dict[str, str] | None = None, agent_id: str | None = None,
            auth: bool = True, with_cert: bool = False) -> Any:
        q = dict(query or {})
        if with_cert and self.session_cert:
            q["sessionCert"] = self.session_cert
        body = self.client.request(
            "GET", path, query=q, headers=self.headers(agent_id) if auth else None
        )
        return unwrap(body, path)

    def post(self, path: str, *, body: dict[str, Any] | None = None, agent_id: str | None = None,
             auth: bool = True, inject_cert: bool = False) -> Any:
        payload = dict(body or {})
        if inject_cert and self.session_cert and "sessionCert" not in payload:
            payload["sessionCert"] = self.session_cert
        raw = self.client.request(
            "POST", path, body=payload, headers=self.headers(agent_id) if auth else None
        )
        return unwrap(raw, path)

    # ── 身份（缓存，替代 CLI 的每次重新解析）──────────────────────────
    def agents(self, refresh: bool = False, owner_address: str | None = None) -> list[dict[str, Any]]:
        if self._agents is not None and not refresh and not owner_address:
            return self._agents
        query = {"chainIndex": XLAYER_CHAIN_INDEX, "pageSize": "100"}
        if owner_address:
            query["ownerAddress"] = owner_address
        data = self.get("/priapi/v5/wallet/agentic/agent/agent-list", query=query)
        rows = _flatten_agents(data)
        if not owner_address:
            self._agents = rows
        return rows

    def agent_id(self, role: str = "user", agent_id: str | None = None) -> str:
        """解析 agenticId：显式给定 > 按 role 找第一个 > 报错。"""
        if agent_id:
            return str(agent_id)
        role_code = ROLE_CODES.get(role)
        if not role_code:
            raise ValueError(f"unknown role {role!r}; expected one of {sorted(ROLE_CODES)}")
        for row in self.agents():
            if str(row.get("role")) in (role_code, role):
                return str(row.get("agentId"))
        raise ApiError("?", f"当前账户下找不到 role={role} 的 agent（先 register）", "agent-list")


def _flatten_agents(data: Any) -> list[dict[str, Any]]:
    """从 agent-list 的任意嵌套形状里摘出 agent 行，并继承所属账户名。

    实测形状（4.5.2）: `data` = `[{list:[{accountName, agentList:[{agentId,...}]}]}]`
    —— 账户组 → 组内 agent 列表。CLI 扁平化时会把 `accountName` 带到每行上
    （`my-agents` 的输出即如此），这里保持同一行为。
    """
    rows: list[dict[str, Any]] = []

    def walk(node: Any, account_name: str = "") -> None:
        if isinstance(node, list):
            for item in node:
                walk(item, account_name)
            return
        if not isinstance(node, dict):
            return
        account_name = str(node.get("accountName") or account_name)
        if node.get("agentId"):
            row = dict(node)
            if account_name:
                row.setdefault("accountName", account_name)
            rows.append(row)
            return
        for key in ("agentList", "list", "data", "agents"):
            if key in node:
                walk(node[key], account_name)

    walk(data)
    return rows
