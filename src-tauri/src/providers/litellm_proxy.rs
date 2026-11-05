//! LiteLLM Proxy connector (local).
//!
//! When a LiteLLM Proxy runs on this machine (default `http://127.0.0.1:4000`),
//! every request routed through it is metered there. This module reads those
//! meters and turns them into per-provider stats:
//!
//! * `GET /spend/logs`        → input/output tokens and USD spend per model
//! * `GET /provider/budgets`  → per-provider budget, spend and reset (the % bar)
//! * `GET /user/info`         → overall spend / max budget (fallback %)
//!
//! Models are attributed to providers by name (`claude*`/`anthropic/` →
//! Claude, `gpt*`/`o1-4*`/`openai/`/`azure/` → ChatGPT, `gemini*`/`vertex_ai/`
//! → Gemini, `cursor*` → Cursor). LiteLLM only knows traffic that actually
//! went through it, so a provider with no routed requests simply has no
//! stats and its card keeps using the normal source cascade.
//!
//! Safety: the base URL must be loopback (`localhost`, `127.0.0.1`, `::1`);
//! the client ignores proxies and redirects. Auth uses `LITELLM_API_KEY` or
//! `LITELLM_MASTER_KEY` if set. One probe is shared by all providers per
//! refresh (single-flight + 20s cache), and a down proxy is remembered for 30s
//! so a closed port costs at most one connection attempt per 30s.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::providers::env;
use crate::usage::models::{
    DataProvenance, DataSource, PlanInfo, ProviderId, SourceView, UsageSnapshot, UsageWindow,
    WindowType,
};

const DEFAULT_BASE: &str = "http://127.0.0.1:4000";
const CACHE_UP: Duration = Duration::from_secs(20);
const CACHE_DOWN: Duration = Duration::from_secs(30);
const SPEND_LOG_DAYS: i64 = 30;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderStats {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub spend_usd: f64,
    /// Percent of the provider budget (or of the overall budget) consumed.
    pub percent: Option<f64>,
    pub budget_usd: Option<f64>,
    pub resets_at: Option<DateTime<Utc>>,
    /// Budget period as reported (e.g. "1d", "30d").
    pub period: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProxyReport {
    pub base_url: String,
    pub by_provider: HashMap<ProviderId, ProviderStats>,
}

struct Cache {
    at: Instant,
    report: Option<ProxyReport>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::const_new(None);

/// Base URL from `LITELLM_PROXY_URL` / `LITELLM_BASE_URL`, loopback only.
fn base_url() -> Option<String> {
    let raw = env::lookup("LITELLM_PROXY_URL")
        .or_else(|| env::lookup("LITELLM_BASE_URL"))
        .map(|(v, _)| v)
        .unwrap_or_else(|| DEFAULT_BASE.to_string());
    let url = reqwest::Url::parse(raw.trim_end_matches('/')).ok()?;
    let loopback = matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("[::1]") | Some("::1")
    );
    if !loopback || url.scheme() != "http" && url.scheme() != "https" {
        tracing::warn!("[litellm] ignoring non-loopback proxy URL");
        return None;
    }
    Some(raw.trim_end_matches('/').to_string())
}

fn api_key() -> Option<String> {
    env::lookup("LITELLM_API_KEY")
        .or_else(|| env::lookup("LITELLM_MASTER_KEY"))
        .map(|(v, _)| v)
        // The key start-litellm.bat gives the proxy when none is configured.
        .or_else(|| Some(crate::litellm_launcher::DEFAULT_MASTER_KEY.to_string()))
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_millis(1500))
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())
}

async fn get_json(client: &reqwest::Client, base: &str, path: &str, key: Option<&str>) -> Result<Value, String> {
    let mut req = client.get(format!("{base}{path}"));
    if let Some(key) = key {
        req = req.bearer_auth(key);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("{path}: HTTP {}", res.status()));
    }
    res.json::<Value>().await.map_err(|e| format!("{path}: {e}"))
}

