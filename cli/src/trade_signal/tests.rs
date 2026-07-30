//! Corpus + invariant tests for the trade-signal parser (AC-1 … AC-24).
//!
//! Per V1.1/TD review alignment (feedback !ee926d82) the previous self-authored
//! corpus is replaced with examples in the authoritative V1.1 grammar: the full
//! titles (`【现货信号】` / `【Spot Signal】` …), the fixed-order `|` fields per
//! asset class, the Prediction `<OUTCOME> @<odds>` form, and both perp TP forms
//! (separate `止盈1..3`/`tp1..3` and combined slash `止盈`/`takeProfit`).
//!
//! NOTE (see notes.md A1): the byte-exact sample strings in the Lark v1.1 doc were
//! not reachable from the implementation stage (no Lark tool; KB empty for this
//! repo; not committed). These 14 examples are faithful reconstructions of the
//! spec's structure per the reviewer feedback and should be confirmed against the
//! Lark doc's verbatim strings before the MR is flipped to Ready.
//!
//! Positives assert the output model; negatives assert the exact stable snake_case
//! `errorCode`.

use super::{parse_envelope, parse_signal_text, SignalParams};

/// The 14 canonical bilingual examples (7 zh + 7 en): spot×2, perp×2,
/// prediction×1, option×1, defi×1 per language (AC-1).
const CANONICAL: &[&str] = &[
    // ── zh ──
    "【现货信号】市场:BTC/USDT|币种:BTC|方向:BUY|价格:60000-65000|类型:limit|仓位:5%|有效期:1h",
    "【现货信号】市场:Solana|币种:WIF|方向:BUY|价格:1.5-2.0|合约地址:EKpQ6uzn|滑点:3%|仓位:10%|有效期:30min",
    "【合约信号】交易对:BTC-PERP|方向:LONG|杠杆:10|入场:60000-61000|止损:59000|止盈1:62000|止盈2:63000|止盈3:64000|保证金:cross|仓位:5%|有效期:2h",
    "【合约信号】交易对:ETH-PERP|方向:SHORT|杠杆:5|入场:3000-3100|止损:3200|止盈:2900/2800|保证金:isolated|仓位:8%|有效期:1d",
    "【预测市场信号】事件:美联储12月降息|结果:YES @0.65|结算日:2025-12-31|仓位:5%|有效期:3d",
    "【期权信号】合约代码:BTC-251231-60000-C|方向:买入|类型:Call|行权价:60000|到期日:2025-12-31|权利金上限:1500|仓位:5%|有效期:5d",
    "【DeFi 信号】链:Ethereum|协议:AaveV3|年化:5.5%|锁仓:1.2B|币种:USDC|赎回:活期|仓位:10%|有效期:7d",
    // ── en ──
    "【Spot Signal】market:ETH/USDT|symbol:ETH|side:SELL|price:3000-3200|orderType:market|position:0.1%|ttl:5min",
    "【Spot Signal】market:base|symbol:DEGEN|side:BUY|price:0.01-0.02|tokenAddr:0xabc123|slippage:5%|position:20%|ttl:7d",
    "【Futures Signal】pair:SOL-PERP|direction:LONG|leverage:20|entry:150-155|stopLoss:145|tp1:160|position:3%|ttl:6h",
    "【Futures Signal】pair:BNB-PERP|direction:SHORT|leverage:3|entry:600-610|stopLoss:620|takeProfit:590/580/570|marginMode:cross|position:12%|ttl:12h",
    "【Prediction Signal】event:BTC above 100k by EOY|outcome:UP @0.4|settleDate:2024-02-29|position:2%|ttl:7d",
    "【Options Signal】contractCode:ETH-250630-3000-P|side:Sell|optionType:Put|strike:3000|expiry:2025-06-30|premiumCap:200|position:4%|ttl:2d",
    "【DeFi Signal】chain:Solana|protocolPool:Kamino|apy:8%|tvl:500M|token:USDC|redeemTerms:flexible|position:15%|ttl:1d",
];

fn code(text: &str) -> &'static str {
    parse_signal_text(text).unwrap_err().code()
}

// ── Positives ─────────────────────────────────────────────────────────────────

