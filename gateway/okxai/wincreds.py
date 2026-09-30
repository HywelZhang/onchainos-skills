"""Windows 凭据管理器（OS keyring）读取 —— 与官方 CLI 的存储顺序一致。

官方 CLI（cli/src/keyring_store.rs）优先使用 **OS keyring**
（Windows 上即 Credential Manager，服务名 `onchainos`，条目 `agentic-wallet`），
只有在 OS keyring 不可用时才回退到加密文件 `keyring.enc`。

实测（2026-09-30）: 本机 `keyring.enc` 里的 access token 早已过期，而 CLI 仍可正常
调用 —— 说明 CLI 读的是 Credential Manager 里的**另一份**（更新的）凭据。
gateway 必须按同样顺序读取，否则会拿到陈旧 token 并得到 `10008 access token invalid`。

实现用 ctypes 直接调 Win32 API，不引入第三方依赖。
"""

from __future__ import annotations

import ctypes
import json
from ctypes import wintypes
from typing import Any

CRED_TYPE_GENERIC = 1
SERVICE = "onchainos"
USER = "agentic-wallet"


class _FILETIME(ctypes.Structure):
    _fields_ = [("dwLowDateTime", wintypes.DWORD), ("dwHighDateTime", wintypes.DWORD)]


class _CREDENTIALW(ctypes.Structure):
    _fields_ = [
        ("Flags", wintypes.DWORD),
        ("Type", wintypes.DWORD),
        ("TargetName", wintypes.LPWSTR),
        ("Comment", wintypes.LPWSTR),
        ("LastWritten", _FILETIME),
        ("CredentialBlobSize", wintypes.DWORD),
        ("CredentialBlob", ctypes.POINTER(ctypes.c_byte)),
        ("Persist", wintypes.DWORD),
        ("AttributeCount", wintypes.DWORD),
        ("Attributes", ctypes.c_void_p),
        ("TargetAlias", wintypes.LPWSTR),
        ("UserName", wintypes.LPWSTR),
    ]


_PCREDENTIALW = ctypes.POINTER(_CREDENTIALW)


def _advapi() -> Any:
    return ctypes.WinDLL("advapi32", use_last_error=True)


def enumerate_targets(service: str = SERVICE) -> list[str]:
    """列出凭据库中与 service 相关的条目名（只返回名字）。"""
    advapi = _advapi()
    count = wintypes.DWORD(0)
    creds = ctypes.POINTER(_PCREDENTIALW)()  # CredEnumerateW 返回指针数组
    ok = advapi.CredEnumerateW(None, 0, ctypes.byref(count), ctypes.byref(creds))
    if not ok:
        return []
    names: list[str] = []
    try:
        for i in range(count.value):
            entry = creds[i]
            if not entry:
                continue
            target = entry.contents.TargetName or ""
            if service.lower() in target.lower():
                names.append(target)
    finally:
        advapi.CredFree(creds)
    return names


def read_raw(target: str) -> bytes | None:
    advapi = _advapi()
    cred = _PCREDENTIALW()
    ok = advapi.CredReadW(ctypes.c_wchar_p(target), CRED_TYPE_GENERIC, 0, ctypes.byref(cred))
    if not ok:
        return None
    try:
        size = cred.contents.CredentialBlobSize
        if not size:
            return b""
        buf = ctypes.string_at(cred.contents.CredentialBlob, size)
        return buf
    finally:
        advapi.CredFree(cred)


def _decode(raw: bytes) -> dict[str, str] | None:
    """blob 可能是 UTF-16LE（keyring crate 在 Windows 的写法）或 UTF-8；两种都试。"""
    candidates: list[str] = []
    for enc in ("utf-16-le", "utf-8"):
        try:
            text = raw.decode(enc).strip("\x00").strip()
        except UnicodeDecodeError:
            continue
        if text.startswith("{"):
            candidates.append(text)
    for text in candidates:
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            continue
        if isinstance(data, dict):
            return {str(k): str(v) for k, v in data.items()}
    return None


def read_blob(service: str = SERVICE, user: str = USER) -> tuple[dict[str, str], str]:
    """返回 (blob, 命中的 target)。找不到返回 ({}, "")。

    先试 keyring crate 的规范命名（`service:user` / `service.user`），
    再枚举含 service 的条目兜底。
    """
    for target in (f"{service}:{user}", f"{service}.{user}", f"{service}/{user}", user):
        raw = read_raw(target)
        if raw:
            blob = _decode(raw)
            if blob:
                return blob, target
    for target in enumerate_targets(service):
        raw = read_raw(target)
        if not raw:
            continue
        blob = _decode(raw)
        if blob and "refresh_token" in blob:
            return blob, target
    return {}, ""


def is_available() -> bool:
    try:
        return bool(enumerate_targets())
    except Exception:  # noqa: BLE001 - 非 Windows 或缺 API 时视为不可用
        return False


CRED_PERSIST_LOCAL_MACHINE = 2


def write_blob(blob: dict[str, str], target: str = f"{USER}.{SERVICE}") -> bool:
    """写回 OS keyring（token 轮换后必须持久化，否则下次要重新登录）。

    编码与官方 CLI 一致：UTF-16LE 的 JSON 字符串。
    """
    advapi = _advapi()
    payload = json.dumps(blob, ensure_ascii=False, separators=(",", ":")).encode("utf-16-le")
    buf = ctypes.create_string_buffer(payload, len(payload))

    cred = _CREDENTIALW()
    cred.Flags = 0
    cred.Type = CRED_TYPE_GENERIC
    cred.TargetName = ctypes.c_wchar_p(target)
    cred.Comment = ctypes.c_wchar_p("okxai gateway token store")
    cred.CredentialBlobSize = len(payload)
    cred.CredentialBlob = ctypes.cast(buf, ctypes.POINTER(ctypes.c_byte))
    cred.Persist = CRED_PERSIST_LOCAL_MACHINE
    cred.AttributeCount = 0
    cred.Attributes = None
    cred.TargetAlias = None
    cred.UserName = ctypes.c_wchar_p(USER)
    try:
        return bool(advapi.CredWriteW(ctypes.byref(cred), 0))
    finally:
        del buf
