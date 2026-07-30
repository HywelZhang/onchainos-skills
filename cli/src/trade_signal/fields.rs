//! FR-2 steps 4-8: fixed-order field parsing, per-position label→id translation
//! with the same-language check, and the shared value validators (position %,
//! TTL, exact decimal, price range, calendar date, keyword whitelists,
//! forbidden-content scan).
//!
//! Per V1.1/TD review alignment (feedback !668dedbf) this is NOT a generic
//! reorderable `label:value` map: each asset class defines a FIXED ORDER of `|`
//! fields, and a field that is missing (required), extra (unknown label), or
//! out-of-order is rejected. Optional fields may be absent but, when present,
//! must occupy their canonical slot. Labels are recognized only within the
//! spec-defined fields for the header's language.
//!
//! All numeric parse/compare goes through the repo's exact [`Decimal`] (no float,
//! NFR-2). `Decimal::parse` already rejects sign / exponent / whitespace /
//! thousands-separator / lone-dot, which satisfies the PRD numeric grammar.

use crate::asset_class::AssetClass;
use crate::commands::agent_commerce::task::common::autotrade::amount::Decimal;

use super::error::ParseError;
use super::{Direction, Language, MarginMode, OptionType, OrderType, Outcome, PriceRange, Side};

// ── Canonical field ids (stable within the parser; not a wire contract) ──────
pub const ID_POSITION: &str = "position";
pub const ID_TTL: &str = "ttl";
pub const ID_MARKET: &str = "market";
pub const ID_SYMBOL: &str = "symbol";
pub const ID_SIDE: &str = "side";
pub const ID_PRICE: &str = "price";
pub const ID_ORDER_TYPE: &str = "orderType";
pub const ID_TOKEN_ADDR: &str = "tokenAddr";
pub const ID_SLIPPAGE: &str = "slippage";
pub const ID_PAIR: &str = "pair";
pub const ID_DIRECTION: &str = "direction";
pub const ID_LEVERAGE: &str = "leverage";
pub const ID_ENTRY: &str = "entry";
pub const ID_STOP_LOSS: &str = "stopLoss";
/// Combined slash form (`v1/v2/v3`) — the alternate to separate `tp1..tp3` fields.
pub const ID_TP: &str = "takeProfit";
pub const ID_TP1: &str = "tp1";
pub const ID_TP2: &str = "tp2";
pub const ID_TP3: &str = "tp3";
pub const ID_MARGIN_MODE: &str = "marginMode";
pub const ID_EVENT: &str = "event";
/// Prediction outcome+odds are one fixed-position field: `<OUTCOME> @<odds>`.
pub const ID_OUTCOME: &str = "outcome";
pub const ID_SETTLE_DATE: &str = "settleDate";
pub const ID_CONTRACT_CODE: &str = "contractCode";
pub const ID_OPTION_TYPE: &str = "optionType";
pub const ID_STRIKE: &str = "strike";
pub const ID_EXPIRY: &str = "expiry";
pub const ID_PREMIUM_CAP: &str = "premiumCap";
pub const ID_CHAIN: &str = "chain";
pub const ID_PROTOCOL_POOL: &str = "protocolPool";
pub const ID_APY: &str = "apy";
pub const ID_TVL: &str = "tvl";
pub const ID_TOKEN: &str = "token";
pub const ID_REDEEM_TERMS: &str = "redeemTerms";

// ── Fixed-order field specs per asset class ──────────────────────────────────
//
// `(id, zh_label, en_label, required)` in canonical order. The shared
// `position`/`ttl` fields always occupy the final two (required) slots.

/// One field's position in a class's canonical order.
struct FieldSpec {
    id: &'static str,
    zh: &'static str,
    en: &'static str,
    required: bool,
}

const fn f(id: &'static str, zh: &'static str, en: &'static str, required: bool) -> FieldSpec {
    FieldSpec {
        id,
        zh,
        en,
        required,
    }
}

