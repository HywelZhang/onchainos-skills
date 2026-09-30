"""HTTPS 传输层 —— 复现官方 CLI 的 DoH 节点策略（CN 环境直连必备）。

背景（实测 2026-09-30）:
    `web3.okx.com` 的 DNS 在本机被污染到 169.254.0.2（`awscn.okpool.top`），
    直连必然失败。官方 CLI 用 DoH 选出一个**代理节点**，把 IP 固定住：
        SNI  = 节点的 host（例如 web3.ynhf1jp.com）
        Host = web3.okx.com
        TCP  = 节点 IP:443
    curl 实测该组合返回 HTTP 200（`--resolve web3.ynhf1jp.com:443:<ip>` + `Host: web3.okx.com`）。

本模块实现同一策略：
    * 读 `$ONCHAINOS_HOME/doh-cache.json`（CLI 已经写好的节点缓存，含 ip/host/mode）
    * 节点模式 → IP 固定 + SNI=node.host + Host=真实域名
    * 直连模式或无缓存 → 普通 DNS
    * 可选走 HTTP(S) 代理（CONNECT 隧道），用于有 Clash 等代理的机器

已知缺口（记录在 docs/design/11 §9）: 节点失效后的**再发现**尚未实现
（官方走 DoH/pilot 二进制）。当前策略：缓存节点失败 → 回退直连 → 报错并提示
`DoH 节点失效，需重新发现`。P0 验收不依赖再发现（缓存节点可用）。
"""

from __future__ import annotations

import http.client
import json
import os
import socket
import ssl
import time
from dataclasses import dataclass
from typing import Any
from urllib.parse import urlencode

from . import home as _home

TARGET_HOST = "web3.okx.com"
HTTPS_PORT = 443
DEFAULT_TIMEOUT = 40.0


class TransportError(RuntimeError):
    """网络层错误（连接/超时/TLS）。"""


class HttpError(RuntimeError):
    """服务端返回非 2xx。"""

    def __init__(self, status: int, body: str, path: str):
        self.status = status
        self.body = body
        self.path = path
        super().__init__(f"HTTP {status} on {path}: {body[:300]}")


@dataclass(frozen=True)
class DohNode:
    mode: str  # "proxy" | "direct" | "none"
    ip: str = ""
    host: str = ""

    @property
    def is_proxy(self) -> bool:
        return self.mode == "proxy" and bool(self.ip)


def load_doh_node(target: str = TARGET_HOST) -> DohNode:
    path = _home.path("doh-cache.json")
    if not path.exists():
        return DohNode(mode="none")
    try:
        cache = json.loads(path.read_text(encoding="utf-8"))
    except Exception:  # noqa: BLE001 - 缓存坏了不该让整个流程不可用
        return DohNode(mode="none")
    entry = cache.get(target) or {}
    node = entry.get("node") or {}
    mode = entry.get("mode") or "none"
    if mode == "proxy" and node.get("ip"):
        return DohNode(mode="proxy", ip=str(node.get("ip")), host=str(node.get("host") or target))
    return DohNode(mode=mode if mode in ("direct", "proxy") else "none")


def _proxy_from_env() -> tuple[str, int] | None:
    raw = (
        os.environ.get("OKXAI_HTTP_PROXY")
        or os.environ.get("https_proxy")
        or os.environ.get("HTTPS_PROXY")
        or os.environ.get("http_proxy")
        or os.environ.get("HTTP_PROXY")
    )
    if not raw:
        return None
    raw = raw.strip()
    for scheme in ("http://", "https://"):
        if raw.startswith(scheme):
            raw = raw[len(scheme) :]
    raw = raw.rstrip("/")
    if ":" in raw:
        host, _, port = raw.rpartition(":")
        return host, int(port)
    return raw, 8080


class _PinnedHTTPSConnection(http.client.HTTPSConnection):
    """把 TCP 目标固定为节点 IP / 代理，但 TLS SNI 与服务端证书校验仍针对节点 host。"""

    def __init__(self, node: DohNode, proxy: tuple[str, int] | None, timeout: float):
        super().__init__(TARGET_HOST, HTTPS_PORT, timeout=timeout, context=ssl.create_default_context())
        self._node = node
        self._proxy = proxy

    def connect(self) -> None:  # noqa: D102 - 复刻 stdlib 行为
        connect_host = self._node.ip if self._node.is_proxy else TARGET_HOST
        connect_port = HTTPS_PORT

        if self._proxy:
            self.sock = socket.create_connection(self._proxy, self.timeout)
            tunnel_host = self._node.host if self._node.is_proxy else TARGET_HOST
            self.sock.sendall(
                f"CONNECT {tunnel_host}:{HTTPS_PORT} HTTP/1.1\r\n"
                f"Host: {tunnel_host}:{HTTPS_PORT}\r\n"
                f"Proxy-Connection: keep-alive\r\n\r\n".encode()
            )
            buf = b""
            while b"\r\n\r\n" not in buf:
                chunk = self.sock.recv(4096)
                if not chunk:
                    raise TransportError("proxy closed the tunnel during CONNECT")
                buf += chunk
            first_line = buf.split(b"\r\n", 1)[0]
            if b" 200" not in first_line:
                raise TransportError(f"proxy refused CONNECT: {first_line!r}")
        else:
            self.sock = socket.create_connection((connect_host, connect_port), self.timeout)

        # SNI / 证书校验针对节点 host（实测：节点按 SNI 路由）
        sni = self._node.host if self._node.is_proxy else TARGET_HOST
        self.sock = self._context.wrap_socket(self.sock, server_hostname=sni)