/// AC-1 / AC-2: every canonical example parses successfully (14/14 target).
#[test]
fn ac1_all_canonical_examples_parse() {
    for (i, text) in CANONICAL.iter().enumerate() {
        assert!(
            parse_signal_text(text).is_ok(),
            "canonical example {i} failed to parse: code={}",
            parse_signal_text(text)
                .map(|_| "ok")
                .unwrap_or_else(|e| e.code())
        );
    }
}

/// AC-9: `assetClass` (top) always equals `params.kind`.
#[test]
fn ac9_asset_class_matches_params_kind() {
    for text in CANONICAL {
        let p = parse_signal_text(text).unwrap();
        let kind = match p.params {
            SignalParams::Spot(_) => "spot",
            SignalParams::Perp(_) => "perp",
            SignalParams::Prediction(_) => "prediction",
            SignalParams::Option(_) => "option",
            SignalParams::Defi(_) => "defi",
        };
        assert_eq!(p.asset_class.as_str(), kind);
    }
}

/// AC-3: spot CEX (no tokenAddr) + on-chain (tokenAddr + slippage) forms.
#[test]
fn ac3_spot_cex_and_onchain() {
    let cex = parse_signal_text(CANONICAL[0]).unwrap();
    match cex.params {
        SignalParams::Spot(s) => {
            assert!(s.token_addr.is_none() && s.slippage.is_none());
            assert_eq!(s.price_range.lo, "60000");
            assert_eq!(s.price_range.hi, "65000");
        }
        _ => panic!("expected spot"),
    }
    let onchain = parse_signal_text(CANONICAL[1]).unwrap();
    match onchain.params {
        SignalParams::Spot(s) => {
            assert_eq!(s.token_addr.as_deref(), Some("EKpQ6uzn"));
            assert_eq!(s.slippage.as_deref(), Some("3"));
            assert_eq!(s.price_range.hi, "2"); // 2.0 normalized
        }
        _ => panic!("expected spot"),
    }
}

/// AC-4: perp LONG (separate tp1..3) & SHORT (combined slash TP), cross & isolated.
#[test]
fn ac4_perp_directions_and_tp_forms() {
    let long = parse_signal_text(CANONICAL[2]).unwrap();
    match long.params {
        SignalParams::Perp(p) => {
            assert_eq!(p.leverage, 10);
            assert_eq!(p.take_profit, vec!["62000", "63000", "64000"]);
            assert_eq!(p.stop_loss, "59000");
        }
        _ => panic!("expected perp"),
    }
    // SHORT with the combined slash TP form.
    let short = parse_signal_text(CANONICAL[3]).unwrap();
    match short.params {
        SignalParams::Perp(p) => assert_eq!(p.take_profit, vec!["2900", "2800"]),
        _ => panic!("expected perp"),
    }
    // en SHORT with a 3-item combined slash TP.
    let short_en = parse_signal_text(CANONICAL[10]).unwrap();
    match short_en.params {
        SignalParams::Perp(p) => assert_eq!(p.take_profit, vec!["590", "580", "570"]),
        _ => panic!("expected perp"),
    }
}

/// AC-8: TTL 5min & 7d boundaries; position 0.1% & 20% boundaries; leap-year date.
#[test]
fn ac8_boundaries() {
    let p = parse_signal_text(CANONICAL[7]).unwrap(); // en spot CEX
    assert_eq!(p.ttl_sec, 300); // 5min
    assert_eq!(p.position_pct, "0.1");
    let p = parse_signal_text(CANONICAL[8]).unwrap(); // en spot on-chain
    assert_eq!(p.ttl_sec, 604_800); // 7d
    assert_eq!(p.position_pct, "20");
    let p = parse_signal_text(CANONICAL[11]).unwrap(); // en prediction (leap year)
    match p.params {
        SignalParams::Prediction(pr) => {
            assert_eq!(pr.settle_date, "2024-02-29");
            assert_eq!(pr.odds, "0.4");
        }
        _ => panic!("expected prediction"),
    }
}

