"""okxai gateway — 本地转换层（P0-11 v1.1）。

把 okx-ai 域所需的会话/密码学/HTTP 语义从官方 CLI 迁到可移植运行时，
使 agent 侧不再需要任何二进制：

    形态 A:  python -m okxai <verb> [--k=v ...]        （库脚本，无全局安装）
    形态 B:  python -m okxai serve [--port 8788]        （本地 HTTP，宿主侧守护）

设计依据: docs/design/11-cli-to-api-refactor.md
模块划分:
    home       — $ONCHAINOS_HOME 定位（与官方 CLI 共享状态目录）
    keystore   — session.json + keyring.enc（scrypt + AES-256-GCM）读写
    transport  — HTTPS 传输（DoH 节点 IP 固定 + Host 头 / 直连回退）
    session    — JWT 生命周期、refresh、agenticId 解析、device-id
    crypto     — HPKE 解 seed / Ed25519 / keccak256+EIP-191（P1 用）
    verbs      — 功能级 verb（读 P0 / 写 P1 / 消息 P2）
    envelope   — 统一信封（phase/decision/reason/nextAction/payload）+ 文案
    cli        — 形态 A 入口
    server     — 形态 B 入口
"""

__version__ = "0.1.0"
