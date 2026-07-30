//! Device routing for subscription messages (buyer side) — two commands:
//!
//! - `device-list`             — list the devices this agent is logged in on,
//!   with a CLI-derived local last-online time and a this-device marker.
//!   Paginates to completion (a dropped page would read as "not receiving",
//!   which is safety-relevant).
//! - `subscribe-device-update` — overwrite the receive-device list for one or
//!   more subscriptions (batch). The passed list wholly replaces the stored
//!   list; empty/omitted clears it.
//!
//! Both are backend-HTTP only — neither takes a `--chain` argument.
//!
//! Grounded pattern notes (source code wins over the architecture doc):
//! - The batch update POSTs via `post_with_identity` (JSON body). The
//!   `raw_post_with_identity` variant the arch named is for hand-rolled
//!   multipart bytes and unwraps `data` / errors on `code != "0"` exactly like
//!   `post_with_identity`, so it offers no envelope-preservation benefit here.
//! - The device-list GET uses `get_with_agent_id` (JWT + agenticId, no
//!   sessionCert): `get_with_identity` appends `?sessionCert=…`, which cannot
//!   coexist with the `?page=&pageSize=` query string (double `?`). The
//!   `/priapi/v5/wallet/agentic/agent/device-list` wallet endpoint authenticates
//!   with JWT + agenticId.

use anyhow::{anyhow, bail, Result};
use chrono::TimeZone;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::commands::agent_commerce::task::common::network::task_api_client::TaskApiClient;
use crate::commands::agentic_wallet::auth::ensure_tokens_refreshed;
use crate::output;

use super::create::resolve_user_agent;
use super::create_subscribe::SUBSCRIBE_API_PREFIX;

/// Wallet device-list endpoint (userId resolved from JWT — never passed).
const DEVICE_LIST_PATH: &str = "/priapi/v5/wallet/agentic/agent/device-list";
/// Max subscriptions per batch update (client pre-validation, AC-01).
const MAX_UPDATE_ITEMS: usize = 100;
/// Default page size when the caller passes `< 1`.
const DEFAULT_PAGE_SIZE: i64 = 20;
/// Hard safety cap on pagination rounds (a buggy backend must not loop forever).
const MAX_PAGES: i64 = 10_000;

// ─── ms → local time helper (device lastOnlineTime is milliseconds) ─────────

/// Format a Unix-**milliseconds** timestamp to a local wall-clock string for
/// display. Mirrors `evaluator::my_stake::fmt_unix_seconds` but for ms — a
/// seconds misread lands in the wrong year (AC-06). Three sentinel rules:
/// `0 → "0"`; parseable → `"%Y-%m-%d %H:%M:%S %Z"`; unparseable →
/// `"{ts_ms} (unparseable)"`.
fn fmt_unix_millis(ts_ms: i64) -> String {
    if ts_ms == 0 {
        "0".to_string()
    } else if let Some(dt) = chrono::Local.timestamp_millis_opt(ts_ms).single() {
        dt.format("%Y-%m-%d %H:%M:%S %Z").to_string()
    } else {
        format!("{ts_ms} (unparseable)")
    }
}

// ─── device-list wire + output shapes ───────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceRow {
    #[serde(default)]
    device_id: String,
    #[serde(default)]
    device_name: String,
    /// Unix **milliseconds** — never seconds.
    #[serde(default)]
    last_online_time: i64,
}

/// A decoded device page. Only `list` + `total` are consumed: the echoed
/// `page`/`pageSize` reflect the request inputs (CLI spec), not the backend's,
/// so those wire fields are intentionally not modelled here.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DevicePage {
    #[serde(default)]
    list: Vec<DeviceRow>,
    #[serde(default)]
    total: i64,
}

/// CLI-derived, ready-to-print device row — the skill never re-formats it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceOut {
    device_id: String,
    device_name: String,
    last_online_time: i64,
    last_online_local: String,
    is_this_device: bool,
}

