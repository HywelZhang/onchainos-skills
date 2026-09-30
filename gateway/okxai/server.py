"""形态 B 入口：本地 HTTP 服务（宿主侧守护，agent 只需 fetch）。

    python -m okxai serve --port 8788

路由:
    GET  /v1/verbs                      → verb 清单
    GET  /v1/<verb>?k=v&...             → 执行 verb（读）
    POST /v1/<verb>  {"kwargs": {...}}  → 执行 verb（写/复杂参数）
    GET  /healthz                       → 存活探测（不触网）

只监听 127.0.0.1；不做鉴权（本机进程边界即信任边界），并在响应头声明这一点。
用 stdlib http.server 实现，无第三方依赖。
"""

from __future__ import annotations

import argparse
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import parse_qs, urlparse

from . import verbs
from . import verbs_write  # noqa: F401 - 注册 P1 写 verb
from .session import ApiError, NotLoggedIn, Session
from .transport import HttpError, TransportError

_session_lock = threading.Lock()
_session: Session | None = None
_started_at = time.time()
_stats = {"requests": 0, "errors": 0}


def _get_session() -> Session:
    """进程内单例：身份解析只做一次（CLI 是每次调用都重查）。"""
    global _session
    with _session_lock:
        if _session is None:
            _session = Session.load()
        return _session


def _coerce(value: str) -> Any:
    if value.lower() in ("true", "false"):
        return value.lower() == "true"
    if value.isdigit():
        return int(value)
    return value


class Handler(BaseHTTPRequestHandler):
    server_version = "okxai-gateway/0.1"
    protocol_version = "HTTP/1.1"

    # ---- 基础 ----
    def log_message(self, fmt: str, *args: Any) -> None:  # noqa: A003 - 静音默认 stderr 日志
        return

    def _send(self, status: int, payload: Any) -> None:
        body = json.dumps(payload, ensure_ascii=False, indent=2).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("X-Okxai-Local-Only", "no-auth, loopback trust boundary")
        self.end_headers()
        self.wfile.write(body)

    # ---- 路由 ----
    def do_GET(self) -> None:  # noqa: N802 - stdlib 约定
        parsed = urlparse(self.path)
        path = parsed.path.rstrip("/")
        if path in ("/healthz", "/v1/healthz"):
            self._send(200, {"ok": True, "uptime_s": int(time.time() - _started_at), "stats": _stats})
            return
        if path in ("/v1/verbs", "/verbs"):
            self._send(200, {"ok": True, "data": {"verbs": verbs.listing()}})
            return
        if path.startswith("/v1/"):
            name = path[4:]
            kwargs = {k: _coerce(v[0]) for k, v in parse_qs(parsed.query).items() if v}
            self._dispatch(name, kwargs)
            return
        self._send(404, {"ok": False, "error": f"unknown path {parsed.path}", "hint": "GET /v1/verbs"})

    def do_POST(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b"{}"
        try:
            body = json.loads(raw.decode("utf-8") or "{}")
        except json.JSONDecodeError as exc:
            self._send(400, {"ok": False, "error": f"invalid JSON body: {exc}"})
            return
        kwargs = body.get("kwargs") if isinstance(body, dict) else None
        if kwargs is None:
            kwargs = {k: v for k, v in (body or {}).items() if k != "verb"}
        if parsed.path.rstrip("/").startswith("/v1/"):
            self._dispatch(parsed.path.rstrip("/")[4:], kwargs or {})
            return
        self._send(404, {"ok": False, "error": f"unknown path {parsed.path}"})

    # ---- 执行 ----
    def _dispatch(self, name: str, kwargs: dict[str, Any]) -> None:
        if name not in verbs.REGISTRY:
            self._send(404, {"ok": False, "error": f"unknown verb {name!r}",
                             "available": sorted(verbs.REGISTRY)})
            return
        _stats["requests"] += 1
        try:
            data = verbs.run(name, session=_get_session(), **kwargs)
            self._send(200, {"ok": True, "verb": name, "data": data, "phase": "done", "decision": "ready"})
        except NotLoggedIn as exc:
            _stats["errors"] += 1
            self._send(401, {"ok": False, "verb": name, "error": str(exc), "phase": "session",
                             "decision": "blocked",
                             "nextAction": [{"id": "login", "actionLabel": "重新登录 OKX 账号"}]})
        except ApiError as exc:
            _stats["errors"] += 1
            self._send(200, {"ok": False, "verb": name, "error": str(exc), "code": exc.code,
                             "phase": "api", "decision": "blocked"})
        except (TransportError, HttpError, verbs.VerbError) as exc:
            _stats["errors"] += 1
            self._send(200, {"ok": False, "verb": name, "error": f"{type(exc).__name__}: {exc}",
                             "phase": "request", "decision": "blocked"})
        except Exception as exc:  # noqa: BLE001
            _stats["errors"] += 1
            self._send(500, {"ok": False, "verb": name, "error": f"{type(exc).__name__}: {exc}",
                             "phase": "internal", "decision": "blocked"})


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="okxai serve")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8788)
    args = ap.parse_args(argv)

    httpd = ThreadingHTTPServer((args.host, args.port), Handler)
    print(json.dumps({
        "ok": True,
        "listening": f"http://{args.host}:{args.port}",
        "routes": ["GET /healthz", "GET /v1/verbs", "GET /v1/<verb>?arg=value", "POST /v1/<verb>"],
        "note": "loopback only; no auth — 进程边界即信任边界",
    }, ensure_ascii=False))
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("bye")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