const SPOT_SPEC: &[FieldSpec] = &[
    f(ID_MARKET, "市场", "market", true),
    f(ID_SYMBOL, "币种", "symbol", true),
    f(ID_SIDE, "方向", "side", true),
    f(ID_PRICE, "价格", "price", true),
    f(ID_ORDER_TYPE, "类型", "orderType", false),
    f(ID_TOKEN_ADDR, "合约地址", "tokenAddr", false),
    f(ID_SLIPPAGE, "滑点", "slippage", false),
    f(ID_POSITION, "仓位", "position", true),
    f(ID_TTL, "有效期", "ttl", true),
];

const PERP_SPEC: &[FieldSpec] = &[
    f(ID_PAIR, "交易对", "pair", true),
    f(ID_DIRECTION, "方向", "direction", true),
    f(ID_LEVERAGE, "杠杆", "leverage", true),
    f(ID_ENTRY, "入场", "entry", true),
    f(ID_STOP_LOSS, "止损", "stopLoss", true),
    f(ID_TP, "止盈", "takeProfit", false),
    f(ID_TP1, "止盈1", "tp1", false),
    f(ID_TP2, "止盈2", "tp2", false),
    f(ID_TP3, "止盈3", "tp3", false),
    f(ID_MARGIN_MODE, "保证金", "marginMode", false),
    f(ID_POSITION, "仓位", "position", true),
    f(ID_TTL, "有效期", "ttl", true),
];

const PREDICTION_SPEC: &[FieldSpec] = &[
    f(ID_EVENT, "事件", "event", true),
    f(ID_OUTCOME, "结果", "outcome", true),
    f(ID_SETTLE_DATE, "结算日", "settleDate", true),
    f(ID_POSITION, "仓位", "position", true),
    f(ID_TTL, "有效期", "ttl", true),
];

const OPTION_SPEC: &[FieldSpec] = &[
    f(ID_CONTRACT_CODE, "合约代码", "contractCode", true),
    f(ID_SIDE, "方向", "side", true),
    f(ID_OPTION_TYPE, "类型", "optionType", true),
    f(ID_STRIKE, "行权价", "strike", true),
    f(ID_EXPIRY, "到期日", "expiry", true),
    f(ID_PREMIUM_CAP, "权利金上限", "premiumCap", true),
    f(ID_POSITION, "仓位", "position", true),
    f(ID_TTL, "有效期", "ttl", true),
];

const DEFI_SPEC: &[FieldSpec] = &[
    f(ID_CHAIN, "链", "chain", true),
    f(ID_PROTOCOL_POOL, "协议", "protocolPool", true),
    f(ID_APY, "年化", "apy", true),
    f(ID_TVL, "锁仓", "tvl", true),
    f(ID_TOKEN, "币种", "token", true),
    f(ID_REDEEM_TERMS, "赎回", "redeemTerms", true),
    f(ID_POSITION, "仓位", "position", true),
    f(ID_TTL, "有效期", "ttl", true),
];

fn class_spec(class: AssetClass) -> &'static [FieldSpec] {
    match class {
        AssetClass::Spot => SPOT_SPEC,
        AssetClass::Perp => PERP_SPEC,
        AssetClass::Prediction => PREDICTION_SPEC,
        AssetClass::Option => OPTION_SPEC,
        AssetClass::Defi => DEFI_SPEC,
    }
}

/// Resolve a label to `(canonical_id, label_language)` within this class's spec,
/// or `None` when the label is not valid for the class in either language.
fn resolve_label(spec: &'static [FieldSpec], label: &str) -> Option<(&'static str, Language)> {
    for s in spec {
        if label == s.zh {
            return Some((s.id, Language::Zh));
        }
        if label == s.en {
            return Some((s.id, Language::En));
        }
    }
    None
}

// ── Field splitting ──────────────────────────────────────────────────────────