class Client:
    """最小 HTTP 客户端：JSON in / JSON out，Host 头固定为 web3.okx.com。"""

    def __init__(self, timeout: float = DEFAULT_TIMEOUT, log_audit: bool = True):
        self.timeout = timeout
        self.log_audit = log_audit
        self.node = load_doh_node()
        self.proxy = _proxy_from_env()

    # ---- 描述 ----
    def describe(self) -> dict[str, Any]:
        return {
            "target": TARGET_HOST,
            "doh_mode": self.node.mode,
            "doh_node_ip": self.node.ip,
            "doh_node_host": self.node.host,
            "proxy": f"{self.proxy[0]}:{self.proxy[1]}" if self.proxy else None,
        }

    # ---- 请求 ----
    def request(
        self,
        method: str,
        path: str,
        *,
        query: dict[str, str] | None = None,
        body: dict[str, Any] | None = None,
        headers: dict[str, str] | None = None,
        expect_json: bool = True,
    ) -> Any:
        if query:
            pairs: list[tuple[str, str]] = []
            for k, v in query.items():
                if isinstance(v, (list, tuple)):
                    pairs.extend((k, str(item)) for item in v)
                else:
                    pairs.append((k, str(v)))
            path = f"{path}?{urlencode(pairs)}"
        payload = json.dumps(body, ensure_ascii=False).encode("utf-8") if body is not None else None

        send_headers = {
            "Host": TARGET_HOST,
            "Accept": "application/json, text/plain, */*",
            "User-Agent": "okxai-gateway/0.1",
            "Connection": "close",
        }
        if payload is not None:
            send_headers["Content-Type"] = "application/json"
            send_headers["Content-Length"] = str(len(payload))
        if headers:
            send_headers.update(headers)

        started = time.time()
        conn = _PinnedHTTPSConnection(self.node, self.proxy, self.timeout)
        try:
            conn.request(method, path, body=payload, headers=send_headers)
            resp = conn.getresponse()
            raw = resp.read()
            status = resp.status
        except (OSError, ssl.SSLError, http.client.HTTPException) as exc:
            self._audit(method, path, False, started, f"{type(exc).__name__}: {exc}")
            hint = ""
            if self.node.is_proxy:
                hint = (
                    f"（DoH 节点 {self.node.host}/{self.node.ip} 可能已失效；"
                    "删除 $ONCHAINOS_HOME/doh-cache.json 后由官方 CLI 刷新，或设置 OKXAI_HTTP_PROXY）"
                )
            raise TransportError(f"{type(exc).__name__}: {exc} {hint}") from exc
        finally:
            try:
                conn.close()
            except Exception:  # noqa: BLE001
                pass

        text = raw.decode("utf-8", "replace")
        if not 200 <= status < 300:
            self._audit(method, path, False, started, f"HTTP {status}")
            raise HttpError(status, text, path)
        self._audit(method, path, True, started, None)

        if not expect_json:
            return raw
        if not text.strip():
            return {}
        try:
            return json.loads(text)
        except json.JSONDecodeError as exc:
            raise TransportError(f"响应不是合法 JSON: {text[:200]!r}") from exc

    # ---- 审计（与 CLI 的 audit.jsonl 分开，避免互相污染）----
    def _audit(self, method: str, path: str, ok: bool, started: float, error: str | None) -> None:
        if not self.log_audit:
            return
        try:
            record = {
                "ts": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
                "source": "gateway",
                "command": f"http/{method.lower()}",
                "ok": ok,
                "duration_ms": int((time.time() - started) * 1000),
                "path": path,
                "error": error,
            }
            with open(_home.home() / "gateway-audit.jsonl", "a", encoding="utf-8") as fh:
                fh.write(json.dumps(record, ensure_ascii=False) + "\n")
        except Exception:  # noqa: BLE001 - 审计失败不得影响业务
            pass
