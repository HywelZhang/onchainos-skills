//! FR-2.3 perp parser with SL/TP direction integrity (SR-5).
//!
//! `leverage` is a positive integer; exactly one `stopLoss`; 1..=3 take-profits.
//! Per V1.1/TD review alignment (feedback !1a4cebc6) TPs may be given as either
//! form:
//! - separate fields `止盈1|止盈2|止盈3` (`tp1|tp2|tp3`) with contiguous numbering, or
//! - one combined `止盈`/`takeProfit` field carrying `v1/v2/v3` slash-separated prices.
//!
//! Direction rules (the ONLY ordering constraint the protocol defines):
//! - LONG:  stopLoss < entryLo; every TP > entryLo.
//! - SHORT: stopLoss > entryHi; every TP < entryHi.
//!
//! The previous extra strict-monotonic TP ordering constraint is removed — it was
//! not required by the spec and rejected valid direction-correct signals. A
//! duplicate/zero/four TP, a numbering gap, or a wrong-side SL/TP → `DirectionConstraint`.

use super::super::error::ParseError;
use super::super::fields::{self, FieldMap};
use super::super::{Direction, PerpParams};

pub fn parse(fm: &mut FieldMap) -> Result<PerpParams, ParseError> {
    let pair = fm.require(fields::ID_PAIR)?;
    let direction = fields::parse_direction(&fm.require(fields::ID_DIRECTION)?)?;
    let leverage = fields::parse_leverage(&fm.require(fields::ID_LEVERAGE)?)?;
    let entry_range = fields::parse_range(&fm.require(fields::ID_ENTRY)?)?;
    let stop_loss = fields::parse_decimal(&fm.require(fields::ID_STOP_LOSS)?)?;

    // Two accepted TP forms: a combined slash field, OR separate tp1..tp3.
    let tp_combined = fm.take(fields::ID_TP);
    let tp1 = fm.take(fields::ID_TP1);
    let tp2 = fm.take(fields::ID_TP2);
    let tp3 = fm.take(fields::ID_TP3);
    let take_profit = collect_take_profit(tp_combined, tp1, tp2, tp3)?;

    let margin_mode = match fm.take(fields::ID_MARGIN_MODE) {
        Some(v) => Some(fields::parse_margin_mode(&v)?),
        None => None,
    };

    check_direction(
        direction,
        &entry_range.lo,
        &entry_range.hi,
        &stop_loss,
        &take_profit,
    )?;

    Ok(PerpParams {
        pair,
        direction,
        leverage,
        entry_range,
        stop_loss,
        take_profit,
        margin_mode,
    })
}

/// Collect the take-profit prices from whichever form was used. The two forms are
/// mutually exclusive; 1..=3 prices; separate fields must be contiguously numbered.
fn collect_take_profit(
    tp_combined: Option<String>,
    tp1: Option<String>,
    tp2: Option<String>,
    tp3: Option<String>,
) -> Result<Vec<String>, ParseError> {
    let has_separate = tp1.is_some() || tp2.is_some() || tp3.is_some();
    match tp_combined {
        Some(combined) => {
            // Mixing the combined and separate forms is a constraint violation.
            if has_separate {
                return Err(ParseError::DirectionConstraint);
            }
            let mut out = Vec::new();
            for part in combined.split('/') {
                let part = part.trim();
                if part.is_empty() {
                    return Err(ParseError::DirectionConstraint);
                }
                // A malformed TP price is a number error, not a direction error.
                out.push(fields::parse_decimal(part)?);
            }
            if out.is_empty() || out.len() > 3 {
                return Err(ParseError::DirectionConstraint);
            }
            Ok(out)
        }
        None => {
            // Separate fields: contiguous numbering starting at tp1, 1..=3 present.
            let present = [tp1.is_some(), tp2.is_some(), tp3.is_some()];
            let valid = matches!(
                present,
                [true, false, false] | [true, true, false] | [true, true, true]
            );
            if !valid {
                return Err(ParseError::DirectionConstraint);
            }
            let mut out = Vec::new();
            for tp in [tp1, tp2, tp3].into_iter().flatten() {
                out.push(fields::parse_decimal(&tp)?);
            }
            Ok(out)
        }
    }
}

