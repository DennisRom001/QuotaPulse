//! Locate Claude Code's OAuth credentials on this machine.
//!
//! Sources, all owned by the Claude Code CLI itself (read-only here):
//! 1. `CLAUDE_CODE_OAUTH_TOKEN` environment variable — the long-lived token
//!    printed by `claude setup-token`, which Claude Code also honours.
//! 2. The OS credential store entry `Claude Code-credentials` (macOS
//!    Keychain / Windows Credential Manager / Linux Secret Service), where
//!    some Claude Code builds keep their login.
//! 3. `.credentials.json` in `$CLAUDE_CONFIG_DIR`, `~/.claude` or
//!    `~/.config/claude` (the default on Windows and Linux).
//!
//! When several exist, the first one that is not expired wins; if all are
//! expired the most recent is returned so the UI can say "auth expired".
//! Tokens are never written back or refreshed, and are only sent to
//! api.anthropic.com.

use std::path::PathBuf;

use serde_json::Value;

use crate::providers::http::now_unix;

const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

#[derive(Clone)]
pub struct ClaudeCreds {
    pub access_token: String,
    pub has_refresh_token: bool,
    /// Unix seconds.
    pub expires_at: Option<u64>,
    pub source_path: String,
    /// `subscriptionType` saved by Claude Code (e.g. "pro", "max").
    pub subscription: Option<String>,
}

impl ClaudeCreds {
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| exp <= now_unix() + 30)
    }
}

fn config_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(custom) = std::env::var("CLAUDE_CONFIG_DIR") {
        for part in custom.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            roots.push(PathBuf::from(part));
        }
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".claude"));
        roots.push(home.join(".config").join("claude"));
    }
    roots
}

fn pick<'a>(json: &'a Value, paths: &[&[&str]]) -> Option<&'a Value> {
    paths.iter().find_map(|path| {
        let mut cur = json;
        for key in *path {
            cur = cur.get(*key)?;
        }
        Some(cur)
    })
}

/// Parse the JSON blob Claude Code stores (same shape in the file and in the
/// OS credential store).
fn parse_creds(raw: &str, source: String) -> Option<ClaudeCreds> {
    let json = serde_json::from_str::<Value>(raw.trim()).ok()?;
    let access_token = pick(
        &json,
        &[&["claudeAiOauth", "accessToken"], &["accessToken"], &["access_token"]],
    )
    .and_then(Value::as_str)?
    .trim()
    .to_string();
    if access_token.is_empty() {
        return None;
    }
    let has_refresh_token = pick(
        &json,
        &[&["claudeAiOauth", "refreshToken"], &["refreshToken"], &["refresh_token"]],
    )
    .and_then(Value::as_str)
    .is_some_and(|v| !v.trim().is_empty());
    // Claude Code stores expiresAt in milliseconds.
    let expires_at = pick(&json, &[&["claudeAiOauth", "expiresAt"], &["expiresAt"]])
        .and_then(Value::as_u64)
        .map(|v| if v > 10_000_000_000 { v / 1000 } else { v });
    let subscription = pick(
        &json,
        &[&["claudeAiOauth", "subscriptionType"], &["subscriptionType"]],
    )
    .and_then(Value::as_str)
    .filter(|v| !v.trim().is_empty())
    .map(str::to_string);
    Some(ClaudeCreds {
        access_token,
        has_refresh_token,
        expires_at,
        source_path: source,
        subscription,
    })
}

/// A Claude Code login that Claude Code itself ended: the file/entry is still
/// there, but the tokens were blanked (e.g. `loginEnded: "time_limit"` when
/// an organisation's maximum session length is reached). Nothing can renew
/// it except signing in again.
#[derive(Debug, Clone, PartialEq)]
pub struct EndedSession {
    pub source_path: String,
    /// `loginEnded` as written by Claude Code, e.g. "time_limit".
    pub reason: Option<String>,
    /// When the refresh token stopped being valid (unix seconds).
    pub refresh_expired_at: Option<u64>,
    pub subscription: Option<String>,
}

fn parse_ended(raw: &str, source: String) -> Option<EndedSession> {
    let json = serde_json::from_str::<Value>(raw.trim()).ok()?;
    let oauth = json.get("claudeAiOauth")?;
    let token = oauth.get("accessToken").and_then(Value::as_str).unwrap_or("");
    if !token.trim().is_empty() {
        return None;
    }
    let str_field = |k: &str| {
        oauth
            .get(k)
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string)
    };
    Some(EndedSession {
        source_path: source,
        reason: str_field("loginEnded"),
        refresh_expired_at: oauth
            .get("refreshTokenExpiresAt")
            .and_then(Value::as_u64)
            .map(|v| if v > 10_000_000_000 { v / 1000 } else { v }),
        subscription: str_field("subscriptionType"),
    })
}

