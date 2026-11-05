use chrono::{DateTime, Utc};
use reqwest::header::AUTHORIZATION;

use crate::providers::http;
use serde_json::Value;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ClaudeOauthWindow {
    pub five_hour_utilization: f64,
    pub seven_day_utilization: f64,
    pub seven_day_opus_utilization: f64,
    /// Whether the API actually reported an Opus window. Distinguishes "Opus
    /// quota is at 0%" from "this plan has no Opus window", so the gauge does
    /// not vanish and reappear between polls.
    pub has_opus_window: bool,
    pub five_hour_resets_at: Option<DateTime<Utc>>,
    pub seven_day_resets_at: Option<DateTime<Utc>>,
    /// Extra/Max usage credits info (when enabled on the plan).
    pub extra_usage: Option<ExtraUsageInfo>,
}

#[derive(Debug, Clone)]
pub struct ExtraUsageInfo {
    pub monthly_limit_usd: f64,
    pub used_credits_usd: f64,
    pub utilization: f64,
}

/// Endpoints on api.anthropic.com, tried in order. The legacy `usage` endpoint
/// has a known response format; `client_data` (Claude Code v2.1+) is used only
/// when the first one is unavailable or returns an unexpected shape. A 429 or
/// auth failure stops immediately — there is no retry loop or sleep.
const ENDPOINTS: &[&str] = &[
    "https://api.anthropic.com/api/oauth/usage",
    "https://api.anthropic.com/api/oauth/claude_cli/client_data",
];

pub async fn fetch_usage(token: &str) -> Result<ClaudeOauthWindow, String> {
    let mut last_err = String::new();
    for endpoint in ENDPOINTS {
        debug!("[claude] GET {endpoint}");
        let req = http::get(endpoint)?
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("anthropic-beta", "oauth-2025-04-20");
        let raw = http::send_json(req).await;
        if std::env::var("QP_LOG_RAW").is_ok_and(|v| v == "1") {
            // Diagnostics: the usage payload carries no secrets (percentages
            // and reset times only). The request token is never logged.
            match &raw {
                Ok(v) => info!("[claude] raw {endpoint}: {}", truncate(&v.to_string(), 4000)),
                Err(e) => info!("[claude] raw {endpoint}: error {e}"),
            }
        }
        match raw.and_then(parse_usage_response) {
            Ok(window) => return Ok(window),
            Err(e) => {
                warn!("[claude] {endpoint}: {e}");
                let fatal = e.contains("429") || e.contains("401") || e.contains("403");
                last_err = e;
                if fatal {
                    break;
                }
            }
        }
    }
    Err(format!("Claude usage API failed: {last_err}"))
}

fn parse_usage_response(payload: Value) -> Result<ClaudeOauthWindow, String> {
    // Try to find the usage data — it could be at the top level (legacy endpoint),
    // nested under "usage" or "rate_limits" (client_data endpoint), or elsewhere.
    //
    // If no recognizable window object is present we MUST fail rather than fall
    // through: every field below is read with `unwrap_or(0.0)`, so an unexpected
    // 200 response would otherwise parse as a perfectly valid "0% used"
    // snapshot. That snapshot then gets cached as the last known-good value and
    // persisted over the real one, which is what makes the gauges collapse to
    // zero for no visible reason.
    let Some(root) = find_usage_root(&payload) else {
        return Err(
            "response contained no five_hour/seven_day usage window (unexpected payload shape)"
                .to_string(),
        );
    };

    let f = root.get("five_hour").cloned().unwrap_or(Value::Null);
    let s = root.get("seven_day").cloned().unwrap_or(Value::Null);
    let o = root.get("seven_day_opus").cloned().unwrap_or(Value::Null);

    let extra_usage_val = root
        .get("extra_usage")
        .or_else(|| payload.get("extra_usage"));
    let extra_usage = extra_usage_val.and_then(|eu| {
        let enabled = eu
            .get("is_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !enabled {
            return None;
        }
        Some(ExtraUsageInfo {
            monthly_limit_usd: eu
                .get("monthly_limit")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            used_credits_usd: eu
                .get("used_credits")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            utilization: eu.get("utilization").and_then(Value::as_f64).unwrap_or(0.0),
        })
    });

    let window = ClaudeOauthWindow {
        five_hour_utilization: f.get("utilization").and_then(Value::as_f64).unwrap_or(0.0),
        seven_day_utilization: s.get("utilization").and_then(Value::as_f64).unwrap_or(0.0),
        seven_day_opus_utilization: o.get("utilization").and_then(Value::as_f64).unwrap_or(0.0),
        has_opus_window: o.is_object(),
        five_hour_resets_at: parse_dt(f.get("resets_at").and_then(Value::as_str)),
        seven_day_resets_at: parse_dt(s.get("resets_at").and_then(Value::as_str)),
        extra_usage,
    };
    info!(
        "[claude] OAuth OK: 5h={:.1}% 7d={:.1}% opus={:.1}%{}",
        window.five_hour_utilization,
        window.seven_day_utilization,
        window.seven_day_opus_utilization,
        window
            .extra_usage
            .as_ref()
            .map(|eu| format!(" extra={:.1}%", eu.utilization))
            .unwrap_or_default(),
    );
    Ok(window)
}

