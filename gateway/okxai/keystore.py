"""本地凭据库读写（与官方 CLI 共用同一格式）。

格式依据 `cli/src/file_keyring.rs` / `cli/src/keyring_store.rs`：

* 凭据文件: `$ONCHAINOS_HOME/keyring.enc`
      salt(32) || nonce(12) || AES-256-GCM(ciphertext||tag)
* 密钥派生: `scrypt(identity, salt, log_n=15, r=8, p=1, dklen=32)`
* identity: `$ONCHAINOS_HOME/machine-identity` 的内容（64 位 hex，trim 后使用）
* 明文: 一个 JSON 字典（键如 access_token / refresh_token / session_key）
* 会话元数据: `$ONCHAINOS_HOME/session.json`（sessionCert / encryptedSessionSk / deviceId …）

写回采用临时文件 + 原子替换，避免与既有文件竞争时留下半截内容。
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
from typing import Any

from . import home as _home

SALT_LEN = 32
NONCE_LEN = 12
SCRYPT_LOG_N = 15
SCRYPT_R = 8
SCRYPT_P = 1
KEY_LEN = 32


class KeystoreError(RuntimeError):
    pass


def _identity() -> str:
    p = _home.path("machine-identity")
    if not p.exists():
        raise KeystoreError(
            f"missing {p} — 该文件是本机凭据库的解密身份，缺失时无法读取 token"
        )
    value = p.read_text(encoding="utf-8").strip()
    if not value:
        raise KeystoreError(f"{p} is empty")
    return value


def _derive_key(identity: str, salt: bytes) -> bytes:
    return hashlib.scrypt(
        identity.encode("utf-8"),
        salt=salt,
        n=2**SCRYPT_LOG_N,
        r=SCRYPT_R,
        p=SCRYPT_P,
        dklen=KEY_LEN,
        maxmem=128 * 1024 * 1024,
    )


def keystore_backend() -> str:
    """'os'（Windows 凭据管理器）/ 'file'（keyring.enc）/ 'none'。"""
    try:
        from . import wincreds

        if wincreds.is_available():
            return "os"
    except Exception:  # noqa: BLE001
        pass
    return "file" if _home.path("keyring.enc").exists() else "none"


def read_blob() -> dict[str, str]:
    """解密凭据 blob。

    顺序与官方 CLI 一致（cli/src/keyring_store.rs）: **先 OS keyring，再文件回退**。
    实测教训（2026-09-30）: 本机两处并存且内容不同 —— 文件里的 token 早已过期，
    Credential Manager 里的是新鲜的；只读文件会稳定拿到 `10008 access token invalid`。
    """
    try:
        from . import wincreds

        blob, _target = wincreds.read_blob()
        if blob:
            return blob
    except Exception:  # noqa: BLE001 - 非 Windows / API 不可用时静默回退
        pass
    return _read_file_blob()


def _read_file_blob() -> dict[str, str]:
    path = _home.path("keyring.enc")
    if not path.exists():
        return {}
    data = path.read_bytes()
    if len(data) < SALT_LEN + NONCE_LEN + 1:
        raise KeystoreError("keyring.enc is corrupted (too short)")

    salt, rest = data[:SALT_LEN], data[SALT_LEN:]
    nonce, ciphertext = rest[:NONCE_LEN], rest[NONCE_LEN:]

    try:
        from cryptography.hazmat.primitives.ciphers.aead import AESGCM
    except ImportError as exc:  # pragma: no cover - env issue
        raise KeystoreError(
            "cryptography 未安装：请用带该依赖的解释器运行 "
            "(例如 %LOCALAPPDATA%/hermes/hermes-agent/venv/Scripts/python.exe)"
        ) from exc

    key = _derive_key(_identity(), salt)
    try:
        plaintext = AESGCM(key).decrypt(nonce, ciphertext, None)
    except Exception as exc:  # noqa: BLE001 - 统一成可读错误
        raise KeystoreError(
            "keyring.enc 解密失败：machine-identity 与文件不匹配（换机/换用户会导致此错）"
        ) from exc

    try:
        blob = json.loads(plaintext.decode("utf-8"))
    except Exception as exc:  # noqa: BLE001
        raise KeystoreError("keyring.enc 明文不是合法 JSON") from exc
    if not isinstance(blob, dict):
        raise KeystoreError("keyring.enc 明文不是对象")
    return {str(k): str(v) for k, v in blob.items()}


def write_blob(blob: dict[str, str]) -> None:
    """写回凭据（token 轮换后必须持久化，否则下次要重新登录）。

    与 CLI 同序：优先 OS keyring；同时镜像到 keyring.enc（两处并存时保持同步，
    避免后续读到陈旧 token —— 这正是我们踩过的坑）。
    """
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
    import secrets

    wrote_os = False
    try:
        from . import wincreds

        wrote_os = wincreds.write_blob(blob)
    except Exception:  # noqa: BLE001
        wrote_os = False

    salt = secrets.token_bytes(SALT_LEN)
    nonce = secrets.token_bytes(NONCE_LEN)
    key = _derive_key(_identity(), salt)
    payload = json.dumps(blob, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    ciphertext = AESGCM(key).encrypt(nonce, payload, None)

    target = _home.ensure_home() / "keyring.enc"
    tmp = target.with_suffix(".enc.tmp")
    tmp.write_bytes(salt + nonce + ciphertext)
    os.replace(tmp, target)

    if not wrote_os and not _home.path("machine-identity").exists():
        raise KeystoreError("凭据写回失败：OS keyring 不可用且缺少 machine-identity")


def read_session() -> dict[str, Any]:
    """会话元数据（session.json）；缺失返回空字典。"""
    path = _home.path("session.json")
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001
        raise KeystoreError("session.json 不是合法 JSON") from exc


def get(key: str, default: str | None = None) -> str | None:
    return read_blob().get(key, default)


def describe() -> dict[str, Any]:
    """诊断信息：只暴露键名与长度，绝不返回任何密文/明文值。"""
    blob = read_blob()
    session = read_session()
    return {
        "home": str(_home.home()),
        "backend": keystore_backend(),
        "blob_keys": sorted(blob.keys()),
        "blob_sizes": {k: len(v) for k, v in blob.items()},
        "session_keys": sorted(session.keys()),
        "device_id": session.get("deviceId", ""),
        "session_key_expire_at": session.get("sessionKeyExpireAt", ""),
        "has_session_cert": bool(session.get("sessionCert")),
        "has_encrypted_session_sk": bool(session.get("encryptedSessionSk")),
    }
