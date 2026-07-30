//! Create a subscription task.
//!
//! Flow: providerConfirmStatus → EIP-712 sign terms → create → sign uopData → broadcast(bizType=101)

use anyhow::{bail, Result};
use std::time::Duration;

use crate::audit;
use crate::commands::agentic_wallet::auth::ensure_tokens_refreshed;
use crate::commands::agent_commerce::task::common::network::task_api_client::TaskApiClient;
use crate::commands::agent_commerce::task::common::DEBUG_LOG;
use crate::commands::agent_commerce::task::signing;

pub(crate) const SUBSCRIBE_API_PREFIX: &str = "/priapi/v1/aieco/task/subscribe";

pub struct CreateSubscribeParams {
    pub service_id: String,
    pub use_trial: bool,
    pub service_params: String,
    pub service_token_amount: String,
    pub service_token_address: String,
    pub auto_renew: i32,
    pub copy_trade: i32,
    pub title: String,
    pub description: String,
    pub description_summary: String,
    pub provider_agent_id: Option<String>,
    pub service_interval: String,
    pub format: String,
    /// Device ids to omit from the default all-devices routing set (repeatable).
    pub exclude_device: Option<Vec<String>>,
}

const MAX_TITLE_CHARS: usize = 64;
const MAX_DESCRIPTION_CHARS: usize = 4096;
const MAX_SUMMARY_CHARS: usize = 512;

impl CreateSubscribeParams {
    fn validate(&self) -> Result<()> {
        if self.service_id.is_empty() {
            bail!("--service-id is required");
        }
        if self.service_token_amount.is_empty() {
            bail!("--service-token-amount is required");
        }
        if self.service_token_address.is_empty() {
            bail!("--service-token-address is required");
        }
        if self.auto_renew != 0 && self.auto_renew != 1 {
            bail!("--auto-renew must be 0 (off) or 1 (on), got {}", self.auto_renew);
        }
        if self.copy_trade != 0 && self.copy_trade != 1 {
            bail!("--copy-trade must be 0 (off) or 1 (on), got {}", self.copy_trade);
        }
        if self.title.is_empty() {
            bail!("--title is required");
        }
        if self.title.chars().count() > MAX_TITLE_CHARS {
            bail!("--title exceeds {MAX_TITLE_CHARS} characters");
        }
        if self.description.is_empty() {
            bail!("--description is required");
        }
        if self.description.chars().count() > MAX_DESCRIPTION_CHARS {
            bail!("--description exceeds {MAX_DESCRIPTION_CHARS} characters");
        }
        if self.description_summary.chars().count() > MAX_SUMMARY_CHARS {
            bail!("--description-summary exceeds {MAX_SUMMARY_CHARS} characters");
        }
        Ok(())
    }
}

/// Assemble the `create` request body. `device_list` is ALWAYS embedded (even
/// empty) so the created record never relies on server-default routing;
/// `providerAgentId` is only present when a designated provider was requested.
fn build_create_body(
    params: &CreateSubscribeParams,
    effective_use_trial: bool,
    terms_for_create: serde_json::Value,
    terms_sig: &str,
    device_list: &[String],
) -> serde_json::Value {
    let mut create_body = serde_json::json!({
        "serviceId": params.service_id,
        "useTrial": effective_use_trial,
        "serviceParams": params.service_params,
        "serviceTokenAmount": params.service_token_amount,
        "serviceTokenAddress": params.service_token_address,
        "autoRenew": params.auto_renew,
        "copyTrade": params.copy_trade,
        "title": params.title,
        "description": params.description,
        "descriptionSummary": params.description_summary,
        "serviceInterval": params.service_interval,
        "terms": terms_for_create,
        "termsSig": terms_sig,
        "deviceList": device_list,
    });
    if let Some(ref pid) = params.provider_agent_id {
        create_body["providerAgentId"] = serde_json::json!(pid);
    }
    create_body
}

/// The json-mode success envelope data — always carries the `deviceRoutingDegraded`
/// marker so the skill can render the degraded notice without a second query.
fn build_create_success(sub_id: &str, tx_hash: &str, degraded: bool) -> serde_json::Value {
    serde_json::json!({
        "subId": sub_id,
        "txHash": tx_hash,
        "deviceRoutingDegraded": degraded,
    })
}