/// AC-5 / AC-6: outcomes (with the `@odds` form) and option side/type coverage.
#[test]
fn ac5_ac6_outcome_and_option() {
    for (text, ok) in [
        (
            "【预测市场信号】事件:x|结果:NO @0.5|结算日:2025-12-31|仓位:5%|有效期:1d",
            true,
        ),
        (
            "【预测市场信号】事件:x|结果:DOWN @0.5|结算日:2025-12-31|仓位:5%|有效期:1d",
            true,
        ),
    ] {
        assert_eq!(parse_signal_text(text).is_ok(), ok);
    }
    // Option Buy/Call (zh) already covered by CANONICAL[5]; Sell/Put (en) by CANONICAL[12].
    let opt = parse_signal_text(CANONICAL[12]).unwrap();
    match opt.params {
        SignalParams::Option(o) => {
            assert_eq!(o.expiry, "2025-06-30");
            assert_eq!(o.strike, "3000");
        }
        _ => panic!("expected option"),
    }
}

// ── Negatives (AC-10 … AC-20) ──────────────────────────────────────────────────

/// AC-10: header preceded by space / unknown / half-width `[` / old short header.
#[test]
fn ac10_header() {
    assert_eq!(code(" 【现货信号】市场:BTC/USDT"), "unknown_header");
    assert_eq!(code("【unknown】市场:BTC"), "unknown_header");
    assert_eq!(code("[现货信号]市场:BTC"), "unknown_header");
    // the self-authored short header is no longer accepted.
    assert_eq!(code("【现货】市场:BTC/USDT|币种:BTC"), "unknown_header");
}

/// AC-11: mixed 中/英 labels.
#[test]
fn ac11_language_mix() {
    assert_eq!(
        code("【现货信号】market:BTC/USDT|币种:BTC|方向:BUY|价格:60000-65000|仓位:5%|有效期:1h"),
        "language_mix"
    );
}

/// AC-12: multi-line, 201 chars, emoji, link, out-of-place @mention, extra field, empty field.
#[test]
fn ac12_forbidden_and_shape() {
    assert_eq!(code("【现货信号】市场:BTC\n方向:BUY"), "multi_line");
    assert_eq!(
        code(&format!("【现货信号】市场:{}", "A".repeat(210))),
        "too_long"
    );
    assert_eq!(code("【现货信号】市场:BTC🚀|方向:BUY"), "forbidden_content");
    assert_eq!(
        code("【现货信号】市场:https://x.io|方向:BUY"),
        "forbidden_content"
    );
    // `@` outside the Prediction outcome field is still forbidden.
    assert_eq!(code("【现货信号】市场:@btc|方向:BUY"), "forbidden_content");
    // extra (unrecognized) label.
    assert_eq!(
        code(
            "【现货信号】市场:BTC/USDT|币种:BTC|方向:BUY|价格:60000-65000|仓位:5%|有效期:1h|额外:1"
        ),
        "forbidden_content"
    );
    // empty field (double pipe).
    assert_eq!(
        code("【现货信号】市场:BTC/USDT||方向:BUY|价格:60000-65000|仓位:5%|有效期:1h"),
        "empty_field"
    );
}

/// AC-12b (feedback !668dedbf): a fixed-order violation — every label valid but
/// out of the canonical order — is rejected.
#[test]
fn ac12b_fixed_order_reorder_rejected() {
    // 币种 (symbol) before 市场 (market).
    assert_eq!(
        code("【现货信号】币种:BTC|市场:BTC/USDT|方向:BUY|价格:60000-65000|仓位:5%|有效期:1h"),
        "field_count_error"
    );
}

/// AC-13: position 0 / 20.1 / range / multi.
#[test]
fn ac13_position() {
    let base = |pos: &str| {
        format!("【现货信号】市场:BTC/USDT|币种:BTC|方向:BUY|价格:60000-65000|仓位:{pos}|有效期:1h")
    };
    for pos in ["0%", "20.1%", "5-10%", "5%,10%"] {
        assert_eq!(code(&base(pos)), "out_of_range", "position {pos}");
    }
}

/// AC-14: TTL 4min / >7d / unknown unit.
#[test]
fn ac14_ttl() {
    let base = |ttl: &str| {
        format!("【现货信号】市场:BTC/USDT|币种:BTC|方向:BUY|价格:60000-65000|仓位:5%|有效期:{ttl}")
    };
    for ttl in ["4min", "8d", "30s"] {
        assert_eq!(code(&base(ttl)), "out_of_range", "ttl {ttl}");
    }
}

