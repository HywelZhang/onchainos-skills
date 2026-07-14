//! Read-only subscription display commands (user-side).
//!
//! Two commands under `onchainos agent`:
//! - `my-subscriptions` — list the logged-in agent's AI-service subscriptions
//!   (buyer or provider view).
//! - `subscribe-detail <subId>` — show one subscription's full detail by id.
//!
//! Both are read-only: they emit the always-on JSON envelope via
//! `crate::output::success` (exit 0) or bubble an error to `main.rs` (exit 1).
//! No on-chain signing, no `confirming` gate (never exit 2).

use anyhow::{anyhow, bail, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::commands::agent_commerce::task::common::network::task_api_client::TaskApiClient;
use crate::commands::agent_commerce::task::common::query as common_query;
use crate::commands::agent_commerce::task::common::{AGENT_ROLE_ASP, AGENT_ROLE_USER};

/// Full path prefix for the subscription endpoints. Per-endpoint paths
/// carrying `/task/` take precedence over the header prefix.
const SUBSCRIBE_PREFIX: &str = "/priapi/v1/aieco/task/subscribe";

/// Subscription viewpoint for `my-subscriptions`.
#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "lower")] // -> "buyer" / "provider"
pub enum SubscriptionRole {
    Buyer,
    Provider,
}

impl SubscriptionRole {
    /// Role code for resolving the representative agenticId header.
    fn agent_role(self) -> i64 {
        match self {
            Self::Buyer => AGENT_ROLE_USER,
            Self::Provider => AGENT_ROLE_ASP,
        }
    }
}

/// One subscription record — the scripting stability contract (`data` shape).
///
/// All wire fields pass through verbatim **plus** the derived `statusName`.
/// Financial amounts stay `String` (never floats). `#[serde(default)]` at the
/// container level tolerates missing/absent fields; unknown wire fields are
/// ignored (no `deny_unknown_fields`).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SubscriptionInfo {
    /// Primary key. NOT `subId`.
    pub job_id: String,
    /// 0=JOB_TASK, 1=JOB_SUBSCRIBE.
    pub job_type: i64,
    /// Raw status enum (see `status_name`).
    pub status: i64,
    /// Derived English status label. NOT read from the wire — always emitted.
    #[serde(skip_deserializing)]
    pub status_name: String,
    /// e.g. 196 (XLayer) — pass-through, not resolved.
    pub chain_id: i64,
    pub title: String,
    pub description: String,
    pub description_summary: String,
    pub buyer_agent_id: String,
    pub buyer_agent_address: String,
    /// May be empty pre-match.
    pub provider_agent_id: String,
    pub provider_agent_address: String,
    /// 0=non-trial, 1=trial.
    pub trial_type: i64,
    /// Seconds epoch; `null` for non-trial. KEEP the wire misspelling `trail`.
    pub trail_start_time: Option<i64>,
    /// Seconds epoch; `null` for non-trial. KEEP `trail`.
    pub trail_end_time: Option<i64>,
    pub sub_start_time: i64,
    pub sub_end_time: i64,
    /// subEndTime + 1 day.
    pub sub_buffer_end_time: i64,
    /// 0/1.
    pub auto_renew: i64,
    /// 0/1.
    pub copy_trade: i64,
    /// Starts at 1.
    pub period_index: i64,
    pub service_id: String,
    /// Opaque JSON string, pass-through.
    pub service_params: String,
    pub service_token_address: String,
    /// Decimal STRING — never a float.
    pub service_token_amount: String,
    pub payment_token_address: String,
    /// Decimal STRING — never a float.
    pub payment_token_amount: String,
    /// Decimal STRING — never a float.
    pub payment_currency_amount: String,
}

/// Internal wrapper for deserializing the `/my` list response.
#[derive(Debug, Default, Deserialize)]
struct SubscriptionList {
    #[serde(default)]
    list: Vec<SubscriptionInfo>,
}

/// English status label derived from `status` alone. Defensive `UNKNOWN_<n>`
/// fallback for unmapped values.
pub fn status_name(status: i64) -> String {
    match status {
        -1 => "INIT".to_string(),
        0 => "NONE".to_string(),
        1 => "ACTIVE".to_string(),
        2 => "REJECTED".to_string(),
        3 => "DISPUTED".to_string(),
        4 => "COMPLETED".to_string(),
        5 => "FAILED".to_string(),
        6 => "CLOSED".to_string(),
        n => format!("UNKNOWN_{n}"),
    }
}

