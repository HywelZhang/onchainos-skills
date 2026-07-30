//! Corpus + invariant tests for the trade-signal parser (AC-1 … AC-24).
//!
//! Per V1.1/TD review alignment (feedback !ee926d82) the previous self-authored
//! corpus is replaced with examples in the authoritative V1.1 grammar: the full
//! titles (the full zh/en signal headers), the fixed-order `|` fields per
//! asset class, the Prediction `<OUTCOME> @<odds>` form, and both perp TP forms
//! (separate tp1..3 and the combined slash takeProfit form).
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
    "【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC/USDT|\u{5e01}\u{79cd}:BTC|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:60000-65000|\u{7c7b}\u{578b}:limit|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h",
    "【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:Solana|\u{5e01}\u{79cd}:WIF|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:1.5-2.0|\u{5408}\u{7ea6}\u{5730}\u{5740}:EKpQ6uzn|\u{6ed1}\u{70b9}:3%|\u{4ed3}\u{4f4d}:10%|\u{6709}\u{6548}\u{671f}:30min",
    "【\u{5408}\u{7ea6}\u{4fe1}\u{53f7}】\u{4ea4}\u{6613}\u{5bf9}:BTC-PERP|\u{65b9}\u{5411}:LONG|\u{6760}\u{6746}:10|\u{5165}\u{573a}:60000-61000|\u{6b62}\u{635f}:59000|\u{6b62}\u{76c8}1:62000|\u{6b62}\u{76c8}2:63000|\u{6b62}\u{76c8}3:64000|\u{4fdd}\u{8bc1}\u{91d1}:cross|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:2h",
    "【\u{5408}\u{7ea6}\u{4fe1}\u{53f7}】\u{4ea4}\u{6613}\u{5bf9}:ETH-PERP|\u{65b9}\u{5411}:SHORT|\u{6760}\u{6746}:5|\u{5165}\u{573a}:3000-3100|\u{6b62}\u{635f}:3200|\u{6b62}\u{76c8}:2900/2800|\u{4fdd}\u{8bc1}\u{91d1}:isolated|\u{4ed3}\u{4f4d}:8%|\u{6709}\u{6548}\u{671f}:1d",
    "【\u{9884}\u{6d4b}\u{5e02}\u{573a}\u{4fe1}\u{53f7}】\u{4e8b}\u{4ef6}:\u{7f8e}\u{8054}\u{50a8}12\u{6708}\u{964d}\u{606f}|\u{7ed3}\u{679c}:YES @0.65|\u{7ed3}\u{7b97}\u{65e5}:2025-12-31|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:3d",
    "【\u{671f}\u{6743}\u{4fe1}\u{53f7}】\u{5408}\u{7ea6}\u{4ee3}\u{7801}:BTC-251231-60000-C|\u{65b9}\u{5411}:\u{4e70}\u{5165}|\u{7c7b}\u{578b}:Call|\u{884c}\u{6743}\u{4ef7}:60000|\u{5230}\u{671f}\u{65e5}:2025-12-31|\u{6743}\u{5229}\u{91d1}\u{4e0a}\u{9650}:1500|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:5d",
    "【DeFi \u{4fe1}\u{53f7}】\u{94fe}:Ethereum|\u{534f}\u{8bae}:AaveV3|\u{5e74}\u{5316}:5.5%|\u{9501}\u{4ed3}:1.2B|\u{5e01}\u{79cd}:USDC|\u{8d4e}\u{56de}:\u{6d3b}\u{671f}|\u{4ed3}\u{4f4d}:10%|\u{6709}\u{6548}\u{671f}:7d",
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
            "【\u{9884}\u{6d4b}\u{5e02}\u{573a}\u{4fe1}\u{53f7}】\u{4e8b}\u{4ef6}:x|\u{7ed3}\u{679c}:NO @0.5|\u{7ed3}\u{7b97}\u{65e5}:2025-12-31|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1d",
            true,
        ),
        (
            "【\u{9884}\u{6d4b}\u{5e02}\u{573a}\u{4fe1}\u{53f7}】\u{4e8b}\u{4ef6}:x|\u{7ed3}\u{679c}:DOWN @0.5|\u{7ed3}\u{7b97}\u{65e5}:2025-12-31|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1d",
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
    assert_eq!(
        code(" 【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC/USDT"),
        "unknown_header"
    );
    assert_eq!(code("【unknown】\u{5e02}\u{573a}:BTC"), "unknown_header");
    assert_eq!(
        code("[\u{73b0}\u{8d27}\u{4fe1}\u{53f7}]\u{5e02}\u{573a}:BTC"),
        "unknown_header"
    );
    // the self-authored short header is no longer accepted.
    assert_eq!(
        code("【\u{73b0}\u{8d27}】\u{5e02}\u{573a}:BTC/USDT|\u{5e01}\u{79cd}:BTC"),
        "unknown_header"
    );
}

