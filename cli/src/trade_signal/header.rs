//! FR-2 steps 2-3: exact-match one of the 10 whitelist headers (5 classes × 2
//! languages) → `(AssetClass, Language)`, and return the remainder after `】`.
//!
//! No whitespace/prefix is tolerated before the header, and the header must use
//! the full-width brackets `【…】` (U+3010/U+3011). A half-width `[` header, an
//! unknown header, or a whitespace-preceded header → [`ParseError::UnknownHeader`].

use crate::asset_class::AssetClass;

use super::error::ParseError;
use super::Language;

/// The 10-item header whitelist: `(header_literal, asset_class, language)`.
const HEADERS: &[(&str, AssetClass, Language)] = &[
    ("【现货】", AssetClass::Spot, Language::Zh),
    ("【SPOT】", AssetClass::Spot, Language::En),
    ("【合约】", AssetClass::Perp, Language::Zh),
    ("【PERP】", AssetClass::Perp, Language::En),
    ("【预测】", AssetClass::Prediction, Language::Zh),
    ("【PREDICTION】", AssetClass::Prediction, Language::En),
    ("【期权】", AssetClass::Option, Language::Zh),
    ("【OPTION】", AssetClass::Option, Language::En),
    ("【理财】", AssetClass::Defi, Language::Zh),
    ("【DEFI】", AssetClass::Defi, Language::En),
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
            parse_header("【现货】市场:BTC/USDT").unwrap(),
            (AssetClass::Spot, Language::Zh, "市场:BTC/USDT")
        );
        assert_eq!(
            parse_header("【PERP】pair:ETH-PERP").unwrap(),
            (AssetClass::Perp, Language::En, "pair:ETH-PERP")
        );
        assert_eq!(parse_header("【预测】").unwrap().0, AssetClass::Prediction);
        assert_eq!(parse_header("【OPTION】").unwrap().1, Language::En);
        assert_eq!(parse_header("【理财】").unwrap().0, AssetClass::Defi);
    }

    #[test]
    fn rejects_unknown_space_and_half_width() {
        assert_eq!(parse_header("【unknown】x"), Err(ParseError::UnknownHeader));
        assert_eq!(parse_header(" 【现货】x"), Err(ParseError::UnknownHeader)); // leading space
        assert_eq!(parse_header("[现货]x"), Err(ParseError::UnknownHeader)); // half-width
        assert_eq!(parse_header("现货|x"), Err(ParseError::UnknownHeader)); // no brackets
    }
}
