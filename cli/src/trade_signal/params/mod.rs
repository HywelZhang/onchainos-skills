//! FR-2.2…FR-2.6 dispatch: route a validated [`FieldMap`] to the per-class
//! parser and wrap the result in the internally-tagged [`SignalParams`].
//!
//! `position`/`ttl` are common fields already consumed by the caller
//! ([`super::parse_signal_text`]); each per-class parser consumes only its own
//! fields, and the caller asserts nothing is left over (extra-field guard).

pub mod defi;
pub mod option;
pub mod perp;
pub mod prediction;
pub mod spot;

use crate::asset_class::AssetClass;

use super::error::ParseError;
use super::fields::FieldMap;
use super::SignalParams;

/// Build the class-specific params variant from the remaining fields.
pub fn dispatch(class: AssetClass, fm: &mut FieldMap) -> Result<SignalParams, ParseError> {
    Ok(match class {
        AssetClass::Spot => SignalParams::Spot(spot::parse(fm)?),
        AssetClass::Perp => SignalParams::Perp(perp::parse(fm)?),
        AssetClass::Prediction => SignalParams::Prediction(prediction::parse(fm)?),
        AssetClass::Option => SignalParams::Option(option::parse(fm)?),
        AssetClass::Defi => SignalParams::Defi(defi::parse(fm)?),
    })
}
