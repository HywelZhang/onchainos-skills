//! FR-2.2 spot parser. Supports the CEX-pair form and the on-chain-token form
//! (which requires both `tokenAddr` and `slippage`, slippage ≤ 5%). `orderType`
//! defaults to `market` when absent; `priceRange.lo < hi`.

use super::super::error::ParseError;
use super::super::fields::{self, FieldMap};
use super::super::{OrderType, SpotParams};

pub fn parse(fm: &mut FieldMap) -> Result<SpotParams, ParseError> {
    let market = fm.require(fields::ID_MARKET)?;
    let symbol = fm.require(fields::ID_SYMBOL)?;
    let side = fields::parse_side(&fm.require(fields::ID_SIDE)?)?;
    let price_range = fields::parse_range(&fm.require(fields::ID_PRICE)?)?;

    let order_type = match fm.take(fields::ID_ORDER_TYPE) {
        Some(v) => fields::parse_order_type(&v)?,
        None => OrderType::Market,
    };

    let token_addr = fm.take(fields::ID_TOKEN_ADDR);
    let slippage = match fm.take(fields::ID_SLIPPAGE) {
        Some(v) => Some(fields::parse_percent_max(&v, "5")?),
        None => None,
    };

    // On-chain form requires BOTH tokenAddr and slippage (SR-6).
    if token_addr.is_some() != slippage.is_some() {
        return Err(ParseError::FieldCountError);
    }

    Ok(SpotParams {
        market,
        symbol,
        side,
        price_range,
        order_type,
        token_addr,
        slippage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_class::AssetClass;
    use crate::trade_signal::Language;

    /// Build a spot `FieldMap` from raw en `(label, value)` pairs (en labels are
    /// identical to the canonical field ids, so this exercises the real path).
    fn spot_fm(pairs: &[(&str, &str)]) -> FieldMap {
        let raw: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        FieldMap::build(AssetClass::Spot, Language::En, &raw).expect("field map builds")
    }

    /// M-5 / SR-6: the on-chain form's slippage carries a hard 5% ceiling.
    #[test]
    fn slippage_above_five_percent_is_out_of_range() {
        let mut fm = spot_fm(&[
            ("market", "base"),
            ("symbol", "DEGEN"),
            ("side", "BUY"),
            ("price", "1-2"),
            ("tokenAddr", "0xabc"),
            ("slippage", "9%"),
        ]);
        assert_eq!(parse(&mut fm).unwrap_err(), ParseError::OutOfRange);
    }

    /// Control: slippage exactly at the 5% ceiling parses.
    #[test]
    fn slippage_at_ceiling_parses() {
        let mut fm = spot_fm(&[
            ("market", "base"),
            ("symbol", "DEGEN"),
            ("side", "BUY"),
            ("price", "1-2"),
            ("tokenAddr", "0xabc"),
            ("slippage", "5%"),
        ]);
        let out = parse(&mut fm).expect("parses");
        assert_eq!(out.slippage.as_deref(), Some("5"));
        assert_eq!(out.token_addr.as_deref(), Some("0xabc"));
    }

    /// M-4: the on-chain form requires BOTH `tokenAddr` and `slippage` (SR-6);
    /// presence of exactly one is a `FieldCountError`.
    #[test]
    fn token_addr_xor_slippage_is_field_count_error() {
        // tokenAddr without slippage.
        let mut fm = spot_fm(&[
            ("market", "base"),
            ("symbol", "DEGEN"),
            ("side", "BUY"),
            ("price", "1-2"),
            ("tokenAddr", "0xabc"),
        ]);
        assert_eq!(parse(&mut fm).unwrap_err(), ParseError::FieldCountError);

        // slippage without tokenAddr.
        let mut fm = spot_fm(&[
            ("market", "BTC/USDT"),
            ("symbol", "BTC"),
            ("side", "BUY"),
            ("price", "1-2"),
            ("slippage", "3%"),
        ]);
        assert_eq!(parse(&mut fm).unwrap_err(), ParseError::FieldCountError);
    }

    /// Control: the CEX form (neither key) parses and defaults `orderType`.
    #[test]
    fn cex_form_without_onchain_keys_parses() {
        let mut fm = spot_fm(&[
            ("market", "BTC/USDT"),
            ("symbol", "BTC"),
            ("side", "BUY"),
            ("price", "1-2"),
        ]);
        let out = parse(&mut fm).expect("parses");
        assert!(out.token_addr.is_none() && out.slippage.is_none());
        assert_eq!(out.order_type, OrderType::Market);
    }
}