/// AC-11: mixed zh/en labels.
#[test]
fn ac11_language_mix() {
    assert_eq!(
        code("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】market:BTC/USDT|\u{5e01}\u{79cd}:BTC|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:60000-65000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h"),
        "language_mix"
    );
}

/// AC-12: multi-line, 201 chars, emoji, link, out-of-place @mention, extra field, empty field.
#[test]
fn ac12_forbidden_and_shape() {
    assert_eq!(
        code("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC\n\u{65b9}\u{5411}:BUY"),
        "multi_line"
    );
    assert_eq!(
        code(&format!(
            "【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:{}",
            "A".repeat(210)
        )),
        "too_long"
    );
    assert_eq!(
        code("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC🚀|\u{65b9}\u{5411}:BUY"),
        "forbidden_content"
    );
    assert_eq!(
        code("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:https://x.io|\u{65b9}\u{5411}:BUY"),
        "forbidden_content"
    );
    // `@` outside the Prediction outcome field is still forbidden.
    assert_eq!(
        code("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:@btc|\u{65b9}\u{5411}:BUY"),
        "forbidden_content"
    );
    // extra (unrecognized) label.
    assert_eq!(
        code(
            "【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC/USDT|\u{5e01}\u{79cd}:BTC|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:60000-65000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h|\u{989d}\u{5916}:1"
        ),
        "forbidden_content"
    );
    // empty field (double pipe).
    assert_eq!(
        code("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC/USDT||\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:60000-65000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h"),
        "empty_field"
    );
}

/// AC-12b (feedback !668dedbf): a fixed-order violation — every label valid but
/// out of the canonical order — is rejected.
#[test]
fn ac12b_fixed_order_reorder_rejected() {
    // symbol before market (reordered).
    assert_eq!(
        code("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e01}\u{79cd}:BTC|\u{5e02}\u{573a}:BTC/USDT|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:60000-65000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h"),
        "field_count_error"
    );
}

/// AC-13: position 0 / 20.1 / range / multi.
#[test]
fn ac13_position() {
    let base = |pos: &str| {
        format!("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC/USDT|\u{5e01}\u{79cd}:BTC|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:60000-65000|\u{4ed3}\u{4f4d}:{pos}|\u{6709}\u{6548}\u{671f}:1h")
    };
    for pos in ["0%", "20.1%", "5-10%", "5%,10%"] {
        assert_eq!(code(&base(pos)), "out_of_range", "position {pos}");
    }
}

/// AC-14: TTL 4min / >7d / unknown unit.
#[test]
fn ac14_ttl() {
    let base = |ttl: &str| {
        format!("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC/USDT|\u{5e01}\u{79cd}:BTC|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:60000-65000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:{ttl}")
    };
    for ttl in ["4min", "8d", "30s"] {
        assert_eq!(code(&base(ttl)), "out_of_range", "ttl {ttl}");
    }
}