/// Split the post-header remainder on `|`, trim, and parse each `label:value`.
/// Empty field → [`ParseError::EmptyField`]; a field with no `:` / empty label →
/// [`ParseError::ForbiddenContent`] (content beyond the field grammar).
pub fn split_fields(remainder: &str) -> Result<Vec<(String, String)>, ParseError> {
    if remainder.trim().is_empty() {
        return Err(ParseError::FieldCountError);
    }
    let mut out = Vec::new();
    for part in remainder.split('|') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return Err(ParseError::EmptyField);
        }
        match trimmed.split_once(':') {
            Some((label, value)) => {
                let label = label.trim();
                let value = value.trim();
                if label.is_empty() {
                    return Err(ParseError::ForbiddenContent);
                }
                if value.is_empty() {
                    return Err(ParseError::EmptyField);
                }
                out.push((label.to_string(), value.to_string()));
            }
            None => return Err(ParseError::ForbiddenContent),
        }
    }
    Ok(out)
}

// ── Field map (fixed-order positional validation + consume-once access) ───────

/// An ordered, consume-once map of canonical field id → raw value.
#[derive(Debug)]
pub struct FieldMap {
    entries: Vec<(&'static str, String)>,
}

impl FieldMap {
    /// Build from raw `(label, value)` pairs, enforcing (FR-2 steps 5-6):
    /// - each label is valid for `class` in `header_lang` (else `LanguageMix` /
    ///   `ForbiddenContent`);
    /// - the fields appear in the class's canonical order — a required field that
    ///   is skipped, an out-of-order field, or a duplicate is `FieldCountError`;
    /// - `@` appears ONLY in the Prediction `outcome` field (the odds separator);
    ///   anywhere else it is `ForbiddenContent` (feedback !21bc5915).
    pub fn build(
        class: AssetClass,
        header_lang: Language,
        raw: &[(String, String)],
    ) -> Result<Self, ParseError> {
        let spec = class_spec(class);
        let mut entries: Vec<(&'static str, String)> = Vec::new();
        let mut exp = 0usize; // pointer into the canonical spec order

        for (label, value) in raw {
            let (id, lang) = resolve_label(spec, label).ok_or(ParseError::ForbiddenContent)?;
            if lang != header_lang {
                return Err(ParseError::LanguageMix);
            }
            // `@` is only legal inside the Prediction outcome field.
            let at_ok = class == AssetClass::Prediction && id == ID_OUTCOME;
            if !at_ok && value.contains('@') {
                return Err(ParseError::ForbiddenContent);
            }
            // Advance the canonical pointer to this field's slot; skipping a
            // REQUIRED slot (or running off the end) means missing/reordered/dup.
            loop {
                if exp >= spec.len() {
                    return Err(ParseError::FieldCountError);
                }
                let s = &spec[exp];
                exp += 1;
                if s.id == id {
                    break;
                }
                if s.required {
                    return Err(ParseError::FieldCountError);
                }
            }
            entries.push((id, value.clone()));
        }

        // Any REQUIRED slot not consumed is a missing field.
        if spec[exp..].iter().any(|s| s.required) {
            return Err(ParseError::FieldCountError);
        }
        Ok(FieldMap { entries })
    }

    /// Remove and return the value for `id`, if present.
    pub fn take(&mut self, id: &str) -> Option<String> {
        let pos = self.entries.iter().position(|(eid, _)| *eid == id)?;
        Some(self.entries.remove(pos).1)
    }

    /// Remove and return a required field's value, or [`ParseError::FieldCountError`].
    pub fn require(&mut self, id: &str) -> Result<String, ParseError> {
        self.take(id).ok_or(ParseError::FieldCountError)
    }

    /// Fail if any field remains unconsumed (an extra field for this form).
    pub fn ensure_consumed(&self) -> Result<(), ParseError> {
        if self.entries.is_empty() {
            Ok(())
        } else {
            Err(ParseError::FieldCountError)
        }
    }
}

// ── Forbidden-content scan (SR-2) ─────────────────────────────────────────────

/// True if the input carries content beyond the field grammar: a link or an
/// emoji. CJK labels and full-width brackets are NOT flagged. `@` is handled
/// per-field after header classification (Prediction odds separator is legal),
/// so it is deliberately NOT globally banned here (feedback !21bc5915).
pub fn contains_forbidden(s: &str) -> bool {
    if s.contains("http://") || s.contains("https://") || s.contains("www.") {
        return true;
    }
    s.chars().any(is_emoji)
}

fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    (0x1F000..=0x1FAFF).contains(&cp) // pictographs, emoticons, transport, symbols-extended
        || (0x2600..=0x27BF).contains(&cp) // misc symbols + dingbats
        || (0x2B00..=0x2BFF).contains(&cp) // misc symbols and arrows
        || (0x2100..=0x214F).contains(&cp) // letterlike symbols (™ ℠ ℡ …)
        || (0x2190..=0x21FF).contains(&cp) // arrows (← → ↔ …)
        || (0x2300..=0x23FF).contains(&cp) // misc technical (⌚ ⌛ ⏰ ⏳ …)
        || (0x25A0..=0x25FF).contains(&cp) // geometric shapes (■ ▲ ● ◆ …)
        || cp == 0x200D // zero-width joiner
        || cp == 0xFE0F // variation selector-16
}

// ── Numeric / range / ttl / date validators ───────────────────────────────────

/// `a < b` for exact decimals (`Decimal` exposes only `le` + `PartialEq`).
fn decimal_lt(a: &Decimal, b: &Decimal) -> bool {
    a.le(b) && a != b
}

/// `a < b` for two decimal strings (already-validated values). A parse failure is
/// treated as `false` — callers only compare values that parsed successfully.
pub fn less_than(a: &str, b: &str) -> bool {
    match (Decimal::parse(a), Decimal::parse(b)) {
        (Ok(x), Ok(y)) => decimal_lt(&x, &y),
        _ => false,
    }
}

/// `a > b` for two decimal strings.
pub fn greater_than(a: &str, b: &str) -> bool {
    less_than(b, a)
}

/// `a == b` for two decimal strings (scale-normalized exact equality).
pub fn equal(a: &str, b: &str) -> bool {
    match (Decimal::parse(a), Decimal::parse(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Parse a plain absolute decimal price; malformed → [`ParseError::InvalidNumber`].
pub fn parse_decimal(value: &str) -> Result<String, ParseError> {
    let d = Decimal::parse(value).map_err(|_| ParseError::InvalidNumber)?;
    Ok(d.to_plain_string())
}

/// Parse a `lo-hi` absolute price range; enforces `lo < hi`.
pub fn parse_range(value: &str) -> Result<PriceRange, ParseError> {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 2 {
        return Err(ParseError::InvalidNumber);
    }
    let lo = Decimal::parse(parts[0]).map_err(|_| ParseError::InvalidNumber)?;
    let hi = Decimal::parse(parts[1]).map_err(|_| ParseError::InvalidNumber)?;
    if !decimal_lt(&lo, &hi) {
        return Err(ParseError::OutOfRange);
    }
    Ok(PriceRange {
        lo: lo.to_plain_string(),
        hi: hi.to_plain_string(),
    })
}

/// Parse `positionPct`: a single `N%` in `0.1 ..= 20`; normalized to a plain
/// decimal string without the `%`. Range / multi-value / `0` → [`ParseError::OutOfRange`].
pub fn parse_position(value: &str) -> Result<String, ParseError> {
    let num = value.strip_suffix('%').ok_or(ParseError::OutOfRange)?;
    let p = Decimal::parse(num).map_err(|_| ParseError::OutOfRange)?;
    let lo = Decimal::parse("0.1").expect("literal");
    let hi = Decimal::parse("20").expect("literal");
    // 0.1 <= p <= 20
    if decimal_lt(&p, &lo) || decimal_lt(&hi, &p) {
        return Err(ParseError::OutOfRange);
    }
    Ok(p.to_plain_string())
}

/// Parse a percent value with an upper bound (e.g. slippage ≤ 5); strips `%`.
pub fn parse_percent_max(value: &str, max: &str) -> Result<String, ParseError> {
    let num = value.strip_suffix('%').ok_or(ParseError::InvalidNumber)?;
    let p = Decimal::parse(num).map_err(|_| ParseError::InvalidNumber)?;
    let cap = Decimal::parse(max).expect("literal");
    if !p.le(&cap) {
        return Err(ParseError::OutOfRange);
    }
    Ok(p.to_plain_string())
}

/// Parse a non-negative percent value (e.g. APY); strips `%`. `Decimal` is always
/// non-negative, so this only rejects malformed numbers.
pub fn parse_percent_nonneg(value: &str) -> Result<String, ParseError> {
    let num = value.strip_suffix('%').ok_or(ParseError::InvalidNumber)?;
    let p = Decimal::parse(num).map_err(|_| ParseError::InvalidNumber)?;
    Ok(p.to_plain_string())
}

/// Parse odds: an absolute decimal in `[0, 1]`.
pub fn parse_odds(value: &str) -> Result<String, ParseError> {
    let p = Decimal::parse(value).map_err(|_| ParseError::InvalidNumber)?;
    let one = Decimal::parse("1").expect("literal");
    if !p.le(&one) {
        return Err(ParseError::OutOfRange);
    }
    Ok(p.to_plain_string())
}

/// Parse the Prediction `outcome` field `<OUTCOME> @<odds>` (feedback !21bc5915).
/// Exactly one `@`: the head is an outcome keyword, the tail an odds decimal in
/// `[0,1]`. Zero or extra `@` → the field is malformed.
pub fn parse_outcome_odds(value: &str) -> Result<(Outcome, String), ParseError> {
    let (outcome_str, odds_str) = value.split_once('@').ok_or(ParseError::IllegalKeyword)?;
    if odds_str.contains('@') {
        return Err(ParseError::ForbiddenContent);
    }
    let outcome = parse_outcome(outcome_str.trim())?;
    let odds = parse_odds(odds_str.trim())?;
    Ok((outcome, odds))
}

/// Parse leverage: a positive integer `×`.
pub fn parse_leverage(value: &str) -> Result<u32, ParseError> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseError::OutOfRange);
    }
    let n: u32 = value.parse().map_err(|_| ParseError::OutOfRange)?;
    if n == 0 {
        return Err(ParseError::OutOfRange);
    }
    Ok(n)
}

/// Parse TTL `Nmin | Nh | Nd` → seconds in `300 ..= 604800` (5min..=7d).
pub fn parse_ttl(value: &str) -> Result<u64, ParseError> {
    let (num, mult) = if let Some(n) = value.strip_suffix("min") {
        (n, 60u64)
    } else if let Some(n) = value.strip_suffix('h') {
        (n, 3_600u64)
    } else if let Some(n) = value.strip_suffix('d') {
        (n, 86_400u64)
    } else {
        return Err(ParseError::OutOfRange); // unknown unit
    };
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseError::OutOfRange);
    }
    let n: u64 = num.parse().map_err(|_| ParseError::OutOfRange)?;
    let secs = n.checked_mul(mult).ok_or(ParseError::OutOfRange)?;
    if !(300..=604_800).contains(&secs) {
        return Err(ParseError::OutOfRange);
    }
    Ok(secs)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Validate a proleptic-Gregorian `YYYY-MM-DD` (leap-aware, no system clock);