/// Walk the JSON payload to find the object containing `five_hour` / `seven_day` keys.
/// Supports: top-level, nested under "usage", "rate_limits", or one level deep in any key.
///
/// Returns `None` when no such object exists. Callers must treat that as a hard
/// error — see the note in `parse_usage_response`.
fn find_usage_root(payload: &Value) -> Option<&Value> {
    fn has_window(value: &Value) -> bool {
        value.get("five_hour").is_some() || value.get("seven_day").is_some()
    }

    // Direct top-level match
    if has_window(payload) {
        return Some(payload);
    }

    // Known nesting keys
    for key in &["usage", "rate_limits", "rateLimits", "data"] {
        if let Some(inner) = payload.get(key) {
            if has_window(inner) {
                return Some(inner);
            }
        }
    }

    // Scan one level deep for any object containing usage fields
    if let Some(obj) = payload.as_object() {
        for val in obj.values() {
            if val.is_object() && has_window(val) {
                return Some(val);
            }
        }
    }

    None
}

fn parse_dt(value: Option<&str>) -> Option<DateTime<Utc>> {
    value
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
        .map(|v| v.with_timezone(&Utc))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Shape returned by api.anthropic.com/api/oauth/usage: the values must
    /// come through exactly (31% / 20%), with no rounding or scaling.
    #[test]
    fn maps_five_hour_and_weekly_exactly() {
        let payload = json!({
            "five_hour": { "utilization": 31.0, "resets_at": "2026-11-05T17:00:00+00:00" },
            "seven_day": { "utilization": 20.0, "resets_at": "2026-11-10T09:00:00+00:00" },
            "seven_day_oauth_apps": null,
            "seven_day_opus": null,
            "extra_usage": { "is_enabled": false }
        });
        let w = parse_usage_response(payload).unwrap();
        assert_eq!(w.five_hour_utilization, 31.0);
        assert_eq!(w.seven_day_utilization, 20.0);
        assert!(!w.has_opus_window);
        assert!(w.extra_usage.is_none());
        assert_eq!(w.five_hour_resets_at.unwrap().to_rfc3339(), "2026-11-05T17:00:00+00:00");
    }

    #[test]
    fn keeps_fractional_values_and_nested_shapes() {
        let payload = json!({ "rate_limits": {
            "five_hour": { "utilization": 31.4 },
            "seven_day": { "utilization": 19.6 }
        }});
        let w = parse_usage_response(payload).unwrap();
        assert_eq!((w.five_hour_utilization, w.seven_day_utilization), (31.4, 19.6));
    }

    #[test]
    fn rejects_payloads_without_windows() {
        assert!(parse_usage_response(json!({ "error": "x" })).is_err());
    }
}