/// AC-15: sci-notation, thousands separator, %-price, inverted range.
#[test]
fn ac15_numbers() {
    let base = |price: &str| {
        format!("【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:BTC/USDT|\u{5e01}\u{79cd}:BTC|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:{price}|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h")
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
        code("【\u{5408}\u{7ea6}\u{4fe1}\u{53f7}】\u{4ea4}\u{6613}\u{5bf9}:BTC-PERP|\u{65b9}\u{5411}:LONG|\u{6760}\u{6746}:10|\u{5165}\u{573a}:60000-61000|\u{6b62}\u{635f}:60500|\u{6b62}\u{76c8}1:62000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h"),
        "direction_constraint"
    );
    // wrong-direction TP (LONG, TP below entry-low).
    assert_eq!(
        code("【\u{5408}\u{7ea6}\u{4fe1}\u{53f7}】\u{4ea4}\u{6613}\u{5bf9}:BTC-PERP|\u{65b9}\u{5411}:LONG|\u{6760}\u{6746}:10|\u{5165}\u{573a}:60000-61000|\u{6b62}\u{635f}:59000|\u{6b62}\u{76c8}1:59500|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h"),
        "direction_constraint"
    );
    // duplicate stop-loss → a fixed-order violation.
    assert_eq!(
        code("【\u{5408}\u{7ea6}\u{4fe1}\u{53f7}】\u{4ea4}\u{6613}\u{5bf9}:BTC-PERP|\u{65b9}\u{5411}:LONG|\u{6760}\u{6746}:10|\u{5165}\u{573a}:60000-61000|\u{6b62}\u{635f}:59000|\u{6b62}\u{635f}:58000|\u{6b62}\u{76c8}1:62000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h"),
        "field_count_error"
    );
    // 0 take-profit.
    assert_eq!(
        code("【\u{5408}\u{7ea6}\u{4fe1}\u{53f7}】\u{4ea4}\u{6613}\u{5bf9}:BTC-PERP|\u{65b9}\u{5411}:LONG|\u{6760}\u{6746}:10|\u{5165}\u{573a}:60000-61000|\u{6b62}\u{635f}:59000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h"),
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
        "【\u{5408}\u{7ea6}\u{4fe1}\u{53f7}】\u{4ea4}\u{6613}\u{5bf9}:BTC-PERP|\u{65b9}\u{5411}:LONG|\u{6760}\u{6746}:10|\u{5165}\u{573a}:60000-61000|\u{6b62}\u{635f}:59000|\u{6b62}\u{76c8}1:64000|\u{6b62}\u{76c8}2:62000|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h"
    )
    .is_ok());
}

/// AC-17: prediction illegal outcome, odds out of [0,1], missing year, nonexistent date.
#[test]
fn ac17_prediction() {
    assert_eq!(
        code("【\u{9884}\u{6d4b}\u{5e02}\u{573a}\u{4fe1}\u{53f7}】\u{4e8b}\u{4ef6}:x|\u{7ed3}\u{679c}:MAYBE @0.5|\u{7ed3}\u{7b97}\u{65e5}:2025-12-31|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1d"),
        "illegal_keyword"
    );
    assert_eq!(
        code("【\u{9884}\u{6d4b}\u{5e02}\u{573a}\u{4fe1}\u{53f7}】\u{4e8b}\u{4ef6}:x|\u{7ed3}\u{679c}:YES @1.5|\u{7ed3}\u{7b97}\u{65e5}:2025-12-31|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1d"),
        "out_of_range"
    );
    assert_eq!(
        code("【\u{9884}\u{6d4b}\u{5e02}\u{573a}\u{4fe1}\u{53f7}】\u{4e8b}\u{4ef6}:x|\u{7ed3}\u{679c}:YES @0.5|\u{7ed3}\u{7b97}\u{65e5}:12-31|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1d"),
        "invalid_date"
    );
    assert_eq!(
        code("【\u{9884}\u{6d4b}\u{5e02}\u{573a}\u{4fe1}\u{53f7}】\u{4e8b}\u{4ef6}:x|\u{7ed3}\u{679c}:YES @0.5|\u{7ed3}\u{7b97}\u{65e5}:2025-02-30|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1d"),
        "invalid_date"
    );
}

