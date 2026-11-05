//! Single HTTP client for every provider quota request.
//!
//! Guarantees enforced here (not by convention in each provider):
//! * **Direct connection** — system/env proxies are ignored (`no_proxy`), so a
//!   request goes straight from this machine to the provider.
//! * **HTTPS only** to an explicit allowlist of official provider hosts; any
//!   other host is refused before a socket is opened.
//! * **No redirects** — a 3xx can never bounce a credential to another host.
//! * **One attempt, short timeout** — no retry loops; the poll scheduler and
//!   the file watcher are the only things that cause another request.
//!
//! The app sends no telemetry: these quota calls are the only outbound traffic.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::{Client, RequestBuilder, Url};

/// Official hosts the app may contact, and why.
pub const ALLOWED_HOSTS: &[&str] = &[
    // Claude Code subscriber usage windows (OAuth token from ~/.claude).
    "api.anthropic.com",
    // ChatGPT / Codex plan usage (OAuth token from ~/.codex/auth.json).
    "chatgpt.com",
    // Gemini Code Assist quota (OAuth token from ~/.gemini/oauth_creds.json).
    "cloudcode-pa.googleapis.com",
    // Gemini API key validation (key from GEMINI_API_KEY / GOOGLE_API_KEY).
    "generativelanguage.googleapis.com",
    // Cursor plan usage (session token from Cursor's local state DB).
    "cursor.com",
    "www.cursor.com",
];

const TIMEOUT: Duration = Duration::from_secs(8);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);

fn client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .no_proxy()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .timeout(TIMEOUT)
                .connect_timeout(CONNECT_TIMEOUT)
                .user_agent(concat!("QuotaPulse/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn checked_url(url: &str) -> Result<Url, String> {
    let parsed = Url::parse(url).map_err(|e| format!("invalid url: {e}"))?;
    let host = parsed.host_str().unwrap_or_default();
    if parsed.scheme() != "https" || !ALLOWED_HOSTS.contains(&host) {
        return Err(format!("blocked request to non-allowlisted host {host}"));
    }
    Ok(parsed)
}

pub fn get(url: &str) -> Result<RequestBuilder, String> {
    Ok(client()?.get(checked_url(url)?))
}

pub fn post(url: &str) -> Result<RequestBuilder, String> {
    Ok(client()?.post(checked_url(url)?))
}

/// Send a request once and return the JSON body, mapping HTTP errors to a
/// short message (body truncated so tokens echoed by an error page never end
/// up in logs at length).
pub async fn send_json(req: RequestBuilder) -> Result<serde_json::Value, String> {
    let res = req.send().await.map_err(|e| format!("request failed: {e}"))?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(160).collect();
        return Err(format!("HTTP {status}: {snippet}"));
    }
    res.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("invalid JSON: {e}"))
}

/// Decode the `exp` / `sub` claims of a JWT without verifying it (we only use
/// them locally to decide whether a token is worth sending).
pub fn jwt_claims(token: &str) -> Option<serde_json::Value> {
    use base64::Engine;
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn jwt_expires_at(token: &str) -> Option<u64> {
    jwt_claims(token)?.get("exp")?.as_u64()
}

pub fn now_unix() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_official_https_hosts_are_allowed() {
        assert!(checked_url("https://api.anthropic.com/api/oauth/usage").is_ok());
        assert!(checked_url("https://cursor.com/api/usage?user=x").is_ok());
        assert!(checked_url("http://api.anthropic.com/x").is_err());
        assert!(checked_url("https://evil.example.com/x").is_err());
        assert!(checked_url("https://api.anthropic.com.evil.com/x").is_err());
    }

    #[test]
    fn decodes_jwt_exp() {
        // {"exp":1700000000,"sub":"auth0|user_1"}
        let token = "e30.eyJleHAiOjE3MDAwMDAwMDAsInN1YiI6ImF1dGgwfHVzZXJfMSJ9.sig";
        assert_eq!(jwt_expires_at(token), Some(1_700_000_000));
    }
}
