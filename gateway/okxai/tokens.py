"""XLayer 代币别名表（由 cli/src/token_alias.rs 生成，勿手改）。

用途: CLI 的 `sub.cost` 等展示型 verb 需要把代币地址翻成符号（golden 里显示 USDT）。
重新生成: python scripts/gen-token-alias.py
"""

from __future__ import annotations

ALIASES: dict[str, dict[str, str]] = {
    "1": {
        "eth": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "usdt": "0xdac17f958d2ee523a2206206994597c13d831ec7",
        "wbtc": "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599",
        "dai": "0x6b175474e89094c44da98b954eedeac495271d0f",
        "weth": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    },
    "10": {
        "eth": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0x0b2c639c533813f4aa9d7837caf62653d097ff85",
        "usdt": "0x94b008aa00579c1307b0ef2c499ad98a8ce58e58",
        "weth": "0x4200000000000000000000000000000000000006",
        "op": "0x4200000000000000000000000000000000000042",
    },
    "56": {
        "bnb": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdt": "0x55d398326f99059ff775485246999027b3197955",
        "usdc": "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d",
        "wbnb": "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c",
        "weth": "0x2170ed0880ac9a755fd29b2688956bd959f933f8",
        "btcb": "0x7130d2a12b9bcbfae4f2634d864a1ee1ce3ead9c",
    },
    "137": {
        "matic": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "pol": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
        "usdt0": "0xc2132d05d31c914a87c6611c10748aeb04b58e8f",
        "weth": "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619",
        "wmatic": "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270",
        "wpol": "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270",
    },
    "195": {
        "trx": "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
        "native": "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
        "usdt": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
        "wtrx": "TNUC9Qb1rRpS5CbWLmNMxXBjyFoydXjWFR",
        "eth": "THb4CqiFdwNHsWsQCs4JhzwjMWys4aqCbF",
    },
    "196": {
        "okb": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0x74b7f16337b8972027f6196a17a631ac6de26d22",
        "xlayer_usdt": "0x1e4a5963abfd975d8c9021ce480b42188849d41d",
        "usdt0": "0x779ded0c9e1022225f8e0630b35a9b54be713736",
        "usdt": "0x779ded0c9e1022225f8e0630b35a9b54be713736",
        "weth": "0x5a77f1443d16ee5761d310e38b62f77f726bc71c",
        "wokb": "0xe538905cf8410324e03a5a23c1c177a474d59b2b",
    },
    "250": {
        "ftm": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "wftm": "0x21be370d5312f44cb42ce377bc9b8a0cef1a4c83",
    },
    "324": {
        "eth": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "weth": "0x5aea5775959fbc2557cc8789bc1bf90a239d9a91",
        "usdt": "0x493257fd37edb34451f62edf8d2a0c418852ba4c",
    },
    "501": {
        "sol": "11111111111111111111111111111111",
        "native": "11111111111111111111111111111111",
        "usdc": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "usdt": "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
        "so11111111111111111111111111111111111111112": "11111111111111111111111111111111",
        "so11111111111111111111111111111111111111111": "11111111111111111111111111111111",
    },
    "784": {
        "sui": "0x2::sui::SUI",
        "native": "0x2::sui::SUI",
        "wusdc": "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN",
        "wusdt": "0xc060006111016b8a020ad5b33834984a437aaa7d3c74c18e09a95d48aceab08c::coin::COIN",
    },
    "1952": {
        "okb": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0xcb8bf24c6ce16ad21d707c9505421a17f2bec79d",
        "usdt": "0x9e29b3aada05bf2d2c827af80bd28dc0b9b4fb0c",
        "usdg": "0xa78e2baabaf5c4f36b7fc394725deb68d332eec1",
    },
    "8453": {
        "eth": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
        "weth": "0x4200000000000000000000000000000000000006",
        "usdbc": "0xd9aaec86b65d86f6a7b5b1b0c42ffa531710b6ca",
    },
    "42161": {
        "eth": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
        "usdt": "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9",
        "weth": "0x82af49447d8a07e3bd95bd0d56f35241523fbab1",
    },
    "43114": {
        "avax": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0xb97ef9ef8734c71904d8002f8b6bc66dd9c48a6e",
        "usdt": "0x9702230a8ea53601f5cd2dc00fdbc13d4df4a8c7",
        "wavax": "0xb31f66aa3c1e785363f0875a1b74e27b85fd66c7",
        "weth.e": "0x49d5c2bdffac6ce2bfdb6640f4f80f226bc10bab",
    },
    "59144": {
        "eth": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0x176211869ca2b568f2a7d4ee941e073a821ee1ff",
        "usdt": "0xa219439258ca9da29e9cc4ce5596924745e12b93",
        "weth": "0xe5d7c2a44ffddf6b295a15c148167daaaf5cf34f",
    },
    "534352": {
        "eth": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "native": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "usdc": "0x06efdbff2a14a7c8e15944d1f4a48f9f95f663a4",
        "usdt": "0xf55bec9cafdbe8730f096aa55dad6d22d44099df",
        "weth": "0x5300000000000000000000000000000000000004",
    },
}