/// returns the zero-padded canonical form.
pub fn parse_date(value: &str) -> Result<String, ParseError> {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 {
        return Err(ParseError::InvalidDate);
    }
    let (ys, ms, ds) = (parts[0], parts[1], parts[2]);
    if ys.len() != 4 || !ys.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseError::InvalidDate); // missing / malformed year
    }
    let year: i64 = ys.parse().map_err(|_| ParseError::InvalidDate)?;
    let month: u32 = parse_date_component(ms)?;
    let day: u32 = parse_date_component(ds)?;
    if !(1..=12).contains(&month) {
        return Err(ParseError::InvalidDate);
    }
    let dim = days_in_month(year, month);
    if day < 1 || day > dim {
        return Err(ParseError::InvalidDate);
    }
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

fn parse_date_component(s: &str) -> Result<u32, ParseError> {
    if s.is_empty() || s.len() > 2 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseError::InvalidDate);
    }
    s.parse().map_err(|_| ParseError::InvalidDate)
}

// ── Keyword whitelists (IllegalKeyword on any non-canonical variant) ──────────

pub fn parse_side(value: &str) -> Result<Side, ParseError> {
    match value {
        "BUY" => Ok(Side::Buy),
        "SELL" => Ok(Side::Sell),
        _ => Err(ParseError::IllegalKeyword),
    }
}

