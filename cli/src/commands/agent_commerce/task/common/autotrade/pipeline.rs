//! FR-4 fixed-order runtime validation chain + idempotency latch.
//!
//! Composes the Phase-2 building blocks into either an [`card::ExecutionCard`]
//! (all checks pass) or a [`card::NotifyOnly`] (any degrade). The order is fixed:
//! structure → freshness → subscription → wallet → type-enabled → grant cap →
//! holding (sell+pct) → idempotency latch. Every branch returns a stable
//! machine-readable reason matching the audit action names.

use std::io::ErrorKind;
use std::path::PathBuf;

use super::amount::Decimal;
use super::card::{self, ExecutionCard, NotifyOnly};
use super::schema::{self, AmountUnit, AutoTradeSignal, DefiAction, Side, TypedParams};
use super::{holding, subscription};
use super::{AutoTradeError, DegradeReason};

/// Runtime inputs for the buyer inbound pipeline.
pub struct PipelineInput<'a> {
    /// The `autotrade:` line JSON (omitting nothing; `signalTime` present).
    pub signal_json: &'a str,
    pub job_id: &'a str,
    pub agent_id: &'a str,
    /// When the buyer received the delivery (ms since epoch).
    pub received_at_ms: u64,
    /// Locally-saved deliverable path — surfaced only on the notify-only path.
    pub saved_path: &'a str,
}

/// The pipeline result: exactly one of card / notify-only.
pub enum PipelineOutcome {
    Card(Box<ExecutionCard>),
    Notify(NotifyOnly),
}

/// Current wall-clock in milliseconds since the Unix epoch.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// AC-4 freshness: age is measured from `min(signalTime, receivedAt)`; expired
/// when `age > ttlSec`. Returns `FreshnessExpired` when stale.
fn check_freshness(
    signal_time_ms: u64,
    received_at_ms: u64,
    now_ms: u64,
    ttl_sec: u64,
) -> Result<(), AutoTradeError> {
    let effective = signal_time_ms.min(received_at_ms);
    // Clock skew / future timestamp ⇒ treat as fresh (age 0).
    let age_ms = now_ms.saturating_sub(effective);
    if age_ms > ttl_sec.saturating_mul(1000) {
        return Err(AutoTradeError::Degrade(DegradeReason::FreshnessExpired));
    }
    Ok(())
}

/// Resolve the buyer's active wallet address for `chain_index` (exact chain, then
/// same family). `None` ⇒ `no_active_wallet`.
fn resolve_active_wallet_address(chain_index: &str) -> Option<String> {
    let wallets = crate::wallet_store::load_wallets().ok().flatten()?;
    if wallets.selected_account_id.is_empty() {
        return None;
    }
    let entry = wallets.accounts_map.get(&wallets.selected_account_id)?;
    let family = crate::chains::chain_family(chain_index);
    entry
        .address_list
        .iter()
        .find(|a| a.chain_index == chain_index)
        .or_else(|| {
            entry
                .address_list
                .iter()
                .find(|a| crate::chains::chain_family(&a.chain_index) == family)
        })
        .map(|a| a.address.clone())
        .filter(|s| !s.is_empty())
}

/// `<onchainos_home>/autotrade/latch/<jobId>/<deliveryId>`.
fn latch_path(job_id: &str, delivery_id: &str) -> Result<PathBuf, AutoTradeError> {
    let home = crate::home::onchainos_home()
        .map_err(|_| AutoTradeError::Degrade(DegradeReason::LatchWriteFail))?;
    Ok(home
        .join("autotrade")
        .join("latch")
        .join(job_id)
        .join(delivery_id))
}

/// Idempotency latch: atomically create the marker (`create_new`) BEFORE returning
/// the card. An existing marker ⇒ `replay_skip`; any other IO error ⇒
/// `latch_write_fail`. Worst case = miss an execution, never a double.
fn write_latch(job_id: &str, delivery_id: &str) -> Result<(), AutoTradeError> {
    let path = latch_path(job_id, delivery_id)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| AutoTradeError::Degrade(DegradeReason::LatchWriteFail))?;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            Err(AutoTradeError::Degrade(DegradeReason::ReplaySkip))
        }
        Err(_) => Err(AutoTradeError::Degrade(DegradeReason::LatchWriteFail)),
    }
}

