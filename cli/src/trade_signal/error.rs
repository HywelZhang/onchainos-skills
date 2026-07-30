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
/// contract. Per V1.1/TD review alignment (feedback !92ea45d6) they are stable
/// **snake_case** codes, and the three envelope faults are kept as distinct,
/// decidable errors (`invalid_schema_version` / `invalid_delivery_id` /
/// `invalid_signal_time`) rather than collapsed onto one opaque code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Empty input string.
    EmptyInput,
    /// First char is neither `{` nor `【`, or there is leading whitespace.
    UnsupportedFormat,
    /// Envelope JSON is malformed / has an unknown or missing field.
    InvalidEnvelope,
    /// `schemaVersion` is not the required V2 value.
    InvalidSchemaVersion,
    /// `deliveryId` is absent / too long / has an illegal character.
    InvalidDeliveryId,
    /// `signalTime` is `0` (must be a non-zero epoch-ms stamp).
    InvalidSignalTime,
    /// More than 200 Unicode chars.
    TooLong,
    /// Contains a newline (single-line only).
    MultiLine,
    /// Header not in the 10-item whitelist / preceded by whitespace / half-width `[`.
    UnknownHeader,
    /// Wrong field count/order for the asset class (missing required label / extra / reordered / duplicate).
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
    /// Emoji, link, out-of-place `@`-mention, extra field, analysis prose — content beyond the field grammar.
    ForbiddenContent,
}

impl ParseError {
    /// Stable machine code string (external contract). MUST NOT change after ship.
    /// Stable snake_case per V1.1/TD alignment (feedback !92ea45d6).
    pub fn code(&self) -> &'static str {
        match self {
            ParseError::EmptyInput => "empty_input",
            ParseError::UnsupportedFormat => "unsupported_format",
            ParseError::InvalidEnvelope => "invalid_envelope",
            ParseError::InvalidSchemaVersion => "invalid_schema_version",
            ParseError::InvalidDeliveryId => "invalid_delivery_id",
            ParseError::InvalidSignalTime => "invalid_signal_time",
            ParseError::TooLong => "too_long",
            ParseError::MultiLine => "multi_line",
            ParseError::UnknownHeader => "unknown_header",
            ParseError::FieldCountError => "field_count_error",
            ParseError::EmptyField => "empty_field",
            ParseError::LanguageMix => "language_mix",
            ParseError::IllegalKeyword => "illegal_keyword",
            ParseError::InvalidNumber => "invalid_number",
            ParseError::OutOfRange => "out_of_range",
            ParseError::InvalidDate => "invalid_date",
            ParseError::DirectionConstraint => "direction_constraint",
            ParseError::OptionFieldMismatch => "option_field_mismatch",
            ParseError::ForbiddenContent => "forbidden_content",
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
            ParseError::InvalidSchemaVersion => Some("schemaVersion"),
            ParseError::InvalidDeliveryId => Some("deliveryId"),
            ParseError::InvalidSignalTime => Some("signalTime"),
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
            ParseError::InvalidSchemaVersion => "unsupported schema version",
            ParseError::InvalidDeliveryId => "invalid delivery id",
            ParseError::InvalidSignalTime => "invalid signal time",
            ParseError::TooLong => "input exceeds the 200 character limit",
            ParseError::MultiLine => "input must be a single line",
            ParseError::UnknownHeader => "unrecognized signal header",
            ParseError::FieldCountError => "wrong number or order of fields for the asset class",
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

    /// Every variant's `code()` is the stable snake_case external contract, and the
    /// three envelope faults stay distinct (feedback !92ea45d6).
    #[test]
    fn code_matches_external_contract() {
        assert_eq!(ParseError::EmptyInput.code(), "empty_input");
        assert_eq!(ParseError::UnsupportedFormat.code(), "unsupported_format");
        assert_eq!(ParseError::InvalidEnvelope.code(), "invalid_envelope");
        assert_eq!(
            ParseError::InvalidSchemaVersion.code(),
            "invalid_schema_version"
        );
        assert_eq!(ParseError::InvalidDeliveryId.code(), "invalid_delivery_id");
        assert_eq!(ParseError::InvalidSignalTime.code(), "invalid_signal_time");
        assert_eq!(ParseError::TooLong.code(), "too_long");
        assert_eq!(ParseError::MultiLine.code(), "multi_line");
        assert_eq!(ParseError::UnknownHeader.code(), "unknown_header");
        assert_eq!(ParseError::FieldCountError.code(), "field_count_error");
        assert_eq!(ParseError::EmptyField.code(), "empty_field");
        assert_eq!(ParseError::LanguageMix.code(), "language_mix");
        assert_eq!(ParseError::IllegalKeyword.code(), "illegal_keyword");
        assert_eq!(ParseError::InvalidNumber.code(), "invalid_number");
        assert_eq!(ParseError::OutOfRange.code(), "out_of_range");
        assert_eq!(ParseError::InvalidDate.code(), "invalid_date");
        assert_eq!(
            ParseError::DirectionConstraint.code(),
            "direction_constraint"
        );
        assert_eq!(
            ParseError::OptionFieldMismatch.code(),
            "option_field_mismatch"
        );
        assert_eq!(ParseError::ForbiddenContent.code(), "forbidden_content");
    }

    /// The split envelope faults carry distinct, decidable field names.
    #[test]
    fn envelope_faults_have_distinct_fields() {
        assert_eq!(
            ParseError::InvalidSchemaVersion.field(),
            Some("schemaVersion")
        );
        assert_eq!(ParseError::InvalidDeliveryId.field(), Some("deliveryId"));
        assert_eq!(ParseError::InvalidSignalTime.field(), Some("signalTime"));
    }

    #[test]
    fn display_writes_value_free_message() {
        assert_eq!(
            format!("{}", ParseError::OutOfRange),
            "value is out of the allowed range"
        );
    }
}