/// Provider for a LiteLLM model name or provider key.
pub fn provider_for_model(model: &str) -> Option<ProviderId> {
    let m = model.to_ascii_lowercase();
    if m.starts_with("anthropic") || m.contains("claude") {
        Some(ProviderId::Claude)
    } else if m.starts_with("gemini") || m.starts_with("vertex_ai") || m.contains("gemini") {
        Some(ProviderId::Antigravity)
    } else if m.starts_with("cursor") {
        Some(ProviderId::Cursor)
    } else {
        let bare = m.rsplit('/').next().unwrap_or(&m);
        let openai_family = m.starts_with("openai")
            || m.starts_with("azure")
            || m.starts_with("chatgpt")
            || bare.starts_with("gpt")
            || bare.starts_with("o1")
            || bare.starts_with("o3")
            || bare.starts_with("o4")
            || bare.contains("codex");
        openai_family.then_some(ProviderId::Codex)
    }
}

fn num(v: Option<&Value>) -> Option<f64> {
    v.and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
}

/// Sum tokens/spend from `/spend/logs`. Accepts both per-request rows
/// (`model`, `prompt_tokens`, `completion_tokens`, `spend`) and daily
/// aggregates (`spend`, `models: {model: spend}`).
pub fn parse_spend_logs(payload: &Value, into: &mut HashMap<ProviderId, ProviderStats>) {
    let rows = payload
        .as_array()
        .or_else(|| payload.get("data").and_then(Value::as_array));
    for row in rows.into_iter().flatten() {
        if let Some(models) = row.get("models").and_then(Value::as_object) {
            for (model, spend) in models {
                if let Some(p) = provider_for_model(model) {
                    into.entry(p).or_default().spend_usd += num(Some(spend)).unwrap_or(0.0);
                }
            }
            continue;
        }
        let model = row
            .get("model")
            .and_then(Value::as_str)
            .or_else(|| row.get("model_group").and_then(Value::as_str))
            .unwrap_or_default();
        let Some(p) = provider_for_model(model) else { continue };
        let stats = into.entry(p).or_default();
        stats.input_tokens += num(row.get("prompt_tokens")).unwrap_or(0.0) as u64;
        stats.output_tokens += num(row.get("completion_tokens")).unwrap_or(0.0) as u64;
        stats.spend_usd += num(row.get("spend")).unwrap_or(0.0);
    }
}

/// Per-provider budgets from `/provider/budgets`:
/// `{"providers": {"anthropic": {"budget_limit", "spend", "budget_reset_at", "time_period"}}}`.
pub fn parse_provider_budgets(payload: &Value, into: &mut HashMap<ProviderId, ProviderStats>) {
    let Some(providers) = payload
        .get("providers")
        .and_then(Value::as_object)
        .or_else(|| payload.as_object())
    else {
        return;
    };
    for (name, info) in providers {
        let Some(p) = provider_for_model(name) else { continue };
        let Some(limit) = num(info.get("budget_limit")).filter(|l| *l > 0.0) else { continue };
        let spend = num(info.get("spend")).unwrap_or(0.0);
        let stats = into.entry(p).or_default();
        stats.budget_usd = Some(limit);
        stats.percent = Some((spend / limit * 100.0).clamp(0.0, 100.0));
        stats.resets_at = info
            .get("budget_reset_at")
            .and_then(Value::as_str)
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc));
        stats.period = info.get("time_period").and_then(Value::as_str).map(str::to_string);
        if stats.spend_usd == 0.0 {
            stats.spend_usd = spend;
        }
    }
}

/// Overall budget from `/user/info` (`user_info.spend` / `user_info.max_budget`),
/// applied to providers that have usage but no provider budget of their own.
pub fn apply_user_budget(payload: &Value, into: &mut HashMap<ProviderId, ProviderStats>) {
    let info = payload.get("user_info").unwrap_or(payload);
    let Some(max) = num(info.get("max_budget")).filter(|m| *m > 0.0) else { return };
    let spend = num(info.get("spend")).unwrap_or(0.0);
    let pct = (spend / max * 100.0).clamp(0.0, 100.0);
    for stats in into.values_mut() {
        if stats.percent.is_none() {
            stats.percent = Some(pct);
            stats.budget_usd = Some(max);
            stats.period = Some("user budget".to_string());
        }
    }
}