/// The venue + action + spend amount used for the grant-check, plus (for dex
/// sell+pct) the resolved readable amount to embed in the command.
struct GrantTarget {
    venue: &'static str,
    action: &'static str,
    amount: String,
    /// Set only for dex sell+pct (the holding-derived absolute amount).
    resolved_dex_amount: Option<String>,
}

/// Determine the grant-check target. For `dex_trade` sell+pct this resolves the
/// on-chain holding first (FR-7) so the cap is checked against the absolute
/// amount. Withdraw / claim spend nothing, so they are not grant-gated.
async fn resolve_grant_target(
    typed: &TypedParams,
    wallet: &str,
) -> Result<Option<GrantTarget>, AutoTradeError> {
    match typed {
        TypedParams::Dex(p) => match (p.side, p.amount_unit) {
            (Side::Buy, _) => Ok(Some(GrantTarget {
                venue: "dex",
                action: "buy",
                amount: p.amount.clone(),
                resolved_dex_amount: None,
            })),
            (Side::Sell, AmountUnit::Base) => Ok(Some(GrantTarget {
                venue: "dex",
                action: "sell",
                amount: p.amount.clone(),
                resolved_dex_amount: None,
            })),
            (Side::Sell, AmountUnit::Pct) => {
                let pct = Decimal::parse(&p.amount)
                    .map_err(|_| AutoTradeError::Degrade(DegradeReason::PctHoldingFail))?;
                let abs =
                    holding::resolve_pct_holding(wallet, &p.chain_index, &p.token_address, &pct)
                        .await?;
                let abs_str = abs.to_plain_string();
                Ok(Some(GrantTarget {
                    venue: "dex",
                    action: "sell",
                    amount: abs_str.clone(),
                    resolved_dex_amount: Some(abs_str),
                }))
            }
            // schema rejects sell+quote; unreachable, treat defensively.
            (Side::Sell, AmountUnit::Quote) => {
                Err(AutoTradeError::Degrade(DegradeReason::StructureReject))
            }
        },
        TypedParams::Defi(p) => match p.action {
            DefiAction::Deposit => Ok(Some(GrantTarget {
                venue: "defi",
                action: "buy",
                amount: p.amount.clone().unwrap_or_default(),
                resolved_dex_amount: None,
            })),
            // withdraw / claim spend nothing → not grant-gated.
            DefiAction::Withdraw | DefiAction::Claim => Ok(None),
        },
        TypedParams::Polymarket(p) => {
            let action = match p.side {
                Side::Buy => "buy",
                Side::Sell => "sell",
            };
            Ok(Some(GrantTarget {
                venue: "polymarket",
                action,
                amount: p.amount.clone(),
                resolved_dex_amount: None,
            }))
        }
    }
}

/// Run the full fixed-order chain. Returns `Ok(outcome)`; the caller maps a
/// [`PipelineOutcome::Card`] / `Notify` onto `output::success`.
pub async fn run(input: PipelineInput<'_>) -> PipelineOutcome {
    match run_inner(&input).await {
        Ok(card) => PipelineOutcome::Card(Box::new(card)),
        Err(AutoTradeError::Degrade(reason)) => {
            PipelineOutcome::Notify(card::make_notify_only(input.saved_path, reason.as_str()))
        }
        // A structural reject on the inbound path is surfaced as a notify-only
        // with the stable `structure_reject` reason (reject, not silent ignore).
        Err(AutoTradeError::Reject(_)) => PipelineOutcome::Notify(card::make_notify_only(
            input.saved_path,
            DegradeReason::StructureReject.as_str(),
        )),
    }
}