pub async fn handle_create_subscribe(
    client: &mut TaskApiClient,
    params: CreateSubscribeParams,
) -> Result<()> {
    params.validate()?;

    let json_mode = params.format.eq_ignore_ascii_case("json");

    ensure_tokens_refreshed().await
        .map_err(|e| anyhow::anyhow!("session has expired; run `onchainos wallet login` first: {e}"))?;

    let (user_agent_id, _) = super::create::resolve_user_agent().await?;
    if DEBUG_LOG {
        eprintln!("[create-subscribe] user identity check passed (agentId: {user_agent_id})");
    }

    let (account_id, address) = signing::resolve_wallet_by_agent_id(&user_agent_id).await?;

    // Step 1: providerConfirmStatus → terms object
    let mut confirm_body = serde_json::json!({
        "serviceId": params.service_id,
        "autoRenew": params.auto_renew,
        "useTrial": params.use_trial,
        "subId": 0,
    });
    if let Some(ref pid) = params.provider_agent_id {
        confirm_body["providerAgentId"] = serde_json::json!(pid);
    }

    let confirm_resp = client
        .post_with_identity(
            &format!("{SUBSCRIBE_API_PREFIX}/providerConfirmStatus"),
            &confirm_body,
            &user_agent_id,
        )
        .await
        .map_err(|e| anyhow::anyhow!("providerConfirmStatus failed: {e}"))?;

    if DEBUG_LOG {
        eprintln!("[create-subscribe] providerConfirmStatus response: {confirm_resp}");
    }

    if confirm_resp.is_null() || confirm_resp.as_object().map_or(true, |o| o.is_empty()) {
        bail!("providerConfirmStatus returned empty terms; the service may not support subscription");
    }

    let typed_data = &confirm_resp["typedData"];
    if typed_data.is_null() || typed_data.as_object().map_or(true, |o| o.is_empty()) {
        bail!("providerConfirmStatus response missing typedData");
    }

    // Step 2: EIP-712 sign terms (sign the typedData sub-object, not the full response)
    let terms_sig = signing::sign_typed_data(typed_data, &address).await
        .map_err(|e| anyhow::anyhow!("EIP-712 terms signing failed: {e}"))?;

    if DEBUG_LOG {
        eprintln!("[create-subscribe] termsSig: {terms_sig}");
    }

    // Step 3: POST create
    // terms = providerConfirmStatus response minus the nested typedData
    // (backend expects flat RenewalTerms fields only: asp, aspAgentId, token, subId, user, etc.)
    let mut terms_for_create = confirm_resp.clone();
    if let Some(obj) = terms_for_create.as_object_mut() {
        obj.remove("typedData");
    }

    // useTrial must come from the backend response (the authoritative source), not the
    // user-supplied flag — the backend may override it (e.g. trial already used).
    let effective_use_trial = confirm_resp["useTrial"].as_bool().unwrap_or(params.use_trial);
    if DEBUG_LOG && effective_use_trial != params.use_trial {
        eprintln!(
            "[create-subscribe] useTrial overridden by backend: requested={}, effective={}",
            params.use_trial, effective_use_trial
        );
    }

    // Resolve the receive-device routing set: default = all logged-in devices
    // (device-list paged to completion) minus any --exclude-device. If that query
    // fails or returns empty, degrade to this device only and mark the result —
    // never abort the create. deviceList is ALWAYS sent explicitly so the
    // created record never depends on server-default semantics.
    let excluded = params.exclude_device.clone().unwrap_or_default();
    let fetched = super::device_routing::fetch_all_device_ids(client, &user_agent_id)
        .await
        .ok();
    let this_device_id = crate::device::id::get_cached_device_id();
    let (device_list, device_routing_degraded) =
        super::device_routing::resolve_create_device_set(fetched, &excluded, this_device_id);
    if DEBUG_LOG {
        eprintln!(
            "[create-subscribe] deviceList={device_list:?} degraded={device_routing_degraded}"
        );
    }

    let create_body = build_create_body(
        &params,
        effective_use_trial,
        terms_for_create,
        &terms_sig,
        &device_list,
    );

    let create_resp = client
        .post_with_identity(
            &format!("{SUBSCRIBE_API_PREFIX}/create"),
            &create_body,
            &user_agent_id,
        )
        .await
        .map_err(|e| anyhow::anyhow!("create-subscribe failed: {e}"))?;

    let sub_id = create_resp["jobId"].as_str().unwrap_or("?").to_string();
    let biz_type = signing::extract_biz_type(&create_resp);
    let uop_data = &create_resp["uopData"];

    if DEBUG_LOG {
        eprintln!("[create-subscribe] subId={sub_id}, bizType={biz_type}");
    }

    // Step 4 + 5: sign uopData → broadcast (reuses the standard task broadcast endpoint)
    let tx_hash = signing::sign_uop_and_broadcast(
        client, uop_data, &account_id, &address, &sub_id, biz_type, &user_agent_id, None,
    ).await?;

    audit::log(
        "cli",
        "user/create_subscribe",
        true,
        Duration::default(),
        Some(vec![
            format!("subId={sub_id}"),
            format!("agentId={user_agent_id}"),
            format!("serviceId={}", params.service_id),
            format!("useTrial={effective_use_trial}"),
            format!("autoRenew={}", params.auto_renew),
            format!("copyTrade={}", params.copy_trade),
            format!("txHash={tx_hash}"),
        ]),
        None,
    );

    // Establish the copy-trade provider XMTP session now (before the first delivery) so
    // signals are not held at consent=0. Prefer the created record's copyTrade; fall back
    // to the requested flag. Runs in both json and text modes.
    {
        let is_ct = create_resp["copyTrade"]
            .as_i64()
            .map(|v| v == 1)
            .unwrap_or(params.copy_trade == 1);
        if let Some(pid) = params.provider_agent_id.as_deref() {
            super::subscription_ops::ensure_subscription_consent(&sub_id, &user_agent_id, pid, is_ct);
        }
    }

    if json_mode {
        crate::output::success(build_create_success(&sub_id, &tx_hash, device_routing_degraded));
        if super::content::is_cli_mode() {
            println!();
            println!("[Watch] 🛑 Mandatory next steps. End the turn after Step 2. Do NOT ask the user whether to watch — it is required to receive the next event.");
            println!();
            println!("**Step 1 (MANDATORY GATE) — Read `skills/okx-ai/references/watch-core.md` now.** If you have NOT read it in this session, you cannot proceed to Step 2 — Step 2's invocation, dispatch rules, and re-arm semantics live ONLY in that file. Skipping this step is a protocol violation.");
            println!();
            println!("**Step 2 — Execute the watch per `skills/okx-ai/references/watch-core.md` §Run watch, scoped to job-id `{sub_id}`.** Then dispatch each returned item per §Dispatch by `kind` and re-enter per §Re-enter after processing. (Do NOT guess the bash invocation — read watch-core.md first.)");
            println!();
            println!("⏭ Skip `detect_watch_support` — this `[Watch]` block is only emitted on supported platforms.");
        }
        return Ok(());
    }

    println!("✓ Subscription submitted (transaction broadcast, awaiting on-chain confirmation)");
    println!("  jobId:  {sub_id}");
    println!("  txHash: {tx_hash}");
    if device_routing_degraded {
        println!(
            "  ⚠ Device list unavailable — this subscription was set to receive on THIS device only; other devices can be added later."
        );
    }
    if let Some(ref pid) = params.provider_agent_id {
        println!("  Designated provider: {pid}");
    }
    if !super::content::is_cli_mode() {
        println!("Next: wait for the on-chain confirmation; the designated provider will be contacted automatically.");
    }
    if super::content::is_cli_mode() {
        println!();
        println!("[Watch] 🛑 Mandatory next steps. End the turn after Step 2. Do NOT ask the user whether to watch — it is required to receive the next event.");
        println!();
        println!("**Step 1 (MANDATORY GATE) — Read `skills/okx-ai/references/watch-core.md` now.** If you have NOT read it in this session, you cannot proceed to Step 2 — Step 2's invocation, dispatch rules, and re-arm semantics live ONLY in that file. Skipping this step is a protocol violation.");
        println!();
        println!("**Step 2 — Execute the watch per `skills/okx-ai/references/watch-core.md` §Run watch, scoped to job-id `{sub_id}`.** Then dispatch each returned item per §Dispatch by `kind` and re-enter per §Re-enter after processing. (Do NOT guess the bash invocation — read watch-core.md first.)");
        println!();
        println!("⏭ Skip `detect_watch_support` — this `[Watch]` block is only emitted on supported platforms.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: super::super::TaskCommand,
    }

    #[test]
    fn cli_create_subscribe_all_required() {
        let cli = TestCli::parse_from([
            "test", "create-subscribe",
            "--service-id", "svc_001",
            "--use-trial",
            "--service-token-amount", "10",
            "--service-token-address", "0x6776",
            "--auto-renew", "1",
            "--copy-trade", "0",
            "--title", "Signal Subscription",
            "--description", "On-chain signal subscription service",
            "--description-summary", "Signal sub",
        ]);
        match cli.cmd {
            super::super::TaskCommand::CreateSubscribe {
                service_id, use_trial, service_token_amount, service_token_address,
                auto_renew, copy_trade, title, description, description_summary,
                provider_agent_id, service_params, service_interval, format, exclude_device,
            } => {
                assert_eq!(service_id, "svc_001");
                assert!(use_trial);
                assert_eq!(service_token_amount, "10");
                assert_eq!(service_token_address, "0x6776");
                assert_eq!(auto_renew, "1");
                assert_eq!(copy_trade, "0");
                assert_eq!(title, "Signal Subscription");
                assert_eq!(description, "On-chain signal subscription service");
                assert_eq!(description_summary, "Signal sub");
                assert!(provider_agent_id.is_none());
                assert_eq!(service_params, "");
                assert_eq!(service_interval, "month");
                assert_eq!(format, "");
                assert!(exclude_device.is_none());
            }
            _ => panic!("expected CreateSubscribe"),
        }
    }

    #[test]
    fn cli_create_subscribe_with_provider() {
        let cli = TestCli::parse_from([
            "test", "create-subscribe",
            "--service-id", "svc_002",
            "--service-token-amount", "5",
            "--service-token-address", "0xAddr",
            "--auto-renew", "0",
            "--copy-trade", "1",
            "--title", "Copy Trade",
            "--description", "Auto copy trade subscription",
            "--description-summary", "Copy",
            "--provider-agent-id", "agent-99",
        ]);
        match cli.cmd {
            super::super::TaskCommand::CreateSubscribe {
                provider_agent_id, use_trial, copy_trade, ..
            } => {
                assert_eq!(provider_agent_id.as_deref(), Some("agent-99"));
                assert!(!use_trial);
                assert_eq!(copy_trade, "1");
            }
            _ => panic!("expected CreateSubscribe"),
        }
    }

    #[test]
    fn cli_create_subscribe_bool_strings() {
        let cli = TestCli::parse_from([
            "test", "create-subscribe",
            "--service-id", "svc_003",
            "--service-token-amount", "1",
            "--service-token-address", "0xA",
            "--auto-renew", "true",
            "--copy-trade", "false",
            "--title", "t",
            "--description", "d for test bool strings ok",
            "--description-summary", "s",
        ]);
        match cli.cmd {
            super::super::TaskCommand::CreateSubscribe { auto_renew, copy_trade, .. } => {
                assert_eq!(auto_renew, "true");
                assert_eq!(copy_trade, "false");
            }
            _ => panic!("expected CreateSubscribe"),
        }
    }

    #[test]
    fn cli_create_subscribe_missing_service_id_fails() {
        assert!(TestCli::try_parse_from([
            "test", "create-subscribe",
            "--service-token-amount", "10",
            "--service-token-address", "0xAddr",
            "--auto-renew", "1",
            "--copy-trade", "0",
            "--title", "t",
            "--description", "d",
            "--description-summary", "s",
        ]).is_err());
    }

    fn params_fixture(provider: Option<&str>) -> super::CreateSubscribeParams {
        super::CreateSubscribeParams {
            service_id: "svc".to_string(),
            use_trial: false,
            service_params: String::new(),
            service_token_amount: "10".to_string(),
            service_token_address: "0xtok".to_string(),
            auto_renew: 1,
            copy_trade: 0,
            title: "t".to_string(),
            description: "d".to_string(),
            description_summary: "s".to_string(),
            provider_agent_id: provider.map(str::to_string),
            service_interval: "month".to_string(),
            format: "json".to_string(),
            exclude_device: None,
        }
    }

    #[test]
    fn create_body_always_embeds_device_list_even_when_empty() {
        // Degrade / all-excluded resolves to an empty set — the body must still
        // carry an explicit (empty) deviceList so routing never falls to a server default.
        let p = params_fixture(None);
        let body = super::build_create_body(&p, false, serde_json::json!({ "asp": "x" }), "0xsig", &[]);
        assert_eq!(body["deviceList"], serde_json::json!([]));
        assert!(body.get("deviceList").is_some());
        assert_eq!(body["termsSig"], serde_json::json!("0xsig"));
        assert!(body.get("providerAgentId").is_none());
    }

    #[test]
    fn create_body_carries_devices_and_provider_when_present() {
        let p = params_fixture(Some("agent-7"));
        let devices = vec!["d1".to_string(), "d2".to_string()];
        let body = super::build_create_body(&p, true, serde_json::json!({}), "0xsig", &devices);
        assert_eq!(body["deviceList"], serde_json::json!(["d1", "d2"]));
        assert_eq!(body["providerAgentId"], serde_json::json!("agent-7"));
        assert_eq!(body["useTrial"], serde_json::json!(true));
    }

    #[test]
    fn create_success_envelope_carries_degrade_marker() {
        let degraded = super::build_create_success("0xjob", "0xhash", true);
        assert_eq!(degraded["deviceRoutingDegraded"], serde_json::json!(true));
        assert_eq!(degraded["subId"], serde_json::json!("0xjob"));
        assert_eq!(degraded["txHash"], serde_json::json!("0xhash"));
        let ok = super::build_create_success("0xjob", "0xhash", false);
        assert_eq!(ok["deviceRoutingDegraded"], serde_json::json!(false));
    }
}