/// `GET /priapi/v1/aieco/task/subscribe/my`
///
/// Request path for the `/my` list endpoint — no query string. The endpoint
/// defines no request params; role/status are applied client-side.
fn my_subscriptions_path() -> String {
    format!("{SUBSCRIBE_PREFIX}/my")
}

/// Client-side view + status filter over the flat `/my` list.
///
/// The endpoint returns the full list for the logged-in agent; there is no
/// server-side paging, role, or status param. Buyer view keeps records whose
/// `buyerAgentId` equals the logged-in agentId; provider view keeps records
/// whose `providerAgentId` equals it. `status`, when given, keeps records whose
/// raw status matches.
fn filter_subscriptions(
    list: Vec<SubscriptionInfo>,
    role: SubscriptionRole,
    self_agent_id: &str,
    status: Option<i32>,
) -> Vec<SubscriptionInfo> {
    list.into_iter()
        .filter(|item| match role {
            SubscriptionRole::Buyer => item.buyer_agent_id == self_agent_id,
            SubscriptionRole::Provider => item.provider_agent_id == self_agent_id,
        })
        .filter(|item| status.is_none_or(|s| item.status == i64::from(s)))
        .collect()
}

/// `GET /priapi/v1/aieco/task/subscribe/my`
///
/// Lists the logged-in agent's subscriptions (buyer or provider view).
pub async fn handle_my_subscriptions(
    client: &mut TaskApiClient,
    role: SubscriptionRole,
    status: Option<i32>,
) -> Result<()> {
    let header_agent = common_query::resolve_agent_id("", role.agent_role()).await;
    // The `/my` endpoint defines NO request params and returns a flat list
    // (buyer + provider records for the logged-in agent), so `role` and
    // `status` are applied client-side below. NOTE: if the flat list ever
    // proves unable to carry provider-side records, confirm with backend
    // before adding a server-side role/view param — do not fabricate one here.
    let path = my_subscriptions_path();
    let data = client
        .get_with_identity(&path, &header_agent)
        .await
        .map_err(|e| anyhow!("failed to fetch subscriptions: {e}"))?;
    let wrapper: SubscriptionList = serde_json::from_value(data)
        .map_err(|e| anyhow!("failed to parse subscription list: {e}"))?;
    let mut list = filter_subscriptions(wrapper.list, role, &header_agent, status);
    for item in &mut list {
        item.status_name = status_name(item.status);
    }
    crate::output::success(serde_json::json!({ "list": list }));
    Ok(())
}

