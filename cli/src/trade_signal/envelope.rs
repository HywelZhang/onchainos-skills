//! FR-3: V2 wire envelope. Validate `schemaVersion` / `deliveryId` / `signalTime`
//! FIRST, then delegate the inner `signalText` to [`super::parse_signal_text`].
//!
//! Serde naming + unknown-field handling mirror the existing autotrade
//! `schema.rs` (camelCase, reject unknown fields). The `deliveryId` default
//! (`sha256(signalText)[:16]`) is computed by the send-side caller (D-4); this
//! parser only checks presence + format.

use serde::Deserialize;

use super::error::ParseError;
use super::{parse_signal_text, ParsedSignal};

/// The required schema version for a V2 text envelope.
const V2_SCHEMA_VERSION: u32 = 2;
/// `deliveryId` length cap (mirrors autotrade `schema.rs` `MAX_DELIVERY_ID`).
const MAX_DELIVERY_ID: usize = 64;

/// The V2 wire envelope. `deny_unknown_fields` mirrors the repo convention: a
/// newer schema may reinterpret fields, so unexpected keys are rejected.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct V2Envelope {
    pub schema_version: u32,
    pub delivery_id: String,
    /// Non-zero epoch milliseconds.
    pub signal_time: u64,
    pub signal_text: String,
}

/// `deliveryId` charset: `[A-Za-z0-9_-]`, length `1..=64` (reuses the existing
/// security-validation rules from the autotrade schema).
fn delivery_id_valid(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_DELIVERY_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Validate a V2 envelope JSON string then parse its `signalText`.
pub fn parse_envelope(input: &str) -> Result<ParsedSignal, ParseError> {
    let env: V2Envelope = serde_json::from_str(input).map_err(|_| ParseError::InvalidEnvelope)?;

    if env.schema_version != V2_SCHEMA_VERSION {
        return Err(ParseError::InvalidEnvelope);
    }
    if env.signal_time == 0 {
        return Err(ParseError::InvalidEnvelope);
    }
    if !delivery_id_valid(&env.delivery_id) {
        return Err(ParseError::InvalidEnvelope);
    }

    parse_signal_text(&env.signal_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_TEXT: &str =
        "【SPOT】market:BTC/USDT|symbol:BTC|side:BUY|price:60000-65000|position:5%|ttl:1h";

    fn envelope(schema: u32, delivery: &str, time: u64) -> String {
        format!(
            "{{\"schemaVersion\":{schema},\"deliveryId\":\"{delivery}\",\"signalTime\":{time},\"signalText\":\"{VALID_TEXT}\"}}"
        )
    }

    #[test]
    fn valid_envelope_delegates_to_text_parse() {
        let json = envelope(2, "abc123", 1_700_000_000_000);
        let parsed = parse_envelope(&json).unwrap();
        assert_eq!(parsed.asset_class.as_str(), "spot");
    }

    #[test]
    fn rejects_bad_envelope_fields() {
        assert_eq!(
            parse_envelope(&envelope(1, "abc123", 1)),
            Err(ParseError::InvalidEnvelope) // schemaVersion != 2
        );
        assert_eq!(
            parse_envelope(&envelope(2, "abc123", 0)),
            Err(ParseError::InvalidEnvelope) // signalTime == 0
        );
        assert_eq!(
            parse_envelope(&envelope(2, "bad id", 1)),
            Err(ParseError::InvalidEnvelope) // illegal deliveryId
        );
        assert_eq!(parse_envelope("not json"), Err(ParseError::InvalidEnvelope));
    }
}
