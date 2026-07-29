//! FR-2.4 prediction parser. `outcome` ∈ {YES,NO,UP,DOWN}; `odds` an absolute
//! decimal in [0,1]; `settleDate` a real `YYYY-MM-DD`. `event` is free text and
//! is NEVER echoed in an error (SR-3) — it is only captured on success.

use super::super::error::ParseError;
use super::super::fields::{self, FieldMap};
use super::super::PredictionParams;

pub fn parse(fm: &mut FieldMap) -> Result<PredictionParams, ParseError> {
    let event = fm.require(fields::ID_EVENT)?;
    let outcome = fields::parse_outcome(&fm.require(fields::ID_OUTCOME)?)?;
    let odds = fields::parse_odds(&fm.require(fields::ID_ODDS)?)?;
    let settle_date = fields::parse_date(&fm.require(fields::ID_SETTLE_DATE)?)?;

    Ok(PredictionParams {
        event,
        outcome,
        odds,
        settle_date,
    })
}
