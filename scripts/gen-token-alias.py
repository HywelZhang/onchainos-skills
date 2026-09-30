#!/usr/bin/env python3
"""从 cli/src/token_alias.rs 生成 gateway/okxai/tokens.py（别名表 + 反向符号表）。

用法: python scripts/gen-token-alias.py [--src cli/src/token_alias.rs] [--out gateway/okxai/tokens.py]
"""

from __future__ import annotations

import argparse
import pathlib
import re

HEADER = '''"""XLayer 代币别名表（由 cli/src/token_alias.rs 生成，勿手改）。

用途: CLI 的 `sub.cost` 等展示型 verb 需要把代币地址翻成符号（golden 里显示 USDT）。
重新生成: python scripts/gen-token-alias.py
"""

from __future__ import annotations
'''


def parse(src: str) -> dict[str, list[tuple[str, str]]]:
    chains: dict[str, list[tuple[str, str]]] = {}
    for m in re.finditer(r'\("(\d+)",\s*HashMap::from\(\[(.*?)\]\)\)', src, re.S):
        pairs = re.findall(r'\("([^"]+)",\s*"([^"]+)"\)', m.group(2))
        if pairs:
            chains[m.group(1)] = pairs
    return chains


def build(chains: dict[str, list[tuple[str, str]]]) -> str:
    rev: dict[str, dict[str, str]] = {}
    for chain, pairs in chains.items():
        for alias, addr in pairs:
            table = rev.setdefault(chain, {})
            key = addr.lower()
            if key in table:
                old = table[key]
                # 多别名指向同一地址时，偏好纯字母且更短的（CLI 显示 USDT 而非 USDT0）
                if (alias.isalpha() and not old.isalpha()) or len(alias) < len(old):
                    table[key] = alias
            else:
                table[key] = alias

    out = [HEADER, "ALIASES: dict[str, dict[str, str]] = {"]
    for chain in sorted(chains, key=int):
        out.append(f'    "{chain}": {{')
        out.extend(f'        "{a}": "{d}",' for a, d in chains[chain])
        out.append("    },")
    out.append("}")
    out.append("")
    out.append("_REVERSE: dict[str, dict[str, str]] = {")
    for chain in sorted(rev, key=int):
        out.append(f'    "{chain}": {{')
        out.extend(f'        "{a}": "{s}",' for a, s in sorted(rev[chain].items()))
        out.append("    },")
    out.append("}")
    out.extend([
        "",
        "",
        "def resolve_address(chain_index: str, token: str) -> str:",
        '    """别名 → 地址（大小写不敏感）；未知输入原样返回。"""',
        "    table = ALIASES.get(str(chain_index), {})",
        "    return table.get(token.strip().lower(), token)",
        "",
        "",
        "def symbol_for(chain_index: str, address: str) -> str:",
        '    """地址 → 展示符号（大写）；未知返回空串。"""',
        "    table = _REVERSE.get(str(chain_index), {})",
        '    symbol = table.get((address or "").strip().lower(), "")',
        "    if not symbol:",
        '        return ""',
        '    return "OKB" if symbol == "okb" else symbol.upper()',
        "",
    ])
    return "\n".join(out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", default="cli/src/token_alias.rs")
    ap.add_argument("--out", default="gateway/okxai/tokens.py")
    args = ap.parse_args()

    src_path = pathlib.Path(args.src)
    if not src_path.exists():
        print(f"source not found: {src_path}")
        return 2
    chains = parse(src_path.read_text(encoding="utf-8"))
    text = build(chains)
    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8")
    print(f"generated {out} — {len(chains)} chains, {sum(len(v) for v in chains.values())} aliases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
