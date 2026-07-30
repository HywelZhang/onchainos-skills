//! FR-3: V2 wire envelope. Validate `schemaVersion` / `deliveryId` / `signalTime`
//! FIRST (each mapped to its own decidable error), then delegate the inner
//! `signalText` to [`super::parse_signal_text`].
//!
//! Per V1.1/TD review alignment (feedback !42eef591) the `deliveryId` check REUSES
//! the existing autotrade schema validator
//! ([`crate::commands::agent_commerce::task::common::autotrade::schema::check_delivery_id`])
//! rather than maintaining a second copy of the length/charset rules — so the
//! rule cannot drift. The envelope only validates protocol fields and delegates.

use serde::Deserialize;

use crate::commands::agent_commerce::task::common::autotrade::schema::check_delivery_id;

use super::error::ParseError;
use super::{parse_signal_text, ParsedSignal};

/// The required schema version for a V2 text envelope.
const V2_SCHEMA_VERSION: u32 = 2;

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

/// Validate a V2 envelope JSON string then parse its `signalText`. Each protocol
/// field maps to its own fine-grained error (feedback !92ea45d6 / !42eef591).
pub fn parse_envelope(input: &str) -> Result<ParsedSignal, ParseError> {
    let env: V2Envelope = serde_json::from_str(input).map_err(|_| ParseError::InvalidEnvelope)?;

    if env.schema_version != V2_SCHEMA_VERSION {
        return Err(ParseError::InvalidSchemaVersion);
    }
    // Reuse the shared autotrade deliveryId validator (single source of truth).
    check_delivery_id(&env.delivery_id).map_err(|_| ParseError::InvalidDeliveryId)?;
    if env.signal_time == 0 {
        return Err(ParseError::InvalidSignalTime);
    }

    parse_signal_text(&env.signal_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_TEXT: &str =
        "【Spot Signal】market:BTC/USDT|symbol:BTC|side:BUY|price:60000-65000|position:5%|ttl:1h";

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
    fn rejects_bad_envelope_fields_with_distinct_codes() {
        // schemaVersion != 2 → invalid_schema_version.
        assert_eq!(
            parse_envelope(&envelope(1, "abc123", 1))
                .unwrap_err()
                .code(),
            "invalid_schema_version"
        );
        // signalTime == 0 → invalid_signal_time.
        assert_eq!(
            parse_envelope(&envelope(2, "abc123", 0))
                .unwrap_err()
                .code(),
            "invalid_signal_time"
        );
        // illegal deliveryId (space) → invalid_delivery_id.
        assert_eq!(
            parse_envelope(&envelope(2, "bad id", 1))
                .unwrap_err()
                .code(),
            "invalid_delivery_id"
        );
        // malformed JSON → invalid_envelope.
        assert_eq!(
            parse_envelope("not json").unwrap_err().code(),
            "invalid_envelope"
        );
    }
}
