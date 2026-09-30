"""CLI 委托执行器（P0-11 v1.2 §12：签名/钱包/登录保持在官方 CLI）。

边界（用户 2026-09-30 决策）:
    * 私钥管理、钱包、登录、**签名/广播** → 官方 CLI 保持现状，不在改造范围；
    * gateway 只管 okx-ai **任务与订阅流程**：组参、守卫、幂等、规范化、文案、审计；
    * 需要签名的写操作 = gateway 组参 → CLI 执行 → gateway 规范化后对外暴露。

安全与工程约束:
    * 参数以 **argv 列表**传递（不经 shell），杜绝注入；
    * 只允许白名单内的子命令（`agent *` 与只读的 wallet/diagnostic），其余拒绝；
    * 统一超时；stdout 按 UTF-8 解码（Windows 下 CLI 输出含中文，必须显式指定）；
    * 每次调用写 gateway-audit.jsonl（source=cli），与直连 HTTP 的审计区分开。
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from pathlib import Path
from typing import Any

from . import home as _home

DEFAULT_TIMEOUT_S = 240

# 允许委托的子命令前缀（宽松到 "agent <name>" 一级，避免白名单维护成本过高，
# 但仍拒绝任何非 agent 域命令，确保委托面不越界到 wallet/defi/dex 等其它域）
ALLOWED_PREFIXES: tuple[str, ...] = ("agent ",)
ALLOWED_EXACT: tuple[str, ...] = ("wallet status", "wallet auth")


class DelegateError(RuntimeError):
    pass


def cli_path() -> str:
    """定位官方 CLI：环境变量 > PATH > 本机默认安装位置。"""
    override = os.environ.get("OKXAI_ONCHAINOS_CLI")
    if override and Path(override).exists():
        return override
    for name in ("onchainos.exe", "onchainos"):
        found = shutil.which(name)
        if found:
            return found
    fallback = Path.home() / ".local" / "bin" / "onchainos.exe"
    if fallback.exists():
        return str(fallback)
    raise DelegateError(
        "找不到 onchainos CLI（设置 OKXAI_ONCHAINOS_CLI 或安装：npx -y @okxweb3/onchainos-installer install）"
    )


def _is_allowed(argv: list[str]) -> bool:
    joined = " ".join(argv[:2])
    return joined.startswith(ALLOWED_PREFIXES) or joined in ALLOWED_EXACT


def _audit(argv: list[str], ok: bool, duration_ms: int, error: str | None) -> None:
    try:
        record = {
            "ts": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
            "source": "cli",
            "command": "delegate/" + " ".join(argv[:2]),
            "ok": ok,
            "duration_ms": duration_ms,
            "args": [" ".join(argv)],
            "error": error,
        }
        with open(_home.home() / "gateway-audit.jsonl", "a", encoding="utf-8") as fh:
            fh.write(json.dumps(record, ensure_ascii=False) + "\n")
    except Exception:  # noqa: BLE001 - 审计失败不得影响业务
        pass


def build_argv(command: list[str], flags: dict[str, Any] | None = None,
               positionals: list[str] | None = None) -> list[str]:
    """把业务参数拼成 CLI argv：None/False 跳过，True 变开关，列表重复出现。"""
    argv = list(command)
    for key, value in (flags or {}).items():
        if value is None or value is False or value == "":
            continue
        flag = f"--{key.replace('_', '-')}"
        if value is True:
            argv.append(flag)
        elif isinstance(value, (list, tuple)):
            for item in value:
                argv.extend([flag, str(item)])
        else:
            argv.extend([flag, str(value)])
    argv.extend(str(p) for p in (positionals or []))
    return argv


def run(command: list[str], *, flags: dict[str, Any] | None = None,
        positionals: list[str] | None = None, timeout: int = DEFAULT_TIMEOUT_S) -> dict[str, Any]:
    """执行一次委托调用，返回统一信封。

    返回: {"ok": bool, "data": Any|None, "rendered": str|None, "exit": int,
           "duration_ms": int, "argv": [...]}
    """
    argv = build_argv(command, flags, positionals)
    if not _is_allowed(argv):
        raise DelegateError(f"拒绝委托越权命令: {' '.join(argv[:2])!r}（只允许 {ALLOWED_PREFIXES}）")

    exe = cli_path()
    started = time.time()
    try:
        proc = subprocess.run([exe, *argv], capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        _audit(argv, False, int((time.time() - started) * 1000), f"timeout after {timeout}s")
        return {"ok": False, "error": f"CLI 超时（{timeout}s）", "argv": argv, "exit": -9,
                "phase": "timeout", "decision": "blocked"}
    duration_ms = int((time.time() - started) * 1000)

    stdout = proc.stdout.decode("utf-8", "replace").strip()
    stderr = proc.stderr.decode("utf-8", "replace").strip()
    _audit(argv, proc.returncode == 0, duration_ms, None if proc.returncode == 0 else stderr[:300])

    result: dict[str, Any] = {
        "ok": proc.returncode == 0,
        "exit": proc.returncode,
        "duration_ms": duration_ms,
        "argv": argv,
    }

    parsed: Any = None
    if stdout:
        try:
            parsed = json.loads(stdout)
        except json.JSONDecodeError:
            parsed = None

    if isinstance(parsed, dict) and "ok" in parsed:
        result["ok"] = bool(parsed.get("ok")) and proc.returncode == 0
        result["data"] = parsed.get("data")
        if not result["ok"]:
            result["error"] = str(parsed.get("error") or parsed.get("msg") or "CLI 返回失败")
    elif parsed is not None:
        result["data"] = parsed
    else:
        result["rendered"] = stdout  # 文本渲染型命令（CLI 直接给人类可读输出）

    if not result["ok"]:
        result.setdefault("error", stderr or stdout or f"CLI 退出码 {proc.returncode}")
        result.setdefault("detail", stderr[:600] if stderr else None)
    return result
