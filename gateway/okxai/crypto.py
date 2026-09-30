"""密码学层 —— 复刻 CLI 的签名链（P0-11 §4）。

三块能力，全部用 `cryptography` 实现（无第三方加密库）：

1. **HPKE 解出签名 seed**（`hpke_decrypt_seed`）
   服务端 TEE 托管钱包，客户端只持有被 HPKE 包裹的 32 字节 Ed25519 seed。
   Suite（照搬 `cli/src/crypto.rs`）:
       DHKEM(X25519, HKDF-SHA256) + HKDF-SHA256 + AES-256-GCM
       info  = b"okx-tee-sign"
       线格式 = enc(32B) || ciphertext(+16B tag)
   RFC 9180 Base 模式手写实现（`cryptography` 未内置 HPKE）。

2. **Ed25519 签名**（`sign_b64` / `sign_eip191_b64`）
   注意: 服务端要求的是 Ed25519（不是 secp256k1）——设备侧只签 msgHash。

3. **keccak256**（EIP-191 前缀哈希）
   用 OpenSSL 的 `keccak-256`（实测可用；不需要纯 Python 实现）。

安全约定: seed 只在函数内短暂存在，返回前不做任何日志输出；调用方用完应尽快丢弃。
"""

from __future__ import annotations

import base64
import hashlib
from typing import Final

HPKE_INFO: Final[bytes] = b"okx-tee-sign"
ENC_SIZE: Final[int] = 32

# RFC 9180 常量
KEM_ID_X25519_HKDF_SHA256: Final[int] = 0x0020
KDF_ID_HKDF_SHA256: Final[int] = 0x0001
AEAD_ID_AES_256_GCM: Final[int] = 0x0002
MODE_BASE: Final[int] = 0x00

B64 = base64.b64encode
B64D = base64.b64decode


def _i2osp(value: int, length: int) -> bytes:
    return value.to_bytes(length, "big")


def _suite_id_kem() -> bytes:
    return b"KEM" + _i2osp(KEM_ID_X25519_HKDF_SHA256, 2)


def _suite_id_hpke() -> bytes:
    return (
        b"HPKE"
        + _i2osp(KEM_ID_X25519_HKDF_SHA256, 2)
        + _i2osp(KDF_ID_HKDF_SHA256, 2)
        + _i2osp(AEAD_ID_AES_256_GCM, 2)
    )


def _hkdf_extract(salt: bytes, ikm: bytes) -> bytes:
    """RFC 5869 HKDF-Extract **只做** extract（HMAC），不是 Extract+Expand。

    踩坑记录: 最初误用 `HKDF(...).derive()`（= Extract+Expand），导致 HPKE 解出的
    seed 校验失败（InvalidTag）。Extract 必须单独用 HMAC-SHA256 实现。
    """
    import hmac as _hmac
    from hashlib import sha256

    key = salt if salt else b"\x00" * 32
    return _hmac.new(key, ikm, sha256).digest()


def _hkdf_expand(prk: bytes, info: bytes, length: int) -> bytes:
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.kdf.hkdf import HKDFExpand

    return HKDFExpand(algorithm=hashes.SHA256(), length=length, info=info).derive(prk)


def _labeled_extract(salt: bytes, suite_id: bytes, label: bytes, ikm: bytes) -> bytes:
    return _hkdf_extract(salt, b"HPKE-v1" + suite_id + label + ikm)


def _labeled_expand(prk: bytes, suite_id: bytes, label: bytes, info: bytes, length: int) -> bytes:
    return _hkdf_expand(prk, _i2osp(length, 2) + b"HPKE-v1" + suite_id + label + info, length)


def hpke_decrypt_seed(encrypted_b64: str, session_key_b64: str) -> bytes:
    """解开被 HPKE 包裹的 32 字节 Ed25519 seed。

    参数与 CLI 的 `hpke_decrypt_session_sk` 完全一致：
        encrypted_b64   = session.json 的 `encryptedSessionSk`
        session_key_b64 = keyring 的 `session_key`（X25519 私钥，base64）

    优先用 `cryptography.hazmat.primitives.hpke`（RFC 9180 原生实现；cryptography ≥ 45），
    失败则退回本文件的手写 Base 模式实现（老版本 cryptography 上仍可用）。
    两条路径在开发机上已验证给出同一 seed。
    """
    encrypted = B64D(encrypted_b64)
    if len(encrypted) <= ENC_SIZE:
        raise ValueError(f"encryptedSessionSk 过短: {len(encrypted)} 字节（需 > {ENC_SIZE}）")
    sk_bytes = B64D(session_key_b64)
    if len(sk_bytes) != 32:
        raise ValueError(f"session_key 必须是 32 字节，实际 {len(sk_bytes)}")

    try:
        return _hpke_decrypt_seed_lib(encrypted, sk_bytes)
    except ImportError:
        return _hpke_decrypt_seed_manual(encrypted, sk_bytes)


