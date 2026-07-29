//! FR-2.5 option parser with `contractCode` cross-consistency (OptionFieldMismatch).
//!
//! `contractCode` = `UNDERLYING-YYMMDD-STRIKE-C|P`. Its trailing date, strike, and
//! C/P MUST match the standalone `expiry`, `strike`, and `optionType` fields
//! (2-digit year is interpreted as 20YY).

use super::super::error::ParseError;
use super::super::fields::{self, FieldMap};
use super::super::{OptionParams, OptionType};

pub fn parse(fm: &mut FieldMap) -> Result<OptionParams, ParseError> {
    let contract_code = fm.require(fields::ID_CONTRACT_CODE)?;
    let side = fields::parse_option_side(&fm.require(fields::ID_SIDE)?)?;
    let option_type = fields::parse_option_type(&fm.require(fields::ID_OPTION_TYPE)?)?;
    let strike = fields::parse_decimal(&fm.require(fields::ID_STRIKE)?)?;
    let expiry = fields::parse_date(&fm.require(fields::ID_EXPIRY)?)?;
    let premium_cap = fields::parse_decimal(&fm.require(fields::ID_PREMIUM_CAP)?)?;

    check_contract_consistency(&contract_code, option_type, &strike, &expiry)?;

    Ok(OptionParams {
        contract_code,
        side,
        option_type,
        strike,
        expiry,
        premium_cap,
    })
}

/// Verify `UNDERLYING-YYMMDD-STRIKE-(C|P)` matches the typed fields.
fn check_contract_consistency(
    code: &str,
    option_type: OptionType,
    strike: &str,
    expiry: &str,
) -> Result<(), ParseError> {
    let parts: Vec<&str> = code.split('-').collect();
    if parts.len() != 4 {
        return Err(ParseError::OptionFieldMismatch);
    }
    let (yymmdd, code_strike, cp) = (parts[1], parts[2], parts[3]);

    // Trailing C/P vs optionType.
    let cp_ok = matches!(
        (cp, option_type),
        ("C", OptionType::Call) | ("P", OptionType::Put)
    );
    if !cp_ok {
        return Err(ParseError::OptionFieldMismatch);
    }

    // Strike equality (exact decimal compare).
    if !fields::equal(code_strike, strike) {
        return Err(ParseError::OptionFieldMismatch);
    }

    // YYMMDD → 20YY-MM-DD vs expiry.
    if yymmdd.len() != 6 || !yymmdd.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseError::OptionFieldMismatch);
    }
    let code_date = format!("20{}-{}-{}", &yymmdd[0..2], &yymmdd[2..4], &yymmdd[4..6]);
    if code_date != expiry {
        return Err(ParseError::OptionFieldMismatch);
    }
    Ok(())
}