/// The ended Claude Code session, if that is why no token is available.
pub fn ended_session() -> Option<EndedSession> {
    let from_store = os_store_blobs()
        .into_iter()
        .find_map(|raw| parse_ended(&raw, format!("OS credential store: {KEYCHAIN_SERVICE}")));
    from_store.or_else(|| {
        config_roots().into_iter().find_map(|root| {
            let path = root.join(".credentials.json");
            let raw = std::fs::read_to_string(&path).ok()?;
            parse_ended(&raw, path.display().to_string())
        })
    })
}

const ENV_TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// `CLAUDE_CODE_OAUTH_TOKEN`, also when set with `setx` after launch.
fn from_env() -> Option<ClaudeCreds> {
    let (token, origin) = crate::providers::env::lookup(ENV_TOKEN)?;
    let source = format!("{origin} {ENV_TOKEN}");
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    Some(ClaudeCreds {
        access_token: token.to_string(),
        has_refresh_token: false,
        expires_at: None,
        source_path: source,
        subscription: None,
    })
}

fn from_os_store() -> Vec<ClaudeCreds> {
    os_store_blobs()
        .iter()
        .filter_map(|secret| parse_creds(secret, format!("OS credential store: {KEYCHAIN_SERVICE}")))
        .collect()
}

/// Raw JSON blobs of the `Claude Code-credentials` entry.
fn os_store_blobs() -> Vec<String> {
    // Claude Code has used both the user name and the service name as the
    // account of this entry, so try both.
    let mut accounts = vec![KEYCHAIN_SERVICE.to_string()];
    for var in ["USERNAME", "USER"] {
        if let Ok(user) = std::env::var(var) {
            if !user.trim().is_empty() && !accounts.contains(&user) {
                accounts.push(user);
            }
        }
    }
    accounts
        .iter()
        .filter_map(|account| {
            let entry = keyring::Entry::new(KEYCHAIN_SERVICE, account).ok()?;
            entry.get_password().ok()
        })
        .collect()
}

fn from_files() -> Vec<ClaudeCreds> {
    config_roots()
        .into_iter()
        .filter_map(|root| {
            let path = root.join(".credentials.json");
            let raw = std::fs::read_to_string(&path).ok()?;
            parse_creds(&raw, path.display().to_string())
        })
        .collect()
}

/// Every non-expired credential in priority order (env token, OS store,
/// files), without duplicates. The connector tries them in turn, so a token
/// the usage API rejects (e.g. a `setup-token` token without the needed
/// scope) never hides a working Claude Code login.
pub fn read_valid_candidates() -> Vec<ClaudeCreds> {
    let mut out: Vec<ClaudeCreds> = Vec::new();
    for c in from_env().into_iter().chain(from_os_store()).chain(from_files()) {
        if !c.is_expired() && !out.iter().any(|o| o.access_token == c.access_token) {
            out.push(c);
        }
    }
    out
}

pub fn read_credentials() -> Option<ClaudeCreds> {
    let mut all: Vec<ClaudeCreds> = Vec::new();
    all.extend(from_env());
    all.extend(from_os_store());
    all.extend(from_files());

    if let Some(fresh) = all.iter().find(|c| !c.is_expired()) {
        // Keep the plan label even if the fresh token came from a source
        // that does not record it (e.g. the env token).
        let mut chosen = fresh.clone();
        if chosen.subscription.is_none() {
            chosen.subscription = all.iter().find_map(|c| c.subscription.clone());
        }
        return Some(chosen);
    }
    all.into_iter().max_by_key(|c| c.expires_at.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ended_session_is_recognised_without_tokens() {
        let raw = r#"{"claudeAiOauth":{"accessToken":"","refreshToken":"","expiresAt":0,
            "refreshTokenExpiresAt":1794593118865,"subscriptionType":"team",
            "loginEnded":"time_limit"},"organizationUuid":"x"}"#;
        assert!(parse_creds(raw, "f".into()).is_none());
        let ended = parse_ended(raw, "f".into()).unwrap();
        assert_eq!(ended.reason.as_deref(), Some("time_limit"));
        assert_eq!(ended.subscription.as_deref(), Some("team"));
        assert_eq!(ended.refresh_expired_at, Some(1_794_593_118));
    }

    #[test]
    fn live_session_is_not_reported_as_ended() {
        let raw = r#"{"claudeAiOauth":{"accessToken":"abc","expiresAt":4102444800000}}"#;
        assert!(parse_ended(raw, "f".into()).is_none());
        assert!(parse_creds(raw, "f".into()).is_some());
    }
}
