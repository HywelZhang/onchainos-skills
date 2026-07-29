//! Closed-set parser error type (repo convention: hand-rolled `Display` +
//! `std::error::Error`, NOT `thiserror` — see architecture DR-B / NFR-2 "no new
//! deps"; mirrors `AmountError` / `StrategyError`).
//!
//! SR-3 hard requirement: `code()` / `field()` / `message()` carry ONLY a stable
//! category + stable field name — NEVER the raw signal text, `tokenAddr`,
//! `event`, `contractCode`, or any ASP-authored free text. Every return here is a
//! value-free `&'static str`, which makes an input leak structurally impossible.

/// The stable closed-set of parse/validation/envelope failures.
///
/// The `code()` strings (not the Rust variant names) are the external stability
/// contract (NFR-4) and are enumerated verbatim in `cli_command_spec.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Empty input string.
    EmptyInput,
    /// First char is neither `{` nor `【`, or there is leading whitespace.
    UnsupportedFormat,
    /// `schemaVersion` != 2, `signalTime` == 0, or an illegal/absent `deliveryId`.
    InvalidEnvelope,
    /// More than 200 Unicode chars.
    TooLong,
    /// Contains a newline (single-line only).
    MultiLine,
    /// Header not in the 10-item whitelist / preceded by whitespace / half-width `[`.
    UnknownHeader,
    /// Wrong field count for the asset class (missing required label / duplicate).
    FieldCountError,
    /// Any field empty after trim.
    EmptyField,
    /// Mixed 中/英 labels in one signal.
    LanguageMix,
    /// Non-whitelist token variant (e.g. `S`/`L`, `做多`, `买进`).
    IllegalKeyword,
    /// Sci-notation, thousands separator, %-price, or otherwise non-decimal number.
    InvalidNumber,
    /// Value out of the allowed range (position, TTL, odds, slippage, leverage, inverted range).
    OutOfRange,
    /// Missing year / nonexistent / malformed `YYYY-MM-DD`.
    InvalidDate,
    /// LONG/SHORT SL or TP on the wrong side; duplicate SL; 0 or 4 TP; TP numbering gap.
    DirectionConstraint,
    /// `contractCode` inconsistent with Call/Put, strike, or expiry.
    OptionFieldMismatch,
    /// Emoji, link, @mention, extra field, analysis prose — content beyond the field grammar.
    ForbiddenContent,
}

impl ParseError {
    /// Stable machine code string (NFR-4 contract). MUST NOT change after ship.
    pub fn code(&self) -> &'static str {
        match self {
            ParseError::EmptyInput => "EmptyInput",
            ParseError::UnsupportedFormat => "UnsupportedFormat",
            ParseError::InvalidEnvelope => "InvalidEnvelope",
            ParseError::TooLong => "TooLong",
            ParseError::MultiLine => "MultiLine",
            ParseError::UnknownHeader => "UnknownHeader",
            ParseError::FieldCountError => "FieldCountError",
            ParseError::EmptyField => "EmptyField",
            ParseError::LanguageMix => "LanguageMix",
            ParseError::IllegalKeyword => "IllegalKeyword",
            ParseError::InvalidNumber => "InvalidNumber",
            ParseError::OutOfRange => "OutOfRange",
            ParseError::InvalidDate => "InvalidDate",
            ParseError::DirectionConstraint => "DirectionConstraint",
            ParseError::OptionFieldMismatch => "OptionFieldMismatch",
            ParseError::ForbiddenContent => "ForbiddenContent",
        }
    }

    /// Stable field name for the offending parameter, or `None`. NEVER the value.
    pub fn field(&self) -> Option<&'static str> {
        match self {
            ParseError::EmptyInput
            | ParseError::UnsupportedFormat
            | ParseError::TooLong
            | ParseError::MultiLine
            | ParseError::UnknownHeader
            | ParseError::EmptyField
            | ParseError::LanguageMix
            | ParseError::ForbiddenContent
            | ParseError::FieldCountError => None,
            ParseError::InvalidEnvelope => Some("envelope"),
            ParseError::IllegalKeyword => Some("keyword"),
            ParseError::InvalidNumber => Some("number"),
            ParseError::OutOfRange => Some("range"),
            ParseError::InvalidDate => Some("date"),
            ParseError::DirectionConstraint => Some("takeProfit"),
            ParseError::OptionFieldMismatch => Some("contractCode"),
        }
    }

    /// Generic, value-free human message (SR-3 log-leak prevention).
    pub fn message(&self) -> &'static str {
        match self {
            ParseError::EmptyInput => "input is empty",
            ParseError::UnsupportedFormat => "unsupported input format",
            ParseError::InvalidEnvelope => "invalid v2 envelope",
            ParseError::TooLong => "input exceeds the 200 character limit",
            ParseError::MultiLine => "input must be a single line",
            ParseError::UnknownHeader => "unrecognized signal header",
            ParseError::FieldCountError => "wrong number of fields for the asset class",
            ParseError::EmptyField => "a field is empty after trimming",
            ParseError::LanguageMix => "mixed-language labels are not allowed",
            ParseError::IllegalKeyword => "unrecognized keyword variant",
            ParseError::InvalidNumber => "malformed number",
            ParseError::OutOfRange => "value is out of the allowed range",
            ParseError::InvalidDate => "invalid calendar date",
            ParseError::DirectionConstraint => {
                "stop-loss/take-profit direction constraint violated"
            }
            ParseError::OptionFieldMismatch => "contract code is inconsistent with its fields",
            ParseError::ForbiddenContent => "input contains content beyond the field grammar",
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant's `code()` equals the exact `errorCode` string in cli_command_spec.md.
    #[test]
    fn code_matches_external_contract() {
        assert_eq!(ParseError::EmptyInput.code(), "EmptyInput");
        assert_eq!(ParseError::UnsupportedFormat.code(), "UnsupportedFormat");
        assert_eq!(ParseError::InvalidEnvelope.code(), "InvalidEnvelope");
        assert_eq!(ParseError::TooLong.code(), "TooLong");
        assert_eq!(ParseError::MultiLine.code(), "MultiLine");
        assert_eq!(ParseError::UnknownHeader.code(), "UnknownHeader");
        assert_eq!(ParseError::FieldCountError.code(), "FieldCountError");
        assert_eq!(ParseError::EmptyField.code(), "EmptyField");
        assert_eq!(ParseError::LanguageMix.code(), "LanguageMix");
        assert_eq!(ParseError::IllegalKeyword.code(), "IllegalKeyword");
        assert_eq!(ParseError::InvalidNumber.code(), "InvalidNumber");
        assert_eq!(ParseError::OutOfRange.code(), "OutOfRange");
        assert_eq!(ParseError::InvalidDate.code(), "InvalidDate");
        assert_eq!(
            ParseError::DirectionConstraint.code(),
            "DirectionConstraint"
        );
        assert_eq!(
            ParseError::OptionFieldMismatch.code(),
            "OptionFieldMismatch"
        );
        assert_eq!(ParseError::ForbiddenContent.code(), "ForbiddenContent");
    }

    #[test]
    fn display_writes_value_free_message() {
        assert_eq!(
            format!("{}", ParseError::OutOfRange),
            "value is out of the allowed range"
        );
    }
}
