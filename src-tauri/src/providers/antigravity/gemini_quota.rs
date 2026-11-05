//! Gemini CLI quota via Google's Code Assist API.
//!
//! When the Gemini CLI is signed in with a Google account it stores its OAuth
//! token in `~/.gemini/oauth_creds.json`. With that token we ask Google's own
//! `cloudcode-pa.googleapis.com` (the backend the CLI itself talks to) for the
//! per-model request quota. The file is only read; the token is never
//! refreshed or written back by this app — the CLI renews it when it runs.
//!
//! API-key users (`GEMINI_API_KEY`) have no quota endpoint on
//! generativelanguage.googleapis.com, so for them only local logs apply.

use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::providers::http::{self, now_unix};

const BASE: &str = "https://cloudcode-pa.googleapis.com/v1internal";

#[derive(Debug, Clone)]
pub struct GeminiBucket {
    pub model: String,
    /// 0.0..=1.0 of the quota still available.
    pub remaining_fraction: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

pub struct GeminiCreds {
    pub access_token: String,
    pub expires_at: Option<u64>,
}

pub fn read_creds() -> Option<GeminiCreds> {
    let path = dirs::home_dir()?.join(".gemini").join("oauth_creds.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    let access_token = json.get("access_token")?.as_str()?.trim().to_string();
    if access_token.is_empty() {
        return None;
    }
    let expires_at = json
        .get("expiry_date")
        .and_then(Value::as_u64)
        .map(|ms| ms / 1000);
    Some(GeminiCreds {
        access_token,
        expires_at,
    })
}

pub fn token_is_fresh(creds: &GeminiCreds) -> bool {
    creds.expires_at.is_none_or(|exp| exp > now_unix() + 30)
}

/// Code Assist project id, resolved once per run.
static PROJECT: Mutex<Option<String>> = Mutex::new(None);

async fn project_id(token: &str) -> Result<String, String> {
    if let Ok(env) = std::env::var("GOOGLE_CLOUD_PROJECT") {
        if !env.trim().is_empty() {
            return Ok(env.trim().to_string());
        }
    }
    if let Some(cached) = PROJECT.lock().ok().and_then(|g| g.clone()) {
        return Ok(cached);
    }
    let req = http::post(&format!("{BASE}:loadCodeAssist"))?
        .bearer_auth(token)
        .json(&json!({
            "metadata": {
                "ideType": "IDE_UNSPECIFIED",
                "platform": "PLATFORM_UNSPECIFIED",
                "pluginType": "GEMINI"
            }
        }));
    let res = http::send_json(req).await?;
    let project = res
        .get("cloudaicompanionProject")
        .and_then(|p| p.as_str().map(str::to_string).or_else(|| {
            p.get("id").and_then(Value::as_str).map(str::to_string)
        }))
        .ok_or("Gemini account has no Code Assist project")?;
    if let Ok(mut g) = PROJECT.lock() {
        *g = Some(project.clone());
    }
    Ok(project)
}

pub async fn fetch_quota(token: &str) -> Result<Vec<GeminiBucket>, String> {
    let project = project_id(token).await?;
    let req = http::post(&format!("{BASE}:retrieveUserQuota"))?
        .bearer_auth(token)
        .json(&json!({ "project": project }));
    let res = http::send_json(req).await?;
    let buckets = res
        .get("buckets")
        .and_then(Value::as_array)
        .ok_or("quota response had no buckets")?;
    let out: Vec<GeminiBucket> = buckets
        .iter()
        .filter_map(|b| {
            Some(GeminiBucket {
                model: b
                    .get("modelId")
                    .and_then(Value::as_str)
                    .unwrap_or("gemini")
                    .to_string(),
                remaining_fraction: b.get("remainingFraction")?.as_f64()?.clamp(0.0, 1.0),
                resets_at: b
                    .get("resetTime")
                    .and_then(Value::as_str)
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&Utc)),
            })
        })
        .collect();
    if out.is_empty() {
        return Err("quota response had no usable buckets".to_string());
    }
    Ok(out)
}