/// AC-15: sci-notation, thousands separator, %-price, inverted range.
#[test]
fn ac15_numbers() {
    let base = |price: &str| {
        format!("【现货信号】市场:BTC/USDT|币种:BTC|方向:BUY|价格:{price}|仓位:5%|有效期:1h")
    };
    assert_eq!(code(&base("1e3-2e3")), "invalid_number"); // sci-notation
    assert_eq!(code(&base("1,000-2,000")), "invalid_number"); // thousands sep
    assert_eq!(code(&base("5%-10%")), "invalid_number"); // %-price
    assert_eq!(code(&base("65000-60000")), "out_of_range"); // inverted
}

/// AC-16: perp SL/TP wrong direction, duplicate SL, 0 TP, TP numbering gap.
/// (monotonic ordering is no longer a constraint — feedback !1a4cebc6.)
#[test]
fn ac16_perp_direction() {
    // wrong-direction SL (LONG, SL not below entry-low).
    assert_eq!(
        code("【合约信号】交易对:BTC-PERP|方向:LONG|杠杆:10|入场:60000-61000|止损:60500|止盈1:62000|仓位:5%|有效期:1h"),
        "direction_constraint"
    );
    // wrong-direction TP (LONG, TP below entry-low).
    assert_eq!(
        code("【合约信号】交易对:BTC-PERP|方向:LONG|杠杆:10|入场:60000-61000|止损:59000|止盈1:59500|仓位:5%|有效期:1h"),
        "direction_constraint"
    );
    // duplicate stop-loss → a fixed-order violation.
    assert_eq!(
        code("【合约信号】交易对:BTC-PERP|方向:LONG|杠杆:10|入场:60000-61000|止损:59000|止损:58000|止盈1:62000|仓位:5%|有效期:1h"),
        "field_count_error"
    );
    // 0 take-profit.
    assert_eq!(
        code("【合约信号】交易对:BTC-PERP|方向:LONG|杠杆:10|入场:60000-61000|止损:59000|仓位:5%|有效期:1h"),
        "direction_constraint"
    );
    // TP numbering gap (tp1 + tp3 without tp2).
    assert_eq!(
        code("【Futures Signal】pair:BTC-PERP|direction:LONG|leverage:10|entry:60000-61000|stopLoss:59000|tp1:62000|tp3:64000|position:5%|ttl:1h"),
        "direction_constraint"
    );
}

/// AC-16b (feedback !1a4cebc6): a LONG signal whose TPs are direction-correct but
/// NOT strictly ascending now PARSES (the extra monotonic constraint was removed).
#[test]
fn ac16b_non_monotonic_tps_accepted() {
    assert!(parse_signal_text(
        "【合约信号】交易对:BTC-PERP|方向:LONG|杠杆:10|入场:60000-61000|止损:59000|止盈1:64000|止盈2:62000|仓位:5%|有效期:1h"
    )
    .is_ok());
}

/// AC-17: prediction illegal outcome, odds out of [0,1], missing year, nonexistent date.
#[test]
fn ac17_prediction() {
    assert_eq!(
        code("【预测市场信号】事件:x|结果:MAYBE @0.5|结算日:2025-12-31|仓位:5%|有效期:1d"),
        "illegal_keyword"
    );
    assert_eq!(
        code("【预测市场信号】事件:x|结果:YES @1.5|结算日:2025-12-31|仓位:5%|有效期:1d"),
        "out_of_range"
    );
    assert_eq!(
        code("【预测市场信号】事件:x|结果:YES @0.5|结算日:12-31|仓位:5%|有效期:1d"),
        "invalid_date"
    );
    assert_eq!(
        code("【预测市场信号】事件:x|结果:YES @0.5|结算日:2025-02-30|仓位:5%|有效期:1d"),
        "invalid_date"
    );
}