/// Decode the dual-envelope `data`: a bare object OR a single-element array
/// (production wraps it). An empty array is "no such page" ⇒ empty page.
fn decode_device_page(data: Value) -> Result<DevicePage> {
    match data {
        Value::Array(arr) => match arr.into_iter().next() {
            Some(first) => serde_json::from_value(first)
                .map_err(|e| anyhow!("failed to parse device page: {e}")),
            None => Ok(DevicePage::default()),
        },
        Value::Null => Ok(DevicePage::default()),
        other => {
            serde_json::from_value(other).map_err(|e| anyhow!("failed to parse device page: {e}"))
        }
    }
}

/// Fetch and aggregate **all** pages. Normalizes `page < 1 → 1` and
/// `page_size < 1 → 20`; `page_size > 100` is passed through (backend returns
/// error `81001`). Loops until a page returns fewer rows than requested, an
/// empty page, or the accumulated count reaches `total`.
async fn fetch_all_devices(
    client: &mut TaskApiClient,
    agent_id: &str,
    page: i64,
    page_size: i64,
) -> Result<DevicePage> {
    let start_page = if page < 1 { 1 } else { page };
    let norm_size = if page_size < 1 {
        DEFAULT_PAGE_SIZE
    } else {
        page_size
    };

    let mut acc: Vec<DeviceRow> = Vec::new();
    let total: i64;
    let mut cur = start_page;
    loop {
        let path = format!("{DEVICE_LIST_PATH}?page={cur}&pageSize={norm_size}");
        let data = client.get_with_agent_id(&path, agent_id).await?;
        let dpage = decode_device_page(data)?;
        let page_total = dpage.total;
        let got = dpage.list.len() as i64;
        acc.extend(dpage.list);

        // Stop on an empty page, a short (final) page, once the accumulated
        // count reaches `total`, or at the hard safety cap.
        let reached_total = page_total > 0 && (acc.len() as i64) >= page_total;
        if got == 0 || got < norm_size || reached_total || cur - start_page >= MAX_PAGES {
            total = page_total;
            break;
        }
        cur += 1;
    }
    Ok(DevicePage { list: acc, total })
}

/// Fetch every logged-in device id (paginated to completion). Reuse convenience
/// for `create-subscribe`'s default all-devices routing set — NOT an MCP `fetch_*`
/// delegate. The caller decides how to handle an error / empty result (degrade).
pub(crate) async fn fetch_all_device_ids(
    client: &mut TaskApiClient,
    agent_id: &str,
) -> Result<Vec<String>> {
    let aggregated = fetch_all_devices(client, agent_id, 1, DEFAULT_PAGE_SIZE).await?;
    Ok(aggregated
        .list
        .into_iter()
        .map(|r| r.device_id)
        .filter(|id| !id.is_empty())
        .collect())
}

/// Resolve `create-subscribe`'s `deviceList` + `deviceRoutingDegraded` flag:
/// - fetch succeeded with ≥ 1 device ⇒ all fetched ids minus `excluded`, not degraded;
/// - fetch failed (`None`) or returned no devices ⇒ **this device only**, degraded.
///
/// An unresolved this-device id in the degrade branch yields an empty list (still
/// degraded) — the create flow must not abort (§4.4).
pub(crate) fn resolve_create_device_set(
    fetched: Option<Vec<String>>,
    excluded: &[String],
    this_device_id: Option<&str>,
) -> (Vec<String>, bool) {
    match fetched {
        Some(ids) if !ids.is_empty() => {
            let kept = ids
                .into_iter()
                .filter(|id| !excluded.iter().any(|e| e == id))
                .collect();
            (kept, false)
        }
        _ => (
            this_device_id
                .map(|id| vec![id.to_string()])
                .unwrap_or_default(),
            true,
        ),
    }
}

