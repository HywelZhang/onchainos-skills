#!/usr/bin/env python3
"""P1 前置验证：签名链在**真实服务端**上跑通（不广播、不花钱）。

验证三件事：
    1. HPKE 解出 Ed25519 seed（库实现与手写实现互验一致）
    2. keccak256 对已知向量正确（EIP-191 依赖）
    3. `pre-transaction/sign-msg` 接受我们的 `personalSign` 会话签名
       —— 只要服务端回签名，就说明"设备侧签名"这条链在我们手里完全可用

用法: python tests/probe_signing.py [--message "custom probe"]
退出码: 0 全通; 1 有失败。
"""

from __future__ import annotations

import argparse
import base64
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from okxai import crypto, keystore, session  # noqa: E402
from okxai.session import ApiError  # noqa: E402

KECCAK_VECTORS = {
    b"": "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
    b"abc": "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45",
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--message", default="okxai gateway P1 probe")
    args = ap.parse_args()

    failures: list[str] = []

    # 1) keccak256 向量
    for payload, expected in KECCAK_VECTORS.items():
        got = crypto.keccak256(payload).hex()
        ok = got == expected
        print(f"[{'ok ' if ok else 'ERR'}] keccak256({payload!r}) == {expected[:16]}…")
        if not ok:
            failures.append(f"keccak256({payload!r}) 不符: {got}")

    # 2) HPKE 两条路径一致
    sess = keystore.read_session()
    blob = keystore.read_blob()
    if "session_key" not in blob or not sess.get("encryptedSessionSk"):
        print("[ERR] keyring 缺少 session_key 或 session.json 缺少 encryptedSessionSk（未登录？）")
        return 1
    ct = base64.b64decode(sess["encryptedSessionSk"])
    sk = base64.b64decode(blob["session_key"])
    try:
        seed_lib = crypto._hpke_decrypt_seed_lib(ct, sk)
        seed_manual = crypto._hpke_decrypt_seed_manual(ct, sk)
        same = seed_lib == seed_manual
        print(f"[{'ok ' if same else 'ERR'}] HPKE 库/手写一致 (seed 32B: {same})")
        if not same:
            failures.append("HPKE 两条实现结果不一致")
        seed = seed_lib
    except Exception as exc:  # noqa: BLE001
        print(f"[ERR] HPKE 解密 seed 失败: {type(exc).__name__}: {exc}")
        return 1

    # 3) 真实服务端签名验收
    s = session.Session.load()
    try:
        aid = s.agent_id("user")
        row = [r for r in s.agents() if str(r.get("agentId")) == aid][0]
        address = row["agentWalletAddress"]
    except (ApiError, IndexError, KeyError) as exc:
        print(f"[ERR] 无法解析 agent/钱包地址: {type(exc).__name__}: {exc}")
        return 1

    signature = crypto.sign_eip191_b64(seed, args.message, "utf8")
    body = {
        "chainIndex": "196",
        "from": address,
        "sessionCert": s.session_cert,
        "payload": [
            {"signType": "personalSign", "message": {"value": args.message}, "sessionSignature": signature}
        ],
    }
    try:
        data = s.post("/priapi/v5/wallet/agentic/pre-transaction/sign-msg", body=body)
    except Exception as exc:  # noqa: BLE001
        print(f"[ERR] sign-msg 被拒: {type(exc).__name__}: {exc}")
        return 1

    item = data[0] if isinstance(data, list) and data else data
    got_sig = item.get("signature") if isinstance(item, dict) else None
    if not got_sig:
        print(f"[ERR] sign-msg 响应缺少 signature: {json.dumps(item, ensure_ascii=False)[:200]}")
        failures.append("sign-msg 未回签名")
    else:
        print(f"[ok ] 服务端接受会话签名 → assistantSignature {str(got_sig)[:26]}… (agent {aid})")

    print("\nPASS" if not failures else f"\nFAIL: {failures}")
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