/// Option side accepts the canonical + bilingual variants (FR-2.5).
pub fn parse_option_side(value: &str) -> Result<Side, ParseError> {
    match value {
        "BUY" | "Buy" | "买入" => Ok(Side::Buy),
        "SELL" | "Sell" | "卖出" => Ok(Side::Sell),
        _ => Err(ParseError::IllegalKeyword),
    }
}

pub fn parse_direction(value: &str) -> Result<Direction, ParseError> {
    match value {
        "LONG" => Ok(Direction::Long),
        "SHORT" => Ok(Direction::Short),
        _ => Err(ParseError::IllegalKeyword),
    }
}

pub fn parse_order_type(value: &str) -> Result<OrderType, ParseError> {
    match value {
        "market" => Ok(OrderType::Market),
        "limit" => Ok(OrderType::Limit),
        _ => Err(ParseError::IllegalKeyword),
    }
}

pub fn parse_margin_mode(value: &str) -> Result<MarginMode, ParseError> {
    match value {
        "cross" => Ok(MarginMode::Cross),
        "isolated" => Ok(MarginMode::Isolated),
        _ => Err(ParseError::IllegalKeyword),
    }
}

pub fn parse_outcome(value: &str) -> Result<Outcome, ParseError> {
    match value {
        "YES" => Ok(Outcome::Yes),
        "NO" => Ok(Outcome::No),
        "UP" => Ok(Outcome::Up),
        "DOWN" => Ok(Outcome::Down),
        _ => Err(ParseError::IllegalKeyword),
    }
}