/// AC-18: option contractCode inconsistent with Call/Put, strike, or expiry.
#[test]
fn ac18_option_mismatch() {
    // C code but optionType Put.
    assert_eq!(
        code("【\u{671f}\u{6743}\u{4fe1}\u{53f7}】\u{5408}\u{7ea6}\u{4ee3}\u{7801}:BTC-251231-60000-C|\u{65b9}\u{5411}:\u{4e70}\u{5165}|\u{7c7b}\u{578b}:Put|\u{884c}\u{6743}\u{4ef7}:60000|\u{5230}\u{671f}\u{65e5}:2025-12-31|\u{6743}\u{5229}\u{91d1}\u{4e0a}\u{9650}:1500|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:5d"),
        "option_field_mismatch"
    );
    // strike mismatch.
    assert_eq!(
        code("【\u{671f}\u{6743}\u{4fe1}\u{53f7}】\u{5408}\u{7ea6}\u{4ee3}\u{7801}:BTC-251231-60000-C|\u{65b9}\u{5411}:\u{4e70}\u{5165}|\u{7c7b}\u{578b}:Call|\u{884c}\u{6743}\u{4ef7}:59000|\u{5230}\u{671f}\u{65e5}:2025-12-31|\u{6743}\u{5229}\u{91d1}\u{4e0a}\u{9650}:1500|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:5d"),
        "option_field_mismatch"
    );
    // expiry mismatch.
    assert_eq!(
        code("【\u{671f}\u{6743}\u{4fe1}\u{53f7}】\u{5408}\u{7ea6}\u{4ee3}\u{7801}:BTC-251231-60000-C|\u{65b9}\u{5411}:\u{4e70}\u{5165}|\u{7c7b}\u{578b}:Call|\u{884c}\u{6743}\u{4ef7}:60000|\u{5230}\u{671f}\u{65e5}:2025-12-30|\u{6743}\u{5229}\u{91d1}\u{4e0a}\u{9650}:1500|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:5d"),
        "option_field_mismatch"
    );
}

/// AC-19: DeFi missing APY / redeem terms (fixed-order required field absent).
#[test]
fn ac19_defi_missing_field() {
    // missing apy.
    assert_eq!(
        code(
            "【DeFi \u{4fe1}\u{53f7}】\u{94fe}:Ethereum|\u{534f}\u{8bae}:AaveV3|\u{9501}\u{4ed3}:1.2B|\u{5e01}\u{79cd}:USDC|\u{8d4e}\u{56de}:\u{6d3b}\u{671f}|\u{4ed3}\u{4f4d}:10%|\u{6709}\u{6548}\u{671f}:7d"
        ),
        "field_count_error"
    );
    // missing redeemTerms.
    assert_eq!(
        code("【DeFi \u{4fe1}\u{53f7}】\u{94fe}:Ethereum|\u{534f}\u{8bae}:AaveV3|\u{5e74}\u{5316}:5%|\u{9501}\u{4ed3}:1.2B|\u{5e01}\u{79cd}:USDC|\u{4ed3}\u{4f4d}:10%|\u{6709}\u{6548}\u{671f}:7d"),
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
        "【\u{73b0}\u{8d27}\u{4fe1}\u{53f7}】\u{5e02}\u{573a}:base|\u{5e01}\u{79cd}:X|\u{65b9}\u{5411}:BUY|\u{4ef7}\u{683c}:1-2|\u{5408}\u{7ea6}\u{5730}\u{5740}:0xSECRETADDR|\u{6ed1}\u{70b9}:9%|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1h",
        // event free text in a bad prediction (odds out of range).
        "【\u{9884}\u{6d4b}\u{5e02}\u{573a}\u{4fe1}\u{53f7}】\u{4e8b}\u{4ef6}:SECRETEVENTTEXT|\u{7ed3}\u{679c}:YES @9|\u{7ed3}\u{7b97}\u{65e5}:2025-12-31|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:1d",
        // contractCode in a mismatched option.
        "【\u{671f}\u{6743}\u{4fe1}\u{53f7}】\u{5408}\u{7ea6}\u{4ee3}\u{7801}:SECRETCODE-251231-60000-C|\u{65b9}\u{5411}:\u{4e70}\u{5165}|\u{7c7b}\u{578b}:Put|\u{884c}\u{6743}\u{4ef7}:60000|\u{5230}\u{671f}\u{65e5}:2025-12-31|\u{6743}\u{5229}\u{91d1}\u{4e0a}\u{9650}:1|\u{4ed3}\u{4f4d}:5%|\u{6709}\u{6548}\u{671f}:5d",
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
