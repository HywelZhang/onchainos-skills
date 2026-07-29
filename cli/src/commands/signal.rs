use anyhow::Result;
use clap::Subcommand;
use serde_json::{json, Value};

use super::Context;
use crate::client::ApiClient;
use crate::commands::sink::CodedError;
use crate::output;
use crate::trade_signal::{self, InputFormat, ParseError};

/// Parse mode for the hidden `signal parse` diagnostic.
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum ParseMode {
    /// Detect the input format, then parse accordingly.
    Auto,
    /// Force `parse_signal_text` (bare V1.1 signal text).
    Text,
    /// Force `parse_envelope` (V2 wire envelope JSON).
    Envelope,
    /// Only run `detect_format` and return the classification.
    Detect,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum SignalCommand {
    /// Get supported chains for market signals
    Chains,
    /// Get latest signal list (smart money / KOL / whale activity)
    List {
        /// Chain (e.g. ethereum, solana, base). Required.
        #[arg(long)]
        chain: String,
        /// Wallet type filter: 1=Smart Money, 2=KOL/Influencer, 3=Whales (comma-separated, e.g. "1,2")
        #[arg(long)]
        wallet_type: Option<String>,
        /// Minimum transaction amount in USD
        #[arg(long)]
        min_amount_usd: Option<String>,
        /// Maximum transaction amount in USD
        #[arg(long)]
        max_amount_usd: Option<String>,
        /// Minimum triggering wallet address count
        #[arg(long)]
        min_address_count: Option<String>,
        /// Maximum triggering wallet address count
        #[arg(long)]
        max_address_count: Option<String>,
        /// Token contract address (filter signals for a specific token)
        #[arg(long)]
        token_address: Option<String>,
        /// Minimum token market cap in USD
        #[arg(long)]
        min_market_cap_usd: Option<String>,
        /// Maximum token market cap in USD
        #[arg(long)]
        max_market_cap_usd: Option<String>,
        /// Minimum token liquidity in USD
        #[arg(long)]
        min_liquidity_usd: Option<String>,
        /// Maximum token liquidity in USD
        #[arg(long)]
        max_liquidity_usd: Option<String>,
        /// Number of results per page (default: 20, max: 100)
        #[arg(long)]
        limit: Option<String>,
        /// Pagination cursor — pass the cursor from the last item of the previous page; omit for first page
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Diagnostic: parse a V1.1 trade-signal text or V2 envelope into typed JSON.
    /// Internal/eval use only — the trade-signal parser diagnostic, distinct from
    /// the market-signal `Chains`/`List` verbs above.
    #[command(hide = true)]
    Parse {
        /// Raw input: a bare signalText (【…】…) or a V2 envelope JSON ({…}).
        #[arg(long)]
        text: String,
        /// Parse mode: auto (detect+parse) | text | envelope | detect (classify only).
        #[arg(long, value_enum, default_value_t = ParseMode::Auto)]
        mode: ParseMode,
    },
}

pub async fn execute(ctx: &Context, cmd: SignalCommand) -> Result<()> {
    match cmd {
        SignalCommand::Chains => signal_chains(ctx).await,
        SignalCommand::Parse { text, mode } => signal_parse(&text, mode),
        SignalCommand::List {
            chain,
            wallet_type,
            min_amount_usd,
            max_amount_usd,
            min_address_count,
            max_address_count,
            token_address,
            min_market_cap_usd,
            max_market_cap_usd,
            min_liquidity_usd,
            max_liquidity_usd,
            limit,
            cursor,
        } => {
            signal_list(
                ctx,
                &chain,
                wallet_type,
                min_amount_usd,
                max_amount_usd,
                min_address_count,
                max_address_count,
                token_address,
                min_market_cap_usd,
                max_market_cap_usd,
                min_liquidity_usd,
                max_liquidity_usd,
                limit,
                cursor,
            )
            .await
        }
    }
}

// ── Public fetch functions (used by both CLI and MCP) ────────────────

/// GET /api/v6/dex/market/signal/supported/chain
pub async fn fetch_chains(client: &mut ApiClient) -> Result<Value> {
    client
        .get("/api/v6/dex/market/signal/supported/chain", &[])
        .await
}

/// POST /api/v6/dex/market/signal/list — smart money / KOL / whale signals
#[allow(clippy::too_many_arguments)]
pub async fn fetch_list(
    client: &mut ApiClient,
    chain_index: &str,
    wallet_type: Option<String>,
    min_amount_usd: Option<String>,
    max_amount_usd: Option<String>,
    min_address_count: Option<String>,
    max_address_count: Option<String>,
    token_address: Option<String>,
    min_market_cap_usd: Option<String>,
    max_market_cap_usd: Option<String>,
    min_liquidity_usd: Option<String>,
    max_liquidity_usd: Option<String>,
    limit: Option<String>,
    cursor: Option<String>,
) -> Result<Value> {
    if let Some(ref s) = limit {
        let n: u64 = s
            .parse()
            .map_err(|_| anyhow::anyhow!("--limit must be a number between 1 and 100"))?;
        anyhow::ensure!(
            (1..=100).contains(&n),
            "--limit must be between 1 and 100, got {n}"
        );
    }
    let mut body = json!({
        "chainIndex": chain_index,
        "limit": limit.as_deref().unwrap_or("20"),
    });
    let obj = body.as_object_mut().unwrap();
    if let Some(v) = cursor {
        obj.insert("cursor".into(), Value::String(v));
    }
    if let Some(v) = wallet_type {
        obj.insert("walletType".into(), Value::String(v));
    }
    if let Some(v) = min_amount_usd {
        obj.insert("minAmountUsd".into(), Value::String(v));
    }
    if let Some(v) = max_amount_usd {
        obj.insert("maxAmountUsd".into(), Value::String(v));
    }
    if let Some(v) = min_address_count {
        obj.insert("minAddressCount".into(), Value::String(v));
    }
    if let Some(v) = max_address_count {
        obj.insert("maxAddressCount".into(), Value::String(v));
    }
    if let Some(v) = token_address {
        obj.insert("tokenAddress".into(), Value::String(v));
    }
    if let Some(v) = min_market_cap_usd {
        obj.insert("minMarketCapUsd".into(), Value::String(v));
    }
    if let Some(v) = max_market_cap_usd {
        obj.insert("maxMarketCapUsd".into(), Value::String(v));
    }
    if let Some(v) = min_liquidity_usd {
        obj.insert("minLiquidityUsd".into(), Value::String(v));
    }
    if let Some(v) = max_liquidity_usd {
        obj.insert("maxLiquidityUsd".into(), Value::String(v));
    }
    client.post("/api/v6/dex/market/signal/list", &body).await
}

// ── CLI wrappers ─────────────────────────────────────────────────────

async fn signal_chains(ctx: &Context) -> Result<()> {
    let mut client = ctx.client_async().await?;
    output::success(fetch_chains(&mut client).await?);
    Ok(())
}

/// Hidden diagnostic: run the trade-signal parser core and emit the standard
/// envelope. Pure-local (no `ctx`, no network); `async`-compatible only to match
/// the `execute` signature. On `ParseError` it returns a `CodedError` so `main.rs`
/// renders `{ok:false,error,errorCode,errorField?}` and exits 1 (SR-3: never
/// echoes the raw input). `--mode detect` returns the format classification.
fn signal_parse(text: &str, mode: ParseMode) -> Result<()> {
    let parsed = match mode {
        ParseMode::Detect => {
            output::success(json!({ "format": trade_signal::detect_format(text) }));
            return Ok(());
        }
        ParseMode::Text => trade_signal::parse_signal_text(text),
        ParseMode::Envelope => trade_signal::parse_envelope(text),
        ParseMode::Auto => match trade_signal::detect_format(text) {
            InputFormat::V2Text => trade_signal::parse_signal_text(text),
            InputFormat::V1JsonSchema => trade_signal::parse_envelope(text),
            InputFormat::Unsupported => Err(ParseError::UnsupportedFormat),
        },
    };
    match parsed {
        Ok(signal) => {
            output::success(signal);
            Ok(())
        }
        Err(e) => Err(CodedError::new(e.code(), e.field(), e.message()).into()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn signal_list(
    ctx: &Context,
    chain: &str,
    wallet_type: Option<String>,
    min_amount_usd: Option<String>,
    max_amount_usd: Option<String>,
    min_address_count: Option<String>,
    max_address_count: Option<String>,
    token_address: Option<String>,
    min_market_cap_usd: Option<String>,
    max_market_cap_usd: Option<String>,
    min_liquidity_usd: Option<String>,
    max_liquidity_usd: Option<String>,
    limit: Option<String>,
    cursor: Option<String>,
) -> Result<()> {
    let chain_index = crate::chains::resolve_chain(chain).to_string();
    let mut client = ctx.client_async().await?;
    output::success(
        fetch_list(
            &mut client,
            &chain_index,
            wallet_type,
            min_amount_usd,
            max_amount_usd,
            min_address_count,
            max_address_count,
            token_address,
            min_market_cap_usd,
            max_market_cap_usd,
            min_liquidity_usd,
            max_liquidity_usd,
            limit,
            cursor,
        )
        .await?,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal valid V1.1 signal text and its V2 envelope wrapper (mirrors the
    // parser corpus in `trade_signal/tests.rs`).
    const VALID_TEXT: &str =
        "【SPOT】market:BTC/USDT|symbol:BTC|side:BUY|price:60000-65000|position:5%|ttl:1h";
    const VALID_ENVELOPE: &str = "{\"schemaVersion\":2,\"deliveryId\":\"abc123\",\"signalTime\":1,\"signalText\":\"【SPOT】market:BTC/USDT|symbol:BTC|side:BUY|price:60000-65000|position:5%|ttl:1h\"}";

    /// Extract the `(errorCode, errorField)` the handler surfaces on the exit-1
    /// path — i.e. the `ParseError → CodedError` mapping that `main.rs` renders
    /// via `output::error_coded` and exits 1 (Decision #2 / SR-3).
    fn err_code(res: Result<()>) -> (String, Option<String>) {
        let e = res.expect_err("handler should return Err on a bad parse");
        let c = e
            .downcast_ref::<CodedError>()
            .expect("error must downcast to CodedError (exit-1 coded path)");
        (c.code.clone(), c.field.clone())
    }

    // ── Success (exit 0) — every mode routes to the right entry point ──────────

    #[test]
    fn auto_mode_parses_valid_text() {
        assert!(signal_parse(VALID_TEXT, ParseMode::Auto).is_ok());
    }

    #[test]
    fn auto_mode_parses_valid_envelope() {
        assert!(signal_parse(VALID_ENVELOPE, ParseMode::Auto).is_ok());
    }

    #[test]
    fn text_mode_forces_signal_text_parse() {
        assert!(signal_parse(VALID_TEXT, ParseMode::Text).is_ok());
    }

    #[test]
    fn envelope_mode_forces_envelope_parse() {
        assert!(signal_parse(VALID_ENVELOPE, ParseMode::Envelope).is_ok());
    }

    #[test]
    fn detect_mode_never_errors() {
        // `detect` only classifies — it returns Ok (exit 0) for every shape,
        // including inputs that would fail a real parse.
        assert!(signal_parse(VALID_TEXT, ParseMode::Detect).is_ok());
        assert!(signal_parse(VALID_ENVELOPE, ParseMode::Detect).is_ok());
        assert!(signal_parse("", ParseMode::Detect).is_ok());
        assert!(signal_parse("garbage", ParseMode::Detect).is_ok());
    }

    // ── Failure (exit 1) — coded-error contract ────────────────────────────────

    #[test]
    fn auto_mode_empty_input_is_unsupported_format() {
        // Empty input under Auto is classified `Unsupported` first, so the
        // handler emits `UnsupportedFormat` (not `EmptyInput`).
        let (code, field) = err_code(signal_parse("", ParseMode::Auto));
        assert_eq!(code, "UnsupportedFormat");
        assert_eq!(field, None);
    }

    #[test]
    fn known_bad_input_maps_to_expected_code_and_field() {
        // position 0% → OutOfRange with the stable `range` field name.
        let bad =
            "【SPOT】market:BTC/USDT|symbol:BTC|side:BUY|price:60000-65000|position:0%|ttl:1h";
        let (code, field) = err_code(signal_parse(bad, ParseMode::Text));
        assert_eq!(code, "OutOfRange");
        assert_eq!(field.as_deref(), Some("range"));
    }

    #[test]
    fn option_mismatch_maps_to_contract_code_field() {
        // A second point on the ParseError → CodedError mapping: the C/P vs
        // optionType mismatch surfaces `OptionFieldMismatch` + `contractCode`.
        let bad = "【OPTION】contractCode:BTC-251231-60000-C|side:Buy|optionType:Put|strike:60000|expiry:2025-12-31|premiumCap:1500|position:5%|ttl:5d";
        let (code, field) = err_code(signal_parse(bad, ParseMode::Text));
        assert_eq!(code, "OptionFieldMismatch");
        assert_eq!(field.as_deref(), Some("contractCode"));
    }

    #[test]
    fn envelope_mode_invalid_schema_is_invalid_envelope() {
        let bad =
            "{\"schemaVersion\":1,\"deliveryId\":\"abc123\",\"signalTime\":1,\"signalText\":\"x\"}";
        let (code, field) = err_code(signal_parse(bad, ParseMode::Envelope));
        assert_eq!(code, "InvalidEnvelope");
        assert_eq!(field.as_deref(), Some("envelope"));
    }
}