/// `GET /priapi/v1/aieco/task/subscribe/{subId}`
///
/// Shows one subscription's full detail by id.
pub async fn handle_subscribe_detail(client: &mut TaskApiClient, sub_id: &str) -> Result<()> {
    if sub_id.trim().is_empty() {
        bail!("<subId> must not be empty");
    }
    let header_agent = common_query::resolve_agent_id("", AGENT_ROLE_USER).await;
    let path = format!("{SUBSCRIBE_PREFIX}/{sub_id}");
    let data = client
        .get_with_identity(&path, &header_agent)
        .await
        .map_err(|e| anyhow!("failed to fetch subscription {sub_id}: {e}"))?;
    let mut info: SubscriptionInfo = serde_json::from_value(data)
        .map_err(|e| anyhow!("failed to parse subscription detail: {e}"))?;
    info.status_name = status_name(info.status);
    crate::output::success(info);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A representative detail fixture with all detail fields present (trial case).
    fn detail_fixture() -> serde_json::Value {
        json!({
            "jobId": "1234567890",
            "jobType": 1,
            "status": 1,
            "chainId": 196,
            "title": "Alpha signals subscription",
            "description": "Daily alpha signals",
            "descriptionSummary": "alpha signals",
            "buyerAgentId": "1001",
            "buyerAgentAddress": "0xbuyer",
            "providerAgentId": "2002",
            "providerAgentAddress": "0xprovider",
            "trialType": 1,
            "trailStartTime": 1700000000,
            "trailEndTime": 1700600000,
            "subStartTime": 1700600000,
            "subEndTime": 1703192000,
            "subBufferEndTime": 1703278400,
            "autoRenew": 1,
            "copyTrade": 0,
            "periodIndex": 1,
            "serviceId": "svc-1",
            "serviceParams": "{\"k\":\"v\"}",
            "serviceTokenAddress": "0xservice",
            "serviceTokenAmount": "10.500000",
            "paymentTokenAddress": "0xpayment",
            "paymentTokenAmount": "10.500000",
            "paymentCurrencyAmount": "10.50"
        })
    }

    #[test]
    fn detail_json_deserializes_all_fields() {
        let info: SubscriptionInfo = serde_json::from_value(detail_fixture()).unwrap();
        assert_eq!(info.job_id, "1234567890");
        assert_eq!(info.job_type, 1);
        assert_eq!(info.status, 1);
        assert_eq!(info.chain_id, 196);
        assert_eq!(info.title, "Alpha signals subscription");
        assert_eq!(info.buyer_agent_id, "1001");
        assert_eq!(info.provider_agent_id, "2002");
        assert_eq!(info.trial_type, 1);
        assert_eq!(info.sub_start_time, 1700600000);
        assert_eq!(info.sub_end_time, 1703192000);
        assert_eq!(info.sub_buffer_end_time, 1703278400);
        assert_eq!(info.auto_renew, 1);
        assert_eq!(info.copy_trade, 0);
        assert_eq!(info.period_index, 1);
        assert_eq!(info.service_id, "svc-1");
        assert_eq!(info.service_params, "{\"k\":\"v\"}");
    }

    #[test]
    fn list_element_deserializes_via_wrapper() {
        let wire = json!({ "list": [ detail_fixture() ] });
        let wrapper: SubscriptionList = serde_json::from_value(wire).unwrap();
        assert_eq!(wrapper.list.len(), 1);
        assert_eq!(wrapper.list[0].job_id, "1234567890");
    }

    #[test]
    fn status_name_covers_all_nine_cases_and_unknown() {
        assert_eq!(status_name(-1), "INIT");
        assert_eq!(status_name(0), "NONE");
        assert_eq!(status_name(1), "ACTIVE");
        assert_eq!(status_name(2), "REJECTED");
        assert_eq!(status_name(3), "DISPUTED");
        assert_eq!(status_name(4), "COMPLETED");
        assert_eq!(status_name(5), "FAILED");
        assert_eq!(status_name(6), "CLOSED");
        // UNKNOWN_<n> fallback for any unmapped value.
        assert_eq!(status_name(9), "UNKNOWN_9");
        assert_eq!(status_name(42), "UNKNOWN_42");
    }

    #[test]
    fn trial_times_present_deserialize_to_some() {
        let info: SubscriptionInfo = serde_json::from_value(detail_fixture()).unwrap();
        assert_eq!(info.trail_start_time, Some(1700000000));
        assert_eq!(info.trail_end_time, Some(1700600000));
    }

    #[test]
    fn non_trial_null_times_deserialize_to_none() {
        let mut wire = detail_fixture();
        wire["trailStartTime"] = serde_json::Value::Null;
        wire["trailEndTime"] = serde_json::Value::Null;
        wire["trialType"] = json!(0);
        let info: SubscriptionInfo = serde_json::from_value(wire).unwrap();
        assert_eq!(info.trail_start_time, None);
        assert_eq!(info.trail_end_time, None);
        assert_eq!(info.trial_type, 0);
    }

    #[test]
    fn subscription_role_maps_agent_role() {
        assert_eq!(SubscriptionRole::Buyer.agent_role(), AGENT_ROLE_USER);
        assert_eq!(SubscriptionRole::Provider.agent_role(), AGENT_ROLE_ASP);
    }

    /// The `/my` endpoint takes NO request params — the path is exactly
    /// `.../subscribe/my` with no query string.
    #[test]
    fn my_subscriptions_path_has_no_query_string() {
        let path = my_subscriptions_path();
        assert_eq!(path, "/priapi/v1/aieco/task/subscribe/my");
        assert!(
            !path.contains('?'),
            "the /my endpoint takes no query params"
        );
    }

    /// Build a record with a given buyer / provider / status over the fixture.
    fn sub(buyer: &str, provider: &str, status: i64) -> SubscriptionInfo {
        let mut s: SubscriptionInfo = serde_json::from_value(detail_fixture()).unwrap();
        s.buyer_agent_id = buyer.to_string();
        s.provider_agent_id = provider.to_string();
        s.status = status;
        s
    }

    #[test]
    fn filter_subscriptions_buyer_and_provider_views_are_client_side() {
        // The flat /my list carries both viewpoints: record `a` is bought by
        // agent 1001, record `b` is provided by agent 1001.
        let list = || vec![sub("1001", "2002", 1), sub("3003", "1001", 4)];

        // Buyer view for agent 1001 keeps only the record it bought.
        let buyer = filter_subscriptions(list(), SubscriptionRole::Buyer, "1001", None);
        assert_eq!(buyer.len(), 1);
        assert_eq!(buyer[0].provider_agent_id, "2002");

        // Provider view for agent 1001 keeps only the record it provides.
        let provider = filter_subscriptions(list(), SubscriptionRole::Provider, "1001", None);
        assert_eq!(provider.len(), 1);
        assert_eq!(provider[0].buyer_agent_id, "3003");
    }

    #[test]
    fn filter_subscriptions_status_filter_is_client_side() {
        let list = || {
            vec![
                sub("1001", "2002", 1),
                sub("1001", "3003", 4),
                sub("1001", "4004", 1),
            ]
        };
        // No status → all buyer records for 1001 pass.
        assert_eq!(
            filter_subscriptions(list(), SubscriptionRole::Buyer, "1001", None).len(),
            3
        );
        // status=1 → only the two ACTIVE records pass.
        let active = filter_subscriptions(list(), SubscriptionRole::Buyer, "1001", Some(1));
        assert_eq!(active.len(), 2);
        assert!(active.iter().all(|s| s.status == 1));
        // A status with no matches yields an empty list (no fabricated rows).
        assert!(filter_subscriptions(list(), SubscriptionRole::Buyer, "1001", Some(9)).is_empty());
    }

    #[test]
    fn empty_or_missing_list_defaults_to_empty_array() {
        // Missing `list` key → #[serde(default)] supplies an empty Vec.
        let wrapper: SubscriptionList = serde_json::from_value(json!({})).unwrap();
        assert!(wrapper.list.is_empty());
        // Explicit empty array also deserializes to an empty Vec.
        let wrapper: SubscriptionList = serde_json::from_value(json!({ "list": [] })).unwrap();
        assert!(wrapper.list.is_empty());
        // Default wrapper serializes (as the envelope shape) to {"list": []}.
        let envelope = json!({ "list": SubscriptionList::default().list });
        assert_eq!(envelope, json!({ "list": [] }));
    }

    #[test]
    fn decimal_amounts_stay_string_and_round_trip() {
        let info: SubscriptionInfo = serde_json::from_value(detail_fixture()).unwrap();
        assert_eq!(info.service_token_amount, "10.500000");
        assert_eq!(info.payment_token_amount, "10.500000");
        assert_eq!(info.payment_currency_amount, "10.50");
        // Round-trip: re-serialize and confirm the amounts are still JSON strings
        // (no float coercion / precision loss).
        let out = serde_json::to_value(&info).unwrap();
        assert_eq!(out["serviceTokenAmount"], json!("10.500000"));
        assert_eq!(out["paymentTokenAmount"], json!("10.500000"));
        assert_eq!(out["paymentCurrencyAmount"], json!("10.50"));
        assert!(out["serviceTokenAmount"].is_string());
    }

    #[test]
    fn status_name_present_in_serialized_envelope_for_detail_and_list() {
        // Detail object: statusName is emitted after derivation.
        let mut info: SubscriptionInfo = serde_json::from_value(detail_fixture()).unwrap();
        info.status_name = status_name(info.status);
        let out = serde_json::to_value(&info).unwrap();
        assert_eq!(out["statusName"], json!("ACTIVE"));

        // List element: derive per-element then serialize the envelope shape.
        let mut wrapper: SubscriptionList =
            serde_json::from_value(json!({ "list": [ detail_fixture() ] })).unwrap();
        for item in &mut wrapper.list {
            item.status_name = status_name(item.status);
        }
        let envelope = json!({ "list": wrapper.list });
        assert_eq!(envelope["list"][0]["statusName"], json!("ACTIVE"));
    }
}