def _hpke_decrypt_seed_lib(encrypted: bytes, sk_bytes: bytes) -> bytes:
    from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
    from cryptography.hazmat.primitives.hpke import AEAD, KDF, KEM, Suite

    suite = Suite(KEM.X25519, KDF.HKDF_SHA256, AEAD.AES_256_GCM)
    private_key = X25519PrivateKey.from_private_bytes(sk_bytes)
    # 注意实参顺序: (ciphertext, private_key, info)
    plaintext = suite.decrypt(encrypted, private_key, HPKE_INFO)
    if len(plaintext) != 32:
        raise ValueError(f"解出的 seed 必须是 32 字节，实际 {len(plaintext)}")
    return plaintext


def _hpke_decrypt_seed_manual(encrypted: bytes, sk_bytes: bytes) -> bytes:
    from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

    enc, ciphertext = encrypted[:ENC_SIZE], encrypted[ENC_SIZE:]
    private_key = X25519PrivateKey.from_private_bytes(sk_bytes)
    pk_rm = private_key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    dh = private_key.exchange(X25519PublicKey.from_public_bytes(enc))

    kem_suite = _suite_id_kem()
    kem_context = enc + pk_rm
    eae_prk = _labeled_extract(b"", kem_suite, b"eae_prk", dh)
    shared_secret = _labeled_expand(eae_prk, kem_suite, b"shared_secret", kem_context, 32)

    hpke_suite = _suite_id_hpke()
    psk_id_hash = _labeled_extract(b"", hpke_suite, b"psk_id_hash", b"")
    info_hash = _labeled_extract(b"", hpke_suite, b"info_hash", HPKE_INFO)
    context = _i2osp(MODE_BASE, 1) + psk_id_hash + info_hash
    secret = _labeled_extract(shared_secret, hpke_suite, b"secret", b"")
    key = _labeled_expand(secret, hpke_suite, b"key", context, 32)
    nonce = _labeled_expand(secret, hpke_suite, b"base_nonce", context, 12)

    plaintext = AESGCM(key).decrypt(nonce, ciphertext, b"")
    if len(plaintext) != 32:
        raise ValueError(f"解出的 seed 必须是 32 字节，实际 {len(plaintext)}")
    return plaintext


def keccak256(data: bytes) -> bytes:
    """keccak256（非 NIST SHA3）——用 OpenSSL 实现，与链上/EIP-191 一致。"""
    try:
        hasher = hashlib.new("keccak-256")
    except ValueError as exc:  # pragma: no cover - 取决于 OpenSSL 版本
        raise RuntimeError("当前解释器的 hashlib 不提供 keccak-256（需要 OpenSSL 1.1.1+）") from exc
    hasher.update(data)
    return hasher.digest()


def ed25519_sign(seed: bytes, data: bytes) -> bytes:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

    if len(seed) != 32:
        raise ValueError(f"seed 必须是 32 字节，实际 {len(seed)}")
    return Ed25519PrivateKey.from_private_bytes(seed).sign(data)


def sign_b64(seed: bytes, data: bytes) -> str:
    """对**裸字节**做 Ed25519 签名并 base64（对应 CLI 的 `ed25519_sign_encoded`）。"""
    return B64(ed25519_sign(seed, data)).decode()


def sign_hex_hash_b64(seed: bytes, hex_hash: str) -> str:
    """对 hex 表示的 hash 签名（对应 CLI 的 `ed25519_sign_hex`，用于 gen-msg-hash 后的 msgHash）。"""
    clean = hex_hash[2:] if hex_hash.startswith("0x") else hex_hash
    if not clean:
        return ""
    return sign_b64(seed, bytes.fromhex(clean))


def eip191_hash(message: str, encoding: str = "utf8") -> bytes:
    """EIP-191 personal_sign 哈希: keccak256("\\x19Ethereum Signed Message:\\n" + len + payload)。"""
    if not message:
        return b""
    if encoding == "hex":
        clean = message[2:] if message.startswith("0x") else message
        data = bytes.fromhex(clean)
    elif encoding == "utf8":
        data = message.encode("utf-8")
    else:
        raise ValueError(f'unsupported encoding for eip191: {encoding}（期望 "hex" 或 "utf8"）')
    prefix = f"\x19Ethereum Signed Message:\n{len(data)}".encode()
    return keccak256(prefix + data)


def sign_eip191_b64(seed: bytes, message: str, encoding: str = "utf8") -> str:
    """EIP-191 + Ed25519 签名（对应 CLI 的 `ed25519_sign_eip191`）。"""
    digest = eip191_hash(message, encoding)
    if not digest:
        return ""
    return sign_b64(seed, digest)


def is_hex_string(value: str) -> bool:
    """CLI 的 `is_hex_string` 等价物（用于决定 EIP-191 的编码方式）。"""
    if not value.startswith("0x") or len(value) <= 2:
        return False
    try:
        bytes.fromhex(value[2:] if len(value[2:]) % 2 == 0 else "0" + value[2:])
    except ValueError:
        return False
    return True