/// `device-list` handler — full emit. Empty page / no devices ⇒ `success` with
/// `list: []`, `total: 0` (NOT an error). Transport / endpoint-unavailable
/// (endpoint not live in production yet) propagates as `output::error` (exit 1)
/// — the degraded path is a first-class deliverable.
pub async fn handle_device_list(
    client: &mut TaskApiClient,
    page: i64,
    page_size: i64,
) -> Result<()> {
    ensure_tokens_refreshed()
        .await
        .map_err(|e| anyhow!("session has expired; run `onchainos wallet login` first: {e}"))?;
    let (agent_id, _) = resolve_user_agent().await?;

    let aggregated = fetch_all_devices(client, &agent_id, page, page_size).await?;
    let this_id = crate::device::id::get_cached_device_id();

    let list: Vec<DeviceOut> = aggregated
        .list
        .iter()
        .map(|row| DeviceOut {
            device_id: row.device_id.clone(),
            device_name: row.device_name.clone(),
            last_online_time: row.last_online_time,
            last_online_local: fmt_unix_millis(row.last_online_time),
            is_this_device: this_id.is_some_and(|id| id == row.device_id.as_str()),
        })
        .collect();

    let echoed_page = if page < 1 { 1 } else { page };
    let echoed_size = if page_size < 1 {
        DEFAULT_PAGE_SIZE
    } else {
        page_size
    };

    output::success(json!({
        "list": list,
        "total": aggregated.total,
        "page": echoed_page,
        "pageSize": echoed_size,
        "thisDeviceId": this_id,
    }));
    Ok(())
}

// ─── subscribe-device-update ────────────────────────────────────────────────

/// One subscription's overwrite target. `device_list` empty ⇒ that subscription
/// receives on no device (clear).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateItem {
    job_id: String,
    #[serde(default)]
    device_list: Vec<String>,
}

