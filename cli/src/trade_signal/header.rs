//! FR-2 steps 2-3: exact-match one of the 10 whitelist headers (5 classes × 2
//! languages) → `(AssetClass, Language)`, and return the remainder after `】`.
//!
//! No whitespace/prefix is tolerated before the header, and the header must use
//! the full-width brackets `【…】` (U+3010/U+3011). A half-width `[` header, an
//! unknown header, or a whitespace-preceded header → [`ParseError::UnknownHeader`].
//!
//! The whitelist is the **full V1.1 titles only** (feedback !dd1b3502): the
//! self-authored short headers (`【现货】` / `【SPOT】` …) are no longer accepted,
//! to keep the accepted protocol surface exactly the spec's 10 strings.

use crate::asset_class::AssetClass;

use super::error::ParseError;
use super::Language;

/// The 10-item header whitelist: `(header_literal, asset_class, language)`.
///
/// The authoritative V1.1 full titles (feedback !dd1b3502): the Chinese set uses
/// the `…信号` suffix (DeFi keeps its Latin brand + a space), the English set uses
/// the `… Signal` suffix; all use full-width brackets `【】`.
const HEADERS: &[(&str, AssetClass, Language)] = &[
    ("【现货信号】", AssetClass::Spot, Language::Zh),
    ("【Spot Signal】", AssetClass::Spot, Language::En),
    ("【合约信号】", AssetClass::Perp, Language::Zh),
    ("【Futures Signal】", AssetClass::Perp, Language::En),
    ("【预测市场信号】", AssetClass::Prediction, Language::Zh),
    (
        "【Prediction Signal】",
        AssetClass::Prediction,
        Language::En,
    ),
    ("【期权信号】", AssetClass::Option, Language::Zh),
    ("【Options Signal】", AssetClass::Option, Language::En),
    ("【DeFi 信号】", AssetClass::Defi, Language::Zh),
    ("【DeFi Signal】", AssetClass::Defi, Language::En),
];

/// Match the leading header exactly and return `(class, language, remainder)`.
pub fn parse_header(input: &str) -> Result<(AssetClass, Language, &str), ParseError> {
    for (literal, class, language) in HEADERS {
        if let Some(remainder) = input.strip_prefix(*literal) {
            return Ok((*class, *language, remainder));
        }
    }
    Err(ParseError::UnknownHeader)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_every_whitelist_header() {
        assert_eq!(
            parse_header("【现货信号】市场:BTC/USDT").unwrap(),
            (AssetClass::Spot, Language::Zh, "市场:BTC/USDT")
        );
        assert_eq!(
            parse_header("【Futures Signal】pair:ETH-PERP").unwrap(),
            (AssetClass::Perp, Language::En, "pair:ETH-PERP")
        );
        assert_eq!(
            parse_header("【预测市场信号】").unwrap().0,
            AssetClass::Prediction
        );
        assert_eq!(parse_header("【Options Signal】").unwrap().1, Language::En);
        assert_eq!(parse_header("【DeFi 信号】").unwrap().0, AssetClass::Defi);
        assert_eq!(parse_header("【DeFi Signal】").unwrap().0, AssetClass::Defi);
    }

    #[test]
    fn rejects_unknown_space_half_width_and_old_short_headers() {
        assert_eq!(parse_header("【unknown】x"), Err(ParseError::UnknownHeader));
        assert_eq!(
            parse_header(" 【现货信号】x"),
            Err(ParseError::UnknownHeader)
        ); // leading space
        assert_eq!(parse_header("[现货信号]x"), Err(ParseError::UnknownHeader)); // half-width
        assert_eq!(parse_header("现货信号|x"), Err(ParseError::UnknownHeader)); // no brackets
                                                                                // The self-authored short headers are no longer part of the protocol surface.
        assert_eq!(parse_header("【现货】x"), Err(ParseError::UnknownHeader));
        assert_eq!(parse_header("【SPOT】x"), Err(ParseError::UnknownHeader));
    }
}