/// SL/TP direction integrity — the correct side of the entry range. No monotonic
/// ordering constraint (removed per feedback !1a4cebc6).
fn check_direction(
    direction: Direction,
    entry_lo: &str,
    entry_hi: &str,
    stop_loss: &str,
    tps: &[String],
) -> Result<(), ParseError> {
    match direction {
        Direction::Long => {
            if !fields::less_than(stop_loss, entry_lo) {
                return Err(ParseError::DirectionConstraint);
            }
            if tps.iter().any(|tp| !fields::greater_than(tp, entry_lo)) {
                return Err(ParseError::DirectionConstraint);
            }
        }
        Direction::Short => {
            if !fields::greater_than(stop_loss, entry_hi) {
                return Err(ParseError::DirectionConstraint);
            }
            if tps.iter().any(|tp| !fields::less_than(tp, entry_hi)) {
                return Err(ParseError::DirectionConstraint);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// feedback !1a4cebc6: TPs on the correct side of entry but NOT strictly
    /// monotonic (equal, or out-of-order) are now ACCEPTED — the extra monotonic
    /// constraint was removed. Only the side check applies.
    #[test]
    fn long_tps_correct_side_non_monotonic_now_accepted() {
        // All TPs > entryLo (60000) and SL < entryLo → side checks pass.
        let equal = vec!["62000".to_string(), "62000".to_string()];
        assert!(check_direction(Direction::Long, "60000", "61000", "59000", &equal).is_ok());
        let descending = vec!["63000".to_string(), "62000".to_string()];
        assert!(check_direction(Direction::Long, "60000", "61000", "59000", &descending).is_ok());
        // Wrong side is still rejected.
        let wrong_side = vec!["59500".to_string()];
        assert_eq!(
            check_direction(Direction::Long, "60000", "61000", "59000", &wrong_side).unwrap_err(),
            ParseError::DirectionConstraint
        );
    }

    #[test]
    fn short_tps_correct_side_non_monotonic_now_accepted() {
        // All TPs < entryHi (610) and SL > entryHi → side checks pass.
        let equal = vec!["590".to_string(), "590".to_string()];
        assert!(check_direction(Direction::Short, "600", "610", "620", &equal).is_ok());
        let ascending = vec!["580".to_string(), "590".to_string()];
        assert!(check_direction(Direction::Short, "600", "610", "620", &ascending).is_ok());
    }

    /// feedback !1a4cebc6: the combined slash form `v1/v2/v3` and the separate
    /// `tp1..tp3` form yield the same take-profit vector; mixing the two forms is
    /// a constraint violation.
    #[test]
    fn tp_slash_form_and_separate_form() {
        // combined slash form.
        assert_eq!(
            collect_take_profit(Some("62000/63000/64000".to_string()), None, None, None).unwrap(),
            vec!["62000", "63000", "64000"]
        );
        // separate form.
        assert_eq!(
            collect_take_profit(
                None,
                Some("62000".to_string()),
                Some("63000".to_string()),
                None
            )
            .unwrap(),
            vec!["62000", "63000"]
        );
        // mixing the two forms → DirectionConstraint.
        assert_eq!(
            collect_take_profit(
                Some("62000".to_string()),
                Some("63000".to_string()),
                None,
                None
            )
            .unwrap_err(),
            ParseError::DirectionConstraint
        );
        // more than three combined TPs → DirectionConstraint.
        assert_eq!(
            collect_take_profit(Some("1/2/3/4".to_string()), None, None, None).unwrap_err(),
            ParseError::DirectionConstraint
        );
        // numbering gap in the separate form (tp1 + tp3, no tp2) → DirectionConstraint.
        assert_eq!(
            collect_take_profit(
                None,
                Some("62000".to_string()),
                None,
                Some("64000".to_string())
            )
            .unwrap_err(),
            ParseError::DirectionConstraint
        );
    }
}