/// Split a comma-separated device-id list; blanks are dropped. `None` / empty ⇒
/// empty vec (clear).
fn parse_csv_devices(csv: Option<&str>) -> Vec<String> {
    csv.map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Normalize Form A (`--job-id` + `--device-list` csv) and Form B (`--items`
/// JSON) into one `items` array. Form B wins when both are supplied.
fn normalize_items(
    job_id: Option<&str>,
    device_list: Option<&str>,
    items: Option<&str>,
) -> Result<Vec<UpdateItem>> {
    if let Some(items_json) = items {
        let parsed: Vec<UpdateItem> = serde_json::from_str(items_json).map_err(|e| {
            anyhow!("--items must be a JSON array of {{jobId, deviceList}} objects: {e}")
        })?;
        Ok(parsed)
    } else {
        let job_id = job_id
            .ok_or_else(|| anyhow!("either --job-id (form A) or --items (form B) is required"))?;
        if job_id.is_empty() {
            bail!("--job-id must not be empty");
        }
        Ok(vec![UpdateItem {
            job_id: job_id.to_string(),
            device_list: parse_csv_devices(device_list),
        }])
    }
}

/// Client pre-validation: the resolved `items` array must be non-empty and
/// `len <= 100` (AC-01 boundaries 0 / 1 / 100 / 101).
fn validate_items_len(len: usize) -> Result<()> {
    if len == 0 {
        bail!("no subscriptions to update: provide --job-id or a non-empty --items array");
    }
    if len > MAX_UPDATE_ITEMS {
        bail!("too many items ({len}); at most {MAX_UPDATE_ITEMS} subscriptions per batch");
    }
    Ok(())
}

fn build_items_array(items: &[UpdateItem]) -> Vec<Value> {
    items
        .iter()
        .map(|it| json!({ "jobId": it.job_id, "deviceList": it.device_list }))
        .collect()
}

/// Byte-literal request body `{ "items": [ { "jobId", "deviceList": [...] } ] }`.
fn build_update_body(items: &[UpdateItem]) -> Value {
    json!({ "items": build_items_array(items) })
}

/// Success iff the backend `data` is boolean `true`. Any other shape (object,
/// null, `"true"` string) is a failure whose raw body is echoed.
fn is_update_success(data: &Value) -> bool {
    *data == Value::Bool(true)
}

/// `subscribe-device-update` handler. Client-validates locally (0 / >100 items
/// send no request), then POSTs the byte-literal body and asserts `data == true`.
pub async fn handle_subscribe_device_update(
    client: &mut TaskApiClient,
    job_id: Option<&str>,
    device_list: Option<&str>,
    items: Option<&str>,
) -> Result<()> {
    if items.is_some() && (job_id.is_some() || device_list.is_some()) {
        eprintln!(
            "[subscribe-device-update] both --items and --job-id/--device-list provided; \
             --items (form B) takes precedence"
        );
    }

    // Client pre-validation before any request (AC-01): resolve + bound-check.
    let normalized = normalize_items(job_id, device_list, items)?;
    validate_items_len(normalized.len())?;

    ensure_tokens_refreshed()
        .await
        .map_err(|e| anyhow!("session has expired; run `onchainos wallet login` first: {e}"))?;
    let (user_agent_id, _) = resolve_user_agent().await?;

    let body = build_update_body(&normalized);
    let path = format!("{SUBSCRIBE_API_PREFIX}/device/batchUpdate");
    let resp = client
        .post_with_identity(&path, &body, &user_agent_id)
        .await
        .map_err(|e| anyhow!("subscribe-device-update failed: {e}"))?;

    if is_update_success(&resp) {
        // Echo what was written so the skill re-renders without a second fetch.
        output::success(json!({ "updated": build_items_array(&normalized) }));
        Ok(())
    } else {
        // HTTP 200 + code "0" but data != true — echo the raw body verbatim.
        bail!(
            "subscribe-device-update failed: backend did not confirm the update (data != true): {}",
            serde_json::to_string(&resp).unwrap_or_else(|_| resp.to_string())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── fmt_unix_millis (AC-06) ──────────────────────────────────────────
    #[test]
    fn fmt_unix_millis_zero_sentinel() {
        assert_eq!(fmt_unix_millis(0), "0");
    }

    #[test]
    fn fmt_unix_millis_unparseable_sentinel() {
        let out = fmt_unix_millis(i64::MAX);
        assert!(
            out.contains("unparseable"),
            "expected unparseable sentinel: {out}"
        );
    }

    #[test]
    fn fmt_unix_millis_uses_milliseconds_not_seconds() {
        // 1_784_620_000_000 ms → 2026 (UTC 2026-07-19); local offsets keep the year.
        let ms = 1_784_620_000_000i64;
        let as_ms = fmt_unix_millis(ms);
        assert!(
            as_ms.contains("2026"),
            "ms must format to year 2026: {as_ms}"
        );
        // RED assertion: reading the same integer as *seconds* lands in the wrong
        // (far-future) year — proving the helper is millisecond-based.
        if let Some(dt) = chrono::Local.timestamp_opt(ms, 0).single() {
            assert_ne!(
                dt.format("%Y").to_string(),
                "2026",
                "a seconds misread must NOT resolve to 2026"
            );
        }
    }

    // ── decode_device_page: three envelope shapes (AC-06) ────────────────
    #[test]
    fn decode_bare_object() {
        let obj = json!({
            "list": [{ "deviceId": "d1", "deviceName": "Phone", "lastOnlineTime": 1_784_620_000_000i64 }],
            "total": 1, "page": 1, "pageSize": 20
        });
        let p = decode_device_page(obj).unwrap();
        assert_eq!(p.list.len(), 1);
        assert_eq!(p.list[0].device_id, "d1");
        assert_eq!(p.list[0].last_online_time, 1_784_620_000_000);
        assert_eq!(p.total, 1);
    }

    #[test]
    fn decode_single_element_array() {
        let arr = json!([{ "list": [{ "deviceId": "d2" }], "total": 1 }]);
        let p = decode_device_page(arr).unwrap();
        assert_eq!(p.list.len(), 1);
        assert_eq!(p.list[0].device_id, "d2");
    }

    #[test]
    fn decode_empty_array_is_empty_page() {
        let p = decode_device_page(json!([])).unwrap();
        assert!(p.list.is_empty());
        assert_eq!(p.total, 0);
    }

    // ── subscribe-device-update normalization + body (AC-01) ─────────────
    #[test]
    fn normalize_form_a_csv() {
        let items = normalize_items(Some("0xjob"), Some("d1, d2 ,,d3"), None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].job_id, "0xjob");
        assert_eq!(items[0].device_list, vec!["d1", "d2", "d3"]);
    }

    #[test]
    fn normalize_form_a_omitted_device_list_clears() {
        let items = normalize_items(Some("0xjob"), None, None).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].device_list.is_empty());
    }

    #[test]
    fn normalize_form_b_wins_over_form_a() {
        let items = normalize_items(
            Some("0xA"),
            Some("ignored"),
            Some(r#"[{"jobId":"0xB","deviceList":["d9"]}]"#),
        )
        .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].job_id, "0xB");
        assert_eq!(items[0].device_list, vec!["d9"]);
    }

    #[test]
    fn normalize_requires_job_id_or_items() {
        assert!(normalize_items(None, Some("d1"), None).is_err());
    }

    #[test]
    fn build_body_is_byte_literal_items_shape() {
        let items = vec![UpdateItem {
            job_id: "0x..".to_string(),
            device_list: vec!["device1".to_string(), "device2".to_string()],
        }];
        let body = build_update_body(&items);
        assert_eq!(
            body,
            json!({ "items": [ { "jobId": "0x..", "deviceList": ["device1", "device2"] } ] })
        );
    }

    #[test]
    fn validate_item_count_boundaries_0_1_100_101() {
        assert!(validate_items_len(0).is_err()); // 0 → local error
        assert!(validate_items_len(1).is_ok()); // 1 → ok
        assert!(validate_items_len(100).is_ok()); // 100 → ok
        assert!(validate_items_len(101).is_err()); // 101 → local error
    }

    #[test]
    fn only_boolean_true_is_success() {
        assert!(is_update_success(&json!(true)));
        assert!(!is_update_success(&json!(false)));
        assert!(!is_update_success(&json!("true")));
        assert!(!is_update_success(&json!({ "updated": 1 })));
        assert!(!is_update_success(&Value::Null));
    }

    #[test]
    fn form_b_zero_items_fails_validation() {
        // `--items '[]'` resolves to an empty array → 0-item boundary.
        let items = normalize_items(None, None, Some("[]")).unwrap();
        assert!(validate_items_len(items.len()).is_err());
    }

    // ── create-subscribe device set resolution (AC-02) ───────────────────
    #[test]
    fn create_device_set_default_all_devices() {
        let fetched = Some(vec!["d1".to_string(), "d2".to_string(), "d3".to_string()]);
        let (list, degraded) = resolve_create_device_set(fetched, &[], Some("d2"));
        assert_eq!(list, vec!["d1", "d2", "d3"]);
        assert!(!degraded);
    }

    #[test]
    fn create_device_set_excludes_named_devices() {
        let fetched = Some(vec!["d1".to_string(), "d2".to_string(), "d3".to_string()]);
        let excluded = vec!["d2".to_string()];
        let (list, degraded) = resolve_create_device_set(fetched, &excluded, Some("d1"));
        assert_eq!(list, vec!["d1", "d3"]);
        assert!(!degraded); // exclusion is a user choice, not a degrade
    }

    #[test]
    fn create_device_set_degrades_to_this_device_on_fetch_failure() {
        // Fetch failed (None) → this-device only, degraded.
        let (list, degraded) = resolve_create_device_set(None, &[], Some("dME"));
        assert_eq!(list, vec!["dME"]);
        assert!(degraded);
    }

    #[test]
    fn create_device_set_degrades_on_empty_fetch() {
        // Fetch succeeded but returned no devices → degrade too.
        let (list, degraded) = resolve_create_device_set(Some(vec![]), &[], Some("dME"));
        assert_eq!(list, vec!["dME"]);
        assert!(degraded);
    }

    #[test]
    fn create_device_set_degrade_with_unresolved_this_device_is_empty() {
        let (list, degraded) = resolve_create_device_set(None, &[], None);
        assert!(list.is_empty());
        assert!(degraded); // still degraded; create must not abort
    }
}