async fn probe(base: &str) -> Option<ProxyReport> {
    let client = client().ok()?;
    let key = api_key();
    let key = key.as_deref();
    // Cheap unauthenticated liveness check first; any HTTP answer means the
    // proxy is up, a closed port fails here.
    if client.get(format!("{base}/health/liveliness")).send().await.is_err() {
        return None;
    }

    let mut by_provider = HashMap::new();
    let today = Utc::now().date_naive();
    let start = today - chrono::Duration::days(SPEND_LOG_DAYS);
    let logs_path = format!("/spend/logs?start_date={start}&end_date={}", today + chrono::Duration::days(1));
    match get_json(&client, base, &logs_path, key).await {
        Ok(v) => parse_spend_logs(&v, &mut by_provider),
        Err(e) => tracing::info!("[litellm] {e}"),
    }
    if let Ok(v) = get_json(&client, base, "/provider/budgets", key).await {
        parse_provider_budgets(&v, &mut by_provider);
    }
    if let Ok(v) = get_json(&client, base, "/user/info", key).await {
        apply_user_budget(&v, &mut by_provider);
    }
    by_provider.retain(|_, s| s.spend_usd > 0.0 || s.input_tokens + s.output_tokens > 0 || s.percent.is_some());
    tracing::info!("[litellm] proxy at {base} up; providers with data: {}", by_provider.len());
    Some(ProxyReport {
        base_url: base.to_string(),
        by_provider,
    })
}

/// Current proxy report, shared by all providers (single-flight, cached).
pub async fn report() -> Option<ProxyReport> {
    let base = base_url()?;
    let mut guard = CACHE.lock().await;
    if let Some(cache) = guard.as_ref() {
        let ttl = if cache.report.is_some() { CACHE_UP } else { CACHE_DOWN };
        if cache.at.elapsed() < ttl {
            return cache.report.clone();
        }
    }
    let report = probe(&base).await;
    *guard = Some(Cache {
        at: Instant::now(),
        report: report.clone(),
    });
    report
}

/// Stats for one provider if the proxy is up and has data for it.
pub async fn stats_for(provider: ProviderId) -> Option<(String, ProviderStats)> {
    let report = report().await?;
    let stats = report.by_provider.get(&provider)?.clone();
    Some((report.base_url, stats))
}

pub const BADGE: &str = "LIVE — LITELLM";

fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1}K", n as f64 / 1e3),
        n => n.to_string(),
    }
}

fn summary(base: &str, stats: &ProviderStats) -> String {
    format!(
        "{} in · {} out tokens · ${:.2} in the last {SPEND_LOG_DAYS} days (LiteLLM at {base})",
        compact(stats.input_tokens),
        compact(stats.output_tokens),
        stats.spend_usd
    )
}

fn windows(stats: &ProviderStats) -> Vec<UsageWindow> {
    let Some(pct) = stats.percent else { return Vec::new() };
    let window_type = match stats.period.as_deref() {
        Some("1d") | Some("24h") => WindowType::Daily,
        Some("7d") | Some("1w") => WindowType::Weekly,
        _ => WindowType::Monthly,
    };
    let note = match stats.budget_usd {
        Some(budget) => format!("LiteLLM budget: ${:.2} of ${budget:.2}", stats.spend_usd),
        None => "LiteLLM budget".to_string(),
    };
    vec![UsageWindow::percent(window_type, pct, stats.resets_at, note)]
}

/// Card snapshot built from LiteLLM metrics (green "LIVE — LITELLM" badge).
pub fn snapshot(provider: ProviderId, base: &str, stats: &ProviderStats) -> UsageSnapshot {
    UsageSnapshot {
        provider,
        windows: windows(stats),
        credits: None,
        plan: Some(PlanInfo {
            tier: Some("LiteLLM".to_string()),
            note: Some(summary(base, stats)),
        }),
        fetched_at: Utc::now(),
        source: DataSource::Cli,
        provenance: DataProvenance::Official,
        stale: false,
        badge: Some(BADGE.to_string()),
        sources: Vec::new(),
    }
}

