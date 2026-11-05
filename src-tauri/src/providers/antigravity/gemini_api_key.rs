//! Official Gemini API key (Google AI Studio) support.
//!
//! Sources: `GEMINI_API_KEY` / `GOOGLE_API_KEY` (process or user environment,
//! so `setx` works without restarting) and `~/.gemini/.env`, which the Gemini
//! CLI also reads.
//!
//! Google's Generative Language API has no endpoint that reports quota or
//! usage for an API key (those live in Cloud Console / Cloud Monitoring and
//! require OAuth), so the honest thing this module can do is confirm the key
//! is valid with a lightweight call and let the card show
//! "ACTIVE — API KEY". It never invents a percentage. The key is sent in the
//! `x-goog-api-key` header (never in a URL) and validated at most once per
//! hour.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::providers::env;
use crate::providers::http;

const KEY_VARS: &[&str] = &["GEMINI_API_KEY", "GOOGLE_API_KEY"];
const REVALIDATE_EVERY: Duration = Duration::from_secs(3600);
const VALIDATE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1";

pub struct ApiKey {
    pub value: String,
    pub source: String,
}

/// Find a configured key without contacting anything.
pub fn find() -> Option<ApiKey> {
    for var in KEY_VARS {
        if let Some((value, origin)) = env::lookup(var) {
            return Some(ApiKey {
                value,
                source: format!("{origin} {var}"),
            });
        }
    }
    let path = dirs::home_dir()?.join(".gemini").join(".env");
    let raw = std::fs::read_to_string(&path).ok()?;
    raw.lines().find_map(|line| {
        let line = line.trim().trim_start_matches("export ").trim();
        let (name, value) = line.split_once('=')?;
        let value = value.trim().trim_matches('"').trim_matches('\'');
        (KEY_VARS.contains(&name.trim()) && !value.is_empty()).then(|| ApiKey {
            value: value.to_string(),
            source: path.display().to_string(),
        })
    })
}

/// (fingerprint of the key, checked at, valid)
static LAST_CHECK: Mutex<Option<(u64, Instant, bool)>> = Mutex::new(None);

fn fingerprint(key: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    h.finish()
}

/// Whether Google accepts the key (cached for an hour per key).
pub async fn is_valid(key: &ApiKey) -> bool {
    let fp = fingerprint(&key.value);
    if let Some((cached_fp, at, valid)) = LAST_CHECK.lock().ok().and_then(|g| *g) {
        if cached_fp == fp && at.elapsed() < REVALIDATE_EVERY {
            return valid;
        }
    }
    let valid = match http::get(VALIDATE_URL) {
        Ok(req) => http::send_json(req.header("x-goog-api-key", &key.value))
            .await
            .map_err(|e| tracing::info!("[gemini] API key check failed: {e}"))
            .is_ok(),
        Err(_) => false,
    };
    if let Ok(mut g) = LAST_CHECK.lock() {
        *g = Some((fp, Instant::now(), valid));
    }
    valid
}
