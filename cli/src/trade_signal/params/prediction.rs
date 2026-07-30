//! FR-2.4 prediction parser. The `outcome` field is a single fixed-position
//! `<OUTCOME> @<odds>` value (feedback !21bc5915): `outcome` ∈ {YES,NO,UP,DOWN}
//! and `odds` an absolute decimal in [0,1], separated by exactly one `@`.
//! `settleDate` a real `YYYY-MM-DD`. `event` is free text and is NEVER echoed in
//! an error (SR-3) — it is only captured on success.

use super::super::error::ParseError;
use super::super::fields::{self, FieldMap};
use super::super::PredictionParams;

pub fn parse(fm: &mut FieldMap) -> Result<PredictionParams, ParseError> {
    let event = fm.require(fields::ID_EVENT)?;
    let (outcome, odds) = fields::parse_outcome_odds(&fm.require(fields::ID_OUTCOME)?)?;
    let settle_date = fields::parse_date(&fm.require(fields::ID_SETTLE_DATE)?)?;

    Ok(PredictionParams {
        event,
        outcome,
        odds,
        settle_date,
    })
}