_REVERSE: dict[str, dict[str, str]] = {
    "1": {
        "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599": "wbtc",
        "0x6b175474e89094c44da98b954eedeac495271d0f": "dai",
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "usdc",
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "weth",
        "0xdac17f958d2ee523a2206206994597c13d831ec7": "usdt",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "eth",
    },
    "10": {
        "0x0b2c639c533813f4aa9d7837caf62653d097ff85": "usdc",
        "0x4200000000000000000000000000000000000006": "weth",
        "0x4200000000000000000000000000000000000042": "op",
        "0x94b008aa00579c1307b0ef2c499ad98a8ce58e58": "usdt",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "eth",
    },
    "56": {
        "0x2170ed0880ac9a755fd29b2688956bd959f933f8": "weth",
        "0x55d398326f99059ff775485246999027b3197955": "usdt",
        "0x7130d2a12b9bcbfae4f2634d864a1ee1ce3ead9c": "btcb",
        "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d": "usdc",
        "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c": "wbnb",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "bnb",
    },
    "137": {
        "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270": "wpol",
        "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359": "usdc",
        "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619": "weth",
        "0xc2132d05d31c914a87c6611c10748aeb04b58e8f": "usdt0",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "pol",
    },
    "195": {
        "t9yd14nj9j7xab4dbgeix9h8unkkhxuwwb": "trx",
        "thb4cqifdwnhswsqcs4jhzwjmwys4aqcbf": "eth",
        "tnuc9qb1rrps5cbwlmnmxxbjyfoydxjwfr": "wtrx",
        "tr7nhqjekqxgtci8q8zy4pl8otszgjlj6t": "usdt",
    },
    "196": {
        "0x1e4a5963abfd975d8c9021ce480b42188849d41d": "xlayer_usdt",
        "0x5a77f1443d16ee5761d310e38b62f77f726bc71c": "weth",
        "0x74b7f16337b8972027f6196a17a631ac6de26d22": "usdc",
        "0x779ded0c9e1022225f8e0630b35a9b54be713736": "usdt",
        "0xe538905cf8410324e03a5a23c1c177a474d59b2b": "wokb",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "okb",
    },
    "250": {
        "0x21be370d5312f44cb42ce377bc9b8a0cef1a4c83": "wftm",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "ftm",
    },
    "324": {
        "0x493257fd37edb34451f62edf8d2a0c418852ba4c": "usdt",
        "0x5aea5775959fbc2557cc8789bc1bf90a239d9a91": "weth",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "eth",
    },
    "501": {
        "11111111111111111111111111111111": "sol",
        "epjfwdd5aufqssqem2qn1xzybapc8g4weggkzwytdt1v": "usdc",
        "es9vmfrzacermjfrf4h2fyd4kconky11mcce8benwnyb": "usdt",
    },
    "784": {
        "0x2::sui::sui": "sui",
        "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::coin": "wusdc",
        "0xc060006111016b8a020ad5b33834984a437aaa7d3c74c18e09a95d48aceab08c::coin::coin": "wusdt",
    },
    "1952": {
        "0x9e29b3aada05bf2d2c827af80bd28dc0b9b4fb0c": "usdt",
        "0xa78e2baabaf5c4f36b7fc394725deb68d332eec1": "usdg",
        "0xcb8bf24c6ce16ad21d707c9505421a17f2bec79d": "usdc",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "okb",
    },
    "8453": {
        "0x4200000000000000000000000000000000000006": "weth",
        "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913": "usdc",
        "0xd9aaec86b65d86f6a7b5b1b0c42ffa531710b6ca": "usdbc",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "eth",
    },
    "42161": {
        "0x82af49447d8a07e3bd95bd0d56f35241523fbab1": "weth",
        "0xaf88d065e77c8cc2239327c5edb3a432268e5831": "usdc",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "eth",
        "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9": "usdt",
    },
    "43114": {
        "0x49d5c2bdffac6ce2bfdb6640f4f80f226bc10bab": "weth.e",
        "0x9702230a8ea53601f5cd2dc00fdbc13d4df4a8c7": "usdt",
        "0xb31f66aa3c1e785363f0875a1b74e27b85fd66c7": "wavax",
        "0xb97ef9ef8734c71904d8002f8b6bc66dd9c48a6e": "usdc",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "avax",
    },
    "59144": {
        "0x176211869ca2b568f2a7d4ee941e073a821ee1ff": "usdc",
        "0xa219439258ca9da29e9cc4ce5596924745e12b93": "usdt",
        "0xe5d7c2a44ffddf6b295a15c148167daaaf5cf34f": "weth",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "eth",
    },
    "534352": {
        "0x06efdbff2a14a7c8e15944d1f4a48f9f95f663a4": "usdc",
        "0x5300000000000000000000000000000000000004": "weth",
        "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "eth",
        "0xf55bec9cafdbe8730f096aa55dad6d22d44099df": "usdt",
    },
}


def resolve_address(chain_index: str, token: str) -> str:
    """别名 → 地址（大小写不敏感）；未知输入原样返回。"""
    table = ALIASES.get(str(chain_index), {})
    return table.get(token.strip().lower(), token)


def symbol_for(chain_index: str, address: str) -> str:
    """地址 → 展示符号（大写）；未知返回空串。"""
    table = _REVERSE.get(str(chain_index), {})
    symbol = table.get((address or "").strip().lower(), "")
    if not symbol:
        return ""
    return "OKB" if symbol == "okb" else symbol.upper()
