"""形态 A 入口：`python -m okxai <verb> [--arg=value ...]`

不需要任何全局安装；只要解释器能 import 本包即可（agent 侧零二进制）。

用法:
    python -m okxai list
    python -m okxai session.status
    python -m okxai agent.mine --role=user
    python -m okxai task.status --job_id=0x...
    python -m okxai serve --port=8788        # 转到形态 B（本地 HTTP 服务）

输出: 统一信封 JSON（`{"ok":true,"data":...}` / `{"ok":false,"error":...}`），
      与 skill 的渲染契约一致：agent 只读 data，不解析 HTTP 细节。
"""

from __future__ import annotations

import argparse
import json
import sys
from typing import Any

from . import verbs
from . import verbs_write  # noqa: F401 - 注册 P1 写 verb
from .session import ApiError, NotLoggedIn
from .transport import HttpError, TransportError


def _coerce(value: str) -> Any:
    if value.lower() in ("true", "false"):
        return value.lower() == "true"
    if value.isdigit():
        return int(value)
    return value


def _parse_kwargs(items: list[str]) -> dict[str, Any]:
    kwargs: dict[str, Any] = {}
    for item in items:
        if item.startswith("--"):
            body = item[2:]
            if "=" in body:
                key, _, raw = body.partition("=")
                kwargs[key.replace("-", "_")] = _coerce(raw)
            else:
                kwargs[body.replace("-", "_")] = True
    return kwargs


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)

    if argv and argv[0] == "serve":
        from .server import main as serve_main

        return serve_main(argv[1:])

    if not argv or argv[0] in ("-h", "--help", "list"):
        print(json.dumps({"ok": True, "data": {"verbs": verbs.listing()}}, ensure_ascii=False, indent=2))
        return 0

    name = argv[0]
    kwargs = _parse_kwargs(argv[1:])

    try:
        data = verbs.run(name, **kwargs)
        print(json.dumps({"ok": True, "verb": name, "data": data}, ensure_ascii=False, indent=2))
        return 0
    except NotLoggedIn as exc:
        print(json.dumps({
            "ok": False, "verb": name, "error": str(exc),
            "nextAction": [{"id": "login", "actionLabel": "重新登录 OKX 账号"}],
            "decision": "blocked", "phase": "session",
        }, ensure_ascii=False, indent=2))
        return 3
    except ApiError as exc:
        print(json.dumps({"ok": False, "verb": name, "error": str(exc), "code": exc.code,
                          "phase": "api", "decision": "blocked"}, ensure_ascii=False, indent=2))
        return 4
    except (TransportError, HttpError, verbs.VerbError) as exc:
        print(json.dumps({"ok": False, "verb": name, "error": f"{type(exc).__name__}: {exc}",
                          "phase": "request", "decision": "blocked"}, ensure_ascii=False, indent=2))
        return 5
    except Exception as exc:  # noqa: BLE001 - 顶层兜底，保证 agent 永远拿到结构化结果
        print(json.dumps({"ok": False, "verb": name, "error": f"{type(exc).__name__}: {exc}",
                          "phase": "internal", "decision": "blocked"}, ensure_ascii=False, indent=2))
        return 6


if __name__ == "__main__":
    raise SystemExit(main())
