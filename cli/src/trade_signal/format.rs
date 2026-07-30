//! FR-1: input-format detection — an independent public entry point (FR-1.3),
//! deliberately NOT fused into [`super::parse_signal_text`].

use serde::Serialize;

/// Classification of a raw input string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum InputFormat {
    /// First char `{` — a V1 structured-JSON signal (classified only, never parsed here).
    V1JsonSchema,
    /// First char `【` (U+3010) — a V2 text signal.
    V2Text,
    /// Empty, leading whitespace, or any other first char.
    Unsupported,
}

/// FR-1.2 first-char rule — no leading-whitespace tolerance.
pub fn detect_format(input: &str) -> InputFormat {
    match input.chars().next() {
        Some('{') => InputFormat::V1JsonSchema,
        Some('【') => InputFormat::V2Text,
        _ => InputFormat::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_char_brace_is_v1() {
        assert_eq!(
            detect_format("{\"schemaVersion\":2}"),
            InputFormat::V1JsonSchema
        );
    }

    #[test]
    fn first_char_cjk_bracket_is_v2() {
        assert_eq!(
            detect_format("【\u{73b0}\u{8d27}】\u{5e02}\u{573a}:BTC/USDT"),
            InputFormat::V2Text
        );
    }

    #[test]
    fn empty_or_ws_or_other_is_unsupported() {
        assert_eq!(detect_format(""), InputFormat::Unsupported);
        assert_eq!(
            detect_format(" 【\u{73b0}\u{8d27}】"),
            InputFormat::Unsupported
        ); // leading space
        assert_eq!(detect_format("\t{"), InputFormat::Unsupported);
        assert_eq!(
            detect_format("[\u{73b0}\u{8d27}]"),
            InputFormat::Unsupported
        ); // half-width '['
        assert_eq!(detect_format("hello"), InputFormat::Unsupported);
    }
}
