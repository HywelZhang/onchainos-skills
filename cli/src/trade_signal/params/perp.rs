//! FR-2.3 perp parser with SL/TP direction integrity (SR-5).
//!
//! `leverage` is a positive integer; exactly one `stopLoss`; 1..=3 take-profits
//! labelled tp1..tp3 with no gap. Direction rules:
//! - LONG:  stopLoss < entryLo; every TP > entryLo; TPs strictly ascending.
//! - SHORT: stopLoss > entryHi; every TP < entryHi; TPs strictly descending.
//!
//! Any violation, a duplicate/zero/four TP, or a TP numbering gap → `DirectionConstraint`.

use super::super::error::ParseError;
use super::super::fields::{self, FieldMap};
use super::super::{Direction, PerpParams};

pub fn parse(fm: &mut FieldMap) -> Result<PerpParams, ParseError> {
    let pair = fm.require(fields::ID_PAIR)?;
    let direction = fields::parse_direction(&fm.require(fields::ID_DIRECTION)?)?;
    let leverage = fields::parse_leverage(&fm.require(fields::ID_LEVERAGE)?)?;
    let entry_range = fields::parse_range(&fm.require(fields::ID_ENTRY)?)?;
    let stop_loss = fields::parse_decimal(&fm.require(fields::ID_STOP_LOSS)?)?;

    // Collect tp1..tp3; enforce contiguous numbering (no gap, must start at tp1).
    let tp1 = fm.take(fields::ID_TP1);
    let tp2 = fm.take(fields::ID_TP2);
    let tp3 = fm.take(fields::ID_TP3);
    let take_profit = collect_take_profit(tp1, tp2, tp3)?;

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

/// Enforce contiguous tp1..tp3 numbering and parse each as a decimal price.
fn collect_take_profit(
    tp1: Option<String>,
    tp2: Option<String>,
    tp3: Option<String>,
) -> Result<Vec<String>, ParseError> {
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
        // A malformed TP price is a number error, not a direction error.
        out.push(fields::parse_decimal(&tp)?);
    }
    Ok(out)
}

/// SL/TP direction integrity + strict monotonic TP ordering.
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
            // strictly ascending
            for w in tps.windows(2) {
                if !fields::less_than(&w[0], &w[1]) {
                    return Err(ParseError::DirectionConstraint);
                }
            }
        }
        Direction::Short => {
            if !fields::greater_than(stop_loss, entry_hi) {
                return Err(ParseError::DirectionConstraint);
            }
            if tps.iter().any(|tp| !fields::less_than(tp, entry_hi)) {
                return Err(ParseError::DirectionConstraint);
            }
            // strictly descending
            for w in tps.windows(2) {
                if !fields::greater_than(&w[0], &w[1]) {
                    return Err(ParseError::DirectionConstraint);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M-2: TPs on the correct side of entry but NOT strictly monotonic
    /// (equal or out-of-order) are a `DirectionConstraint`, distinct from the
    /// wrong-side / gap cases already covered by the AC-16 corpus.
    #[test]
    fn long_tps_correct_side_but_not_strictly_ascending() {
        // All TPs > entryLo (60000) and SL < entryLo, so the side checks pass;
        // the only violation is the non-strict ordering.
        let equal = vec!["62000".to_string(), "62000".to_string()];
        assert_eq!(
            check_direction(Direction::Long, "60000", "61000", "59000", &equal).unwrap_err(),
            ParseError::DirectionConstraint
        );
        let descending = vec!["63000".to_string(), "62000".to_string()];
        assert_eq!(
            check_direction(Direction::Long, "60000", "61000", "59000", &descending).unwrap_err(),
            ParseError::DirectionConstraint
        );
        // Control: a strictly-ascending, correct-side set passes.
        let ok = vec!["62000".to_string(), "63000".to_string()];
        assert!(check_direction(Direction::Long, "60000", "61000", "59000", &ok).is_ok());
    }

    #[test]
    fn short_tps_correct_side_but_not_strictly_descending() {
        // All TPs < entryHi (610) and SL > entryHi, so the side checks pass.
        let equal = vec!["590".to_string(), "590".to_string()];
        assert_eq!(
            check_direction(Direction::Short, "600", "610", "620", &equal).unwrap_err(),
            ParseError::DirectionConstraint
        );
        let ascending = vec!["580".to_string(), "590".to_string()];
        assert_eq!(
            check_direction(Direction::Short, "600", "610", "620", &ascending).unwrap_err(),
            ParseError::DirectionConstraint
        );
        // Control: a strictly-descending, correct-side set passes.
        let ok = vec!["590".to_string(), "580".to_string()];
        assert!(check_direction(Direction::Short, "600", "610", "620", &ok).is_ok());
    }
}