/// AC-18: option contractCode inconsistent with Call/Put, strike, or expiry.
#[test]
fn ac18_option_mismatch() {
    // C code but optionType Put.
    assert_eq!(
        code("【期权信号】合约代码:BTC-251231-60000-C|方向:买入|类型:Put|行权价:60000|到期日:2025-12-31|权利金上限:1500|仓位:5%|有效期:5d"),
        "option_field_mismatch"
    );
    // strike mismatch.
    assert_eq!(
        code("【期权信号】合约代码:BTC-251231-60000-C|方向:买入|类型:Call|行权价:59000|到期日:2025-12-31|权利金上限:1500|仓位:5%|有效期:5d"),
        "option_field_mismatch"
    );
    // expiry mismatch.
    assert_eq!(
        code("【期权信号】合约代码:BTC-251231-60000-C|方向:买入|类型:Call|行权价:60000|到期日:2025-12-30|权利金上限:1500|仓位:5%|有效期:5d"),
        "option_field_mismatch"
    );
}

/// AC-19: DeFi missing APY / redeem terms (fixed-order required field absent).
#[test]
fn ac19_defi_missing_field() {
    // missing 年化 (apy).
    assert_eq!(
        code(
            "【DeFi 信号】链:Ethereum|协议:AaveV3|锁仓:1.2B|币种:USDC|赎回:活期|仓位:10%|有效期:7d"
        ),
        "field_count_error"
    );
    // missing 赎回 (redeemTerms).
    assert_eq!(
        code("【DeFi 信号】链:Ethereum|协议:AaveV3|年化:5%|锁仓:1.2B|币种:USDC|仓位:10%|有效期:7d"),
        "field_count_error"
    );
}

/// AC-20: envelope schemaVersion ≠ 2 / signalTime = 0 / illegal deliveryId — each
/// a distinct fine-grained code (feedback !92ea45d6 / !42eef591).
#[test]
fn ac20_envelope() {
    let text =
        "【Spot Signal】market:BTC/USDT|symbol:BTC|side:BUY|price:60000-65000|position:5%|ttl:1h";
    let mk = |schema: u32, delivery: &str, time: u64| {
        format!("{{\"schemaVersion\":{schema},\"deliveryId\":\"{delivery}\",\"signalTime\":{time},\"signalText\":\"{text}\"}}")
    };
    assert!(parse_envelope(&mk(2, "abc123", 1)).is_ok());
    assert_eq!(
        parse_envelope(&mk(1, "abc123", 1)).unwrap_err().code(),
        "invalid_schema_version"
    );
    assert_eq!(
        parse_envelope(&mk(2, "abc123", 0)).unwrap_err().code(),
        "invalid_signal_time"
    );
    assert_eq!(
        parse_envelope(&mk(2, "bad id", 1)).unwrap_err().code(),
        "invalid_delivery_id"
    );
}

// ── Invariants ──────────────────────────────────────────────────────────────

/// AC-24: no error path echoes the raw signal text / tokenAddr / event / contractCode.
#[test]
fn ac24_errors_never_leak_input() {
    let leaky_inputs = [
        // tokenAddr in an otherwise-bad on-chain spot (bad slippage).
        "【现货信号】市场:base|币种:X|方向:BUY|价格:1-2|合约地址:0xSECRETADDR|滑点:9%|仓位:5%|有效期:1h",
        // event free text in a bad prediction (odds out of range).
        "【预测市场信号】事件:SECRETEVENTTEXT|结果:YES @9|结算日:2025-12-31|仓位:5%|有效期:1d",
        // contractCode in a mismatched option.
        "【期权信号】合约代码:SECRETCODE-251231-60000-C|方向:买入|类型:Put|行权价:60000|到期日:2025-12-31|权利金上限:1|仓位:5%|有效期:5d",
    ];
    let secrets = ["0xSECRETADDR", "SECRETEVENTTEXT", "SECRETCODE"];
    for input in leaky_inputs {
        let e = parse_signal_text(input).unwrap_err();
        let rendered = format!("{} {} {:?}", e.code(), e.message(), e.field());
        for secret in secrets {
            assert!(
                !rendered.contains(secret),
                "error leaked '{secret}' for input starting {}",
                &input[..12.min(input.len())]
            );
        }
    }
}

/// AC-23: exactly one `AssetClass` type is referenced crate-wide (the crate-root
/// module). This compiles only against `crate::asset_class::AssetClass`; a second
/// definition would be a duplicate flagged at review / by `onchainos_check`.
#[test]
fn ac23_single_asset_class_type() {
    let c: crate::asset_class::AssetClass = crate::asset_class::AssetClass::Spot;
    assert_eq!(c.as_str(), "spot");
}