pub fn parse_option_type(value: &str) -> Result<OptionType, ParseError> {
    match value {
        "Call" => Ok(OptionType::Call),
        "Put" => Ok(OptionType::Put),
        _ => Err(ParseError::IllegalKeyword),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_boundaries() {
        assert_eq!(parse_position("0.1%").unwrap(), "0.1");
        assert_eq!(parse_position("20%").unwrap(), "20");
        assert_eq!(parse_position("5%").unwrap(), "5");
        assert_eq!(parse_position("0%"), Err(ParseError::OutOfRange));
        assert_eq!(parse_position("20.1%"), Err(ParseError::OutOfRange));
        assert_eq!(parse_position("5-10%"), Err(ParseError::OutOfRange)); // range
        assert_eq!(parse_position("5"), Err(ParseError::OutOfRange)); // missing %
    }

    #[test]
    fn ttl_units_and_bounds() {
        assert_eq!(parse_ttl("5min").unwrap(), 300);
        assert_eq!(parse_ttl("7d").unwrap(), 604_800);
        assert_eq!(parse_ttl("1h").unwrap(), 3_600);
        assert_eq!(parse_ttl("24h").unwrap(), 86_400);
        assert_eq!(parse_ttl("4min"), Err(ParseError::OutOfRange)); // below 5min
        assert_eq!(parse_ttl("8d"), Err(ParseError::OutOfRange)); // above 7d
        assert_eq!(parse_ttl("30s"), Err(ParseError::OutOfRange)); // unknown unit
    }

    #[test]
    fn range_and_decimal() {
        assert_eq!(
            parse_range("60000-65000").unwrap(),
            PriceRange {
                lo: "60000".into(),
                hi: "65000".into()
            }
        );
        assert_eq!(parse_range("65000-60000"), Err(ParseError::OutOfRange)); // inverted
        assert_eq!(parse_decimal("1e3"), Err(ParseError::InvalidNumber)); // sci-notation
        assert_eq!(parse_decimal("1,000"), Err(ParseError::InvalidNumber)); // thousands sep
        assert_eq!(parse_decimal("5%"), Err(ParseError::InvalidNumber)); // %-price
    }

    #[test]
    fn calendar_dates() {
        assert_eq!(parse_date("2024-02-29").unwrap(), "2024-02-29"); // leap
        assert_eq!(parse_date("2025-02-29"), Err(ParseError::InvalidDate)); // non-leap
        assert_eq!(parse_date("2025-13-01"), Err(ParseError::InvalidDate)); // month
        assert_eq!(parse_date("12-31"), Err(ParseError::InvalidDate)); // missing year
        assert_eq!(parse_date("2025-04-31"), Err(ParseError::InvalidDate)); // 30-day month
    }

    #[test]
    fn keyword_whitelists() {
        assert_eq!(parse_direction("LONG").unwrap(), Direction::Long);
        assert_eq!(parse_direction("做多"), Err(ParseError::IllegalKeyword));
        assert_eq!(parse_direction("L"), Err(ParseError::IllegalKeyword));
        assert_eq!(parse_option_side("买入").unwrap(), Side::Buy);
        assert_eq!(parse_side("买入"), Err(ParseError::IllegalKeyword)); // spot side is canonical-only
    }

    /// FR-2.4: the Prediction outcome+odds field `<OUTCOME> @<odds>` (feedback !21bc5915).
    #[test]
    fn outcome_odds_field() {
        assert_eq!(
            parse_outcome_odds("YES @0.62").unwrap(),
            (Outcome::Yes, "0.62".to_string())
        );
        assert_eq!(
            parse_outcome_odds("UP @0.4").unwrap(),
            (Outcome::Up, "0.4".to_string())
        );
        // missing `@` odds separator.
        assert_eq!(parse_outcome_odds("YES"), Err(ParseError::IllegalKeyword));
        // out-of-range odds.
        assert_eq!(parse_outcome_odds("YES @1.5"), Err(ParseError::OutOfRange));
        // illegal outcome keyword.
        assert_eq!(
            parse_outcome_odds("MAYBE @0.5"),
            Err(ParseError::IllegalKeyword)
        );
        // extra `@`.
        assert_eq!(
            parse_outcome_odds("YES @0.5@0.6"),
            Err(ParseError::ForbiddenContent)
        );
    }

    #[test]
    fn forbidden_scan() {
        assert!(contains_forbidden("【现货信号】市场:BTC https://x.io"));
        assert!(contains_forbidden("gm 🚀"));
        // `@` is no longer globally forbidden — it is field-scoped (Prediction odds).
        assert!(!contains_forbidden("@alpha"));
        assert!(!contains_forbidden("【现货信号】市场:BTC/USDT|方向:BUY"));
    }

    /// SR-2 regression: the previously-missed symbol blocks are still flagged as
    /// forbidden content — letterlike (™ U+2122), arrows (← U+2190), misc
    /// technical (⌚ U+231A / ⏰ U+23F0), geometric shapes (■ U+25A0 / ● U+25CF).
    #[test]
    fn forbidden_scan_extended_emoji_blocks() {
        for s in ["a™b", "up ←", "⌚ time", "alarm ⏰", "box ■", "dot ●"] {
            assert!(contains_forbidden(s), "expected forbidden: {s:?}");
        }
        // The canonical field grammar (CJK labels, full-width brackets, ASCII,
        // '/', '-', '%', ':') is still clean — no false positives.
        assert!(!contains_forbidden(
            "【期权信号】合约代码:BTC-251231-60000-C|方向:买入|类型:Call"
        ));
    }

    /// M-3: leverage must be a strictly-positive integer.
    #[test]
    fn leverage_rejects_zero_and_non_integer() {
        assert_eq!(parse_leverage("10").unwrap(), 10);
        assert_eq!(parse_leverage("1").unwrap(), 1);
        assert_eq!(parse_leverage("0"), Err(ParseError::OutOfRange)); // zero
        assert_eq!(parse_leverage("10.5"), Err(ParseError::OutOfRange)); // non-integer
        assert_eq!(parse_leverage("-5"), Err(ParseError::OutOfRange)); // signed
        assert_eq!(parse_leverage(""), Err(ParseError::OutOfRange)); // empty
        assert_eq!(parse_leverage("x"), Err(ParseError::OutOfRange)); // non-numeric
    }

    /// M-6: the closed keyword whitelists reject every non-canonical variant.
    #[test]
    fn keyword_whitelists_reject_non_canonical() {
        assert_eq!(parse_order_type("market").unwrap(), OrderType::Market);
        assert_eq!(parse_order_type("limit").unwrap(), OrderType::Limit);
        assert_eq!(parse_order_type("MARKET"), Err(ParseError::IllegalKeyword));
        assert_eq!(parse_order_type("stop"), Err(ParseError::IllegalKeyword));
        assert_eq!(parse_margin_mode("cross").unwrap(), MarginMode::Cross);
        assert_eq!(parse_margin_mode("isolated").unwrap(), MarginMode::Isolated);
        assert_eq!(parse_margin_mode("CROSS"), Err(ParseError::IllegalKeyword));
        assert_eq!(parse_margin_mode("full"), Err(ParseError::IllegalKeyword));
        assert_eq!(parse_option_type("Call").unwrap(), OptionType::Call);
        assert_eq!(parse_option_type("Put").unwrap(), OptionType::Put);
        assert_eq!(parse_option_type("call"), Err(ParseError::IllegalKeyword));
        assert_eq!(parse_option_type("CALL"), Err(ParseError::IllegalKeyword));
    }

    /// FR-2 (feedback !668dedbf): out-of-order fields are rejected even when every
    /// label is individually valid for the class.
    #[test]
    fn fixed_order_rejects_reorder_and_missing() {
        let raw = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        // canonical order parses.
        assert!(FieldMap::build(
            AssetClass::Spot,
            Language::En,
            &raw(&[
                ("market", "BTC/USDT"),
                ("symbol", "BTC"),
                ("side", "BUY"),
                ("price", "60000-65000"),
                ("position", "5%"),
                ("ttl", "1h"),
            ])
        )
        .is_ok());
        // symbol before market (reordered) → FieldCountError.
        assert_eq!(
            FieldMap::build(
                AssetClass::Spot,
                Language::En,
                &raw(&[
                    ("symbol", "BTC"),
                    ("market", "BTC/USDT"),
                    ("side", "BUY"),
                    ("price", "60000-65000"),
                    ("position", "5%"),
                    ("ttl", "1h"),
                ])
            )
            .unwrap_err(),
            ParseError::FieldCountError
        );
        // missing required `side` → FieldCountError.
        assert_eq!(
            FieldMap::build(
                AssetClass::Spot,
                Language::En,
                &raw(&[
                    ("market", "BTC/USDT"),
                    ("symbol", "BTC"),
                    ("price", "60000-65000"),
                    ("position", "5%"),
                    ("ttl", "1h"),
                ])
            )
            .unwrap_err(),
            ParseError::FieldCountError
        );
        // `@` outside the Prediction outcome field → ForbiddenContent.
        assert_eq!(
            FieldMap::build(
                AssetClass::Spot,
                Language::En,
                &raw(&[("market", "@evil"), ("symbol", "BTC")])
            )
            .unwrap_err(),
            ParseError::ForbiddenContent
        );
    }
}
