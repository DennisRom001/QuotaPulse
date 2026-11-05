use chrono::{DateTime, TimeZone, Utc};
use reqwest::header::{ACCEPT, AUTHORIZATION};

use crate::providers::http;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct CodexBearerWindow {
    pub session_used: u64,
    pub weekly_used: u64,
    pub resets_at: Option<DateTime<Utc>>,
    pub weekly_resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct UsageResponse {
    rate_limit: Option<RateLimit>,
}

#[derive(Debug, Deserialize)]
struct RateLimit {
    primary_window: Option<Window>,
    secondary_window: Option<Window>,
}

#[derive(Debug, Deserialize)]
struct Window {
    used_percent: Option<u64>,
    reset_at: Option<i64>,
}

/// ChatGPT plan usage for Codex, on OpenAI's own chatgpt.com domain (the
/// public api.openai.com has no endpoint for ChatGPT-plan quotas). One attempt,
/// no retry loop.
pub async fn fetch_usage(access_token: &str, account_id: Option<&str>) -> Result<CodexBearerWindow, String> {
    let mut req = http::get("https://chatgpt.com/backend-api/codex/usage")?
        .header(AUTHORIZATION, format!("Bearer {access_token}"))
        .header(ACCEPT, "application/json");
    if let Some(id) = account_id.filter(|v| !v.is_empty()) {
        req = req.header("chatgpt-account-id", id);
    }
    let value = http::send_json(req).await?;
    parse_response(value)
}

fn parse_response(value: serde_json::Value) -> Result<CodexBearerWindow, String> {
    let resp: UsageResponse =
        serde_json::from_value(value).map_err(|e| format!("response parse error: {e}"))?;
    let rl = resp.rate_limit.ok_or("No rate_limit in response")?;

    let primary = rl.primary_window.unwrap_or(Window {
        used_percent: Some(0),
        reset_at: None,
    });
    let secondary = rl.secondary_window.unwrap_or(Window {
        used_percent: Some(0),
        reset_at: None,
    });

    let session_pct = primary.used_percent.unwrap_or(0);
    let weekly_pct = secondary.used_percent.unwrap_or(0);

    let session_used = session_pct;
    let weekly_used = weekly_pct;

    let resets_at = primary
        .reset_at
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single());
    let weekly_resets_at = secondary
        .reset_at
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single());

    Ok(CodexBearerWindow {
        session_used,
        weekly_used,
        resets_at,
        weekly_resets_at,
    })
}