async fn run_inner(input: &PipelineInput<'_>) -> Result<ExecutionCard, AutoTradeError> {
    // 1. structure.
    let signal: AutoTradeSignal = serde_json::from_str(input.signal_json)
        .map_err(|e| AutoTradeError::Reject(format!("signal parse: {e}")))?;
    schema::validate_structure(&signal)?;

    // 2. freshness (AC-4: min(signalTime, receivedAt) vs ttl).
    check_freshness(
        signal.signal_time,
        input.received_at_ms,
        now_ms(),
        signal.ttl_sec,
    )?;

    // 3. subscription Active / copyTrade.
    let mut client = super::super::network::task_api_client::TaskApiClient::new();
    let sub = subscription::determine_copy_trade(&mut client, input.job_id, input.agent_id).await?;

    // 5. type enabled (before touching params for the command).
    if !signal.signal_type.is_enabled() {
        return Err(AutoTradeError::Degrade(DegradeReason::TypeDegrade));
    }
    let typed = schema::parse_and_validate(&signal)?;

    // 4. wallet present (needs the signal's chain to pick the right address).
    let chain_index = signal_chain_index(&typed);
    let wallet = match chain_index {
        Some(idx) => resolve_active_wallet_address(idx)
            .ok_or(AutoTradeError::Degrade(DegradeReason::NoActiveWallet))?,
        // polymarket has no on-chain --chain arg; still require an active wallet.
        None => resolve_active_wallet_address("1")
            .ok_or(AutoTradeError::Degrade(DegradeReason::NoActiveWallet))?,
    };

    // 6 + 7. grant cap (+ holding resolution for dex sell+pct).
    let grant_target = resolve_grant_target(&typed, &wallet).await?;
    if let Some(gt) = &grant_target {
        super::grants::check_grant(input.job_id, gt.venue, gt.action, &gt.amount)
            .map_err(|_| AutoTradeError::Degrade(DegradeReason::OverCap))?;
    }

    // Assemble the command before latching (so a build failure doesn't burn the latch).
    let resolved = grant_target
        .as_ref()
        .and_then(|gt| gt.resolved_dex_amount.as_deref());
    let command = card::assemble_command(&typed, &wallet, input.job_id, resolved)?;

    // 8. idempotency latch — write marker first, THEN return the card.
    write_latch(input.job_id, &signal.delivery_id)?;

    Ok(card::make_execution_card(
        &signal.delivery_id,
        signal.signal_type.as_str(),
        &sub.provider_agent_id,
        command,
    ))
}

/// The chain index that governs the executed command's `--chain`, if any.
fn signal_chain_index(typed: &TypedParams) -> Option<&str> {
    match typed {
        TypedParams::Dex(p) => Some(&p.chain_index),
        TypedParams::Defi(p) => p.chain_index.as_deref(),
        TypedParams::Polymarket(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac4_freshness_uses_min_of_signal_and_received() {
        let ttl = 60u64; // 60s
        let now = 1_000_000_000_000u64;
        // signalTime is fresh but receivedAt is old → min(old) ⇒ expired.
        let signal_time = now - 10_000; // 10s ago
        let received_at = now - 120_000; // 120s ago
        assert!(check_freshness(signal_time, received_at, now, ttl).is_err());
        // both fresh ⇒ ok
        assert!(check_freshness(now - 5_000, now - 5_000, now, ttl).is_ok());
        // exactly at the boundary (age == ttl) ⇒ ok
        assert!(check_freshness(now - 60_000, now - 60_000, now, ttl).is_ok());
        // one ms past ⇒ expired
        assert!(check_freshness(now - 60_001, now - 60_001, now, ttl).is_err());
    }

    #[test]
    fn freshness_future_timestamp_is_fresh() {
        let now = 1_000u64;
        assert!(check_freshness(now + 5_000, now + 5_000, now, 1).is_ok());
    }

    #[test]
    fn ac6_latch_marker_before_card_and_replay_skip() {
        let _lock = crate::home::TEST_ENV_MUTEX.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("ONCHAINOS_HOME", tmp.path());

        // First write succeeds and the marker exists on disk.
        assert!(write_latch("job1", "d-1").is_ok());
        let marker = latch_path("job1", "d-1").unwrap();
        assert!(
            marker.exists(),
            "marker must be written before the card is returned"
        );

        // Second write for the same (job, delivery) ⇒ replay_skip.
        assert!(matches!(
            write_latch("job1", "d-1"),
            Err(AutoTradeError::Degrade(DegradeReason::ReplaySkip))
        ));

        // A different deliveryId is independent.
        assert!(write_latch("job1", "d-2").is_ok());

        std::env::remove_var("ONCHAINOS_HOME");
    }
}