/// The same data as a selectable source (used by the Gemini toggle).
pub fn source_view(base: &str, stats: &ProviderStats) -> SourceView {
    SourceView {
        id: "litellm".to_string(),
        label: "LiteLLM".to_string(),
        state: "live".to_string(),
        badge: BADGE.to_string(),
        windows: windows(stats),
        plan: Some(PlanInfo {
            tier: Some("LiteLLM".to_string()),
            note: Some(summary(base, stats)),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_models_to_providers() {
        assert_eq!(provider_for_model("anthropic/claude-sonnet-4-5"), Some(ProviderId::Claude));
        assert_eq!(provider_for_model("claude-opus-4"), Some(ProviderId::Claude));
        assert_eq!(provider_for_model("gpt-5"), Some(ProviderId::Codex));
        assert_eq!(provider_for_model("openai/o3-mini"), Some(ProviderId::Codex));
        assert_eq!(provider_for_model("azure"), Some(ProviderId::Codex));
        assert_eq!(provider_for_model("gemini/gemini-2.5-pro"), Some(ProviderId::Antigravity));
        assert_eq!(provider_for_model("vertex_ai"), Some(ProviderId::Antigravity));
        assert_eq!(provider_for_model("cursor-small"), Some(ProviderId::Cursor));
        assert_eq!(provider_for_model("mistral-large"), None);
    }

    #[test]
    fn sums_spend_logs_and_applies_budgets() {
        let mut map = HashMap::new();
        parse_spend_logs(
            &json!([
                {"model": "claude-sonnet-4-5", "prompt_tokens": 1000, "completion_tokens": 200, "spend": 0.5},
                {"model": "claude-sonnet-4-5", "prompt_tokens": 500, "completion_tokens": 100, "spend": 0.25},
                {"model": "gpt-5", "prompt_tokens": 10, "completion_tokens": 5, "spend": "0.01"},
                {"startTime": "2026-11-04", "spend": 2.0, "models": {"gemini/gemini-2.5-pro": 2.0}}
            ]),
            &mut map,
        );
        parse_provider_budgets(
            &json!({"providers": {"anthropic": {"budget_limit": 10.0, "spend": 2.5, "time_period": "1d",
                     "budget_reset_at": "2026-11-06T00:00:00Z"}}}),
            &mut map,
        );
        apply_user_budget(&json!({"user_info": {"spend": 5.0, "max_budget": 20.0}}), &mut map);

        let claude = &map[&ProviderId::Claude];
        assert_eq!((claude.input_tokens, claude.output_tokens), (1500, 300));
        assert!((claude.spend_usd - 0.75).abs() < 1e-9);
        assert_eq!(claude.percent, Some(25.0));
        assert_eq!(claude.period.as_deref(), Some("1d"));
        assert_eq!(map[&ProviderId::Codex].percent, Some(25.0)); // user budget 5/20
        assert!((map[&ProviderId::Antigravity].spend_usd - 2.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn reads_a_running_proxy_end_to_end() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { break };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let body = if req.starts_with("GET /spend/logs") {
                        r#"[{"model":"claude-sonnet-4-5","prompt_tokens":42,"completion_tokens":8,"spend":0.1}]"#
                    } else if req.starts_with("GET /provider/budgets") {
                        r#"{"providers":{"anthropic":{"budget_limit":1.0,"spend":0.39}}}"#
                    } else {
                        r#"{"status":"ok"}"#
                    };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        let report = probe(&format!("http://{addr}")).await.expect("proxy should be up");
        let claude = &report.by_provider[&ProviderId::Claude];
        assert_eq!((claude.input_tokens, claude.output_tokens), (42, 8));
        assert_eq!(claude.percent.map(|p| p.round()), Some(39.0));
    }

    #[tokio::test]
    async fn closed_port_means_down() {
        // Bind and drop to get a port that is (almost certainly) closed.
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        assert!(probe(&format!("http://127.0.0.1:{port}")).await.is_none());
    }
}
