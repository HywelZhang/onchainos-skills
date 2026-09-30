"""$ONCHAINOS_HOME 定位 —— 与官方 CLI 共享同一状态目录。

官方 CLI 的规则（cli/src/home.rs）: 优先 `ONCHAINOS_HOME` 环境变量，
否则 `<home>/.onchainos`。gateway 沿用同一目录，使会话、设备身份、
deliverables 与既有状态零迁移。
"""

from __future__ import annotations

import os
from pathlib import Path

ENV_HOME = "ONCHAINOS_HOME"


def home() -> Path:
    override = os.environ.get(ENV_HOME)
    if override and override.strip():
        return Path(override.strip())
    return Path.home() / ".onchainos"


def path(*parts: str) -> Path:
    return home().joinpath(*parts)


def ensure_home() -> Path:
    h = home()
    h.mkdir(parents=True, exist_ok=True)
    return h
