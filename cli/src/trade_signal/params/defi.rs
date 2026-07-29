//! FR-2.6 DeFi parser. `apy` is a non-negative percent; `tvl` is a canonical
//! compact amount captured verbatim (NO float conversion); `chain`/`protocolPool`
//! are unresolved strings (D-1). `executionSemantics` is fixed to `deposit`.

use super::super::error::ParseError;
use super::super::fields::{self, FieldMap};
use super::super::{DefiParams, ExecutionSemantics};

pub fn parse(fm: &mut FieldMap) -> Result<DefiParams, ParseError> {
    let chain = fm.require(fields::ID_CHAIN)?;
    let protocol_pool = fm.require(fields::ID_PROTOCOL_POOL)?;
    let apy = fields::parse_percent_nonneg(&fm.require(fields::ID_APY)?)?;
    // TVL is kept as the raw compact string (no float parse); presence is enough.
    let tvl = fm.require(fields::ID_TVL)?;
    let token = fm.require(fields::ID_TOKEN)?;
    let redeem_terms = fm.require(fields::ID_REDEEM_TERMS)?;

    Ok(DefiParams {
        chain,
        protocol_pool,
        apy,
        tvl,
        token,
        redeem_terms,
        execution_semantics: ExecutionSemantics::Deposit,
    })
}
