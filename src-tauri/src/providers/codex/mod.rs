mod bearer_fetcher;
mod descriptor;
mod log_parser;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::Value;
use tracing::info;

use crate::providers::auth::{AuthKind, AuthState};
use crate::providers::http::{jwt_claims, jwt_expires_at, now_unix};
use crate::providers::{
    run_blocking, FetchContext, LiveGate, ProviderDescriptor, UsageProvider, MIN_LIVE_INTERVAL,
};
use crate::usage::models::{
    CostEntry, DataProvenance, DataSource, PlanInfo, ProviderHealth, ProviderId, ProviderStatus,
    UsageSnapshot, UsageUnit, UsageWindow, WindowType,
};
use crate::usage_scanners::codex_home_roots;

/// Approximate token budgets used only when the live API is not reachable.
const SESSION_TOKEN_LIMIT: u64 = 500_000;
const WEEKLY_TOKEN_LIMIT: u64 = 5_000_000;

/// ChatGPT (Codex CLI) provider.
///
/// 1. Live: reads the ChatGPT login the Codex CLI saved in `~/.codex/auth.json`
///    and asks chatgpt.com for the plan's session/weekly usage.
/// 2. Local: token counts and costs come from `~/.codex/sessions` JSONL files.
///
/// `auth.json` is only read, never rewritten, and tokens are never refreshed by
/// this app (Codex rotates refresh tokens, so doing it here could sign the CLI
/// out). An expired token simply falls back to local data until the CLI renews
/// it.
pub struct CodexProvider {
    descriptor: ProviderDescriptor,
    gate: LiveGate,
}

impl CodexProvider {
    pub fn new() -> Self {
        Self {
            descriptor: descriptor::descriptor(),
            gate: LiveGate::default(),
        }
    }
}

struct CodexAuth {
    access_token: String,
    account_id: Option<String>,
    has_refresh_token: bool,
    expires_at: Option<u64>,
    source_path: String,
    /// ChatGPT plan ("plus", "pro", "team"…) from the id_token claims.
    plan: Option<String>,
}

fn read_auth() -> Option<CodexAuth> {
    for root in codex_home_roots() {
        let path = root.join("auth.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let tokens = json.get("tokens").unwrap_or(&Value::Null);
        let access_token = tokens
            .get("access_token")
            .or_else(|| json.get("access_token"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        if access_token.is_empty() {
            continue;
        }
        let plan = tokens
            .get("id_token")
            .and_then(Value::as_str)
            .and_then(jwt_claims)
            .and_then(|c| {
                c.get("https://api.openai.com/auth")?
                    .get("chatgpt_plan_type")?
                    .as_str()
                    .map(str::to_string)
            });
        return Some(CodexAuth {
            plan,
            expires_at: jwt_expires_at(&access_token),
            account_id: tokens
                .get("account_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            has_refresh_token: tokens
                .get("refresh_token")
                .and_then(Value::as_str)
                .is_some_and(|v| !v.is_empty()),
            access_token,
            source_path: path.display().to_string(),
        });
    }
    None
}

fn has_local_logs() -> bool {
    codex_home_roots()
        .iter()
        .any(|root| root.join("sessions").is_dir() || root.join("archived_sessions").is_dir())
}

fn local_log_snapshot(note: Option<String>) -> UsageSnapshot {
    let (session, weekly) = log_parser::usage_windows();
    let now = Utc::now();
    let suffix = note.map(|n| format!(" {n}")).unwrap_or_default();
    UsageSnapshot {
        provider: ProviderId::Codex,
        windows: vec![
            UsageWindow::approximate(
                WindowType::Session,
                session,
                SESSION_TOKEN_LIMIT,
                Some(now + Duration::hours(5)),
                UsageUnit::Tokens,
                format!("Estimated from local Codex session logs.{suffix}"),
            ),
            UsageWindow::approximate(
                WindowType::Weekly,
                weekly,
                WEEKLY_TOKEN_LIMIT,
                Some(now + Duration::days(7)),
                UsageUnit::Tokens,
                "Rolling seven-day total from local Codex logs.",
            ),
        ],
        credits: None,
        plan: None,
        fetched_at: now,
        source: DataSource::LocalLog,
        provenance: DataProvenance::DerivedLocal,
        stale: false,
        badge: None,
        sources: Vec::new(),
    }
}

impl CodexProvider {
    async fn resolve(&self, auth: Option<CodexAuth>) -> Result<UsageSnapshot, String> {
        let note = match auth {
            None => {
                info!("[codex] no ~/.codex/auth.json login found; using local logs");
                Some("Run `codex login` for live ChatGPT plan usage.".to_string())
            }
            Some(a) if a.expires_at.is_some_and(|exp| exp <= now_unix() + 30) => {
                Some("ChatGPT token expired; run `codex` once to renew it.".to_string())
            }
            Some(_) if !self.gate.may_call() => {
                if let Some(cached) = self.gate.cached() {
                    return Ok(cached);
                }
                None
            }
            Some(a) => match bearer_fetcher::fetch_usage(&a.access_token, a.account_id.as_deref())
                .await
            {
                Ok(live) => {
                    let snapshot = UsageSnapshot {
                        provider: ProviderId::Codex,
                        windows: vec![
                            UsageWindow::percent(
                                WindowType::Session,
                                live.session_used as f64,
                                live.resets_at,
                                "Live session usage from chatgpt.com.",
                            ),
                            UsageWindow::percent(
                                WindowType::Weekly,
                                live.weekly_used as f64,
                                live.weekly_resets_at,
                                "Live weekly usage from chatgpt.com.",
                            ),
                        ],
                        credits: None,
                        plan: None,
                        fetched_at: Utc::now(),
                        source: DataSource::Oauth,
                        provenance: DataProvenance::Internal,
                        stale: false,
                        badge: None,
                        sources: Vec::new(),
                    };
                    self.gate.ok(&snapshot);
                    return Ok(snapshot);
                }
                Err(e) => {
                    self.gate.failed();
                    if let Some(cached) = self.gate.cached() {
                        return Ok(cached);
                    }
                    info!("[codex] live usage unavailable, using local logs: {e}");
                    None
                }
            },
        };
        run_blocking(move || local_log_snapshot(note)).await
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[async_trait]
impl UsageProvider for CodexProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn fetch_usage(&self, _ctx: &FetchContext) -> Result<UsageSnapshot, String> {
        // A running LiteLLM Proxy with traffic for this provider wins; otherwise
        // fall through to the normal source cascade.
        if let Some((base, stats)) =
            crate::providers::litellm_proxy::stats_for(ProviderId::Codex).await
        {
            return Ok(crate::providers::litellm_proxy::snapshot(ProviderId::Codex, &base, &stats));
        }
        let auth = run_blocking(read_auth).await?;
        let plan = auth.as_ref().and_then(|a| a.plan.clone()).map(|tier| PlanInfo {
            tier: Some(format!("ChatGPT {}", title_case(&tier))),
            note: None,
        });
        let valid = auth
            .as_ref()
            .is_some_and(|a| a.expires_at.is_none_or(|exp| exp > now_unix() + 30));
        let recent = if valid { self.gate.recent(MIN_LIVE_INTERVAL) } else { None };
        let mut snapshot = match recent {
            Some(snapshot) => snapshot,
            None => self.resolve(auth).await?,
        };
        if snapshot.plan.is_none() {
            snapshot.plan = plan;
        }
        Ok(snapshot)
    }
    async fn fetch_cost_history(&self, days: u32) -> Result<Vec<CostEntry>, String> {
        run_blocking(move || log_parser::cost_history(days)).await
    }

    async fn check_status(&self) -> ProviderStatus {
        let (detected, signed_in) = run_blocking(|| (has_local_logs(), read_auth().is_some()))
            .await
            .unwrap_or((false, false));
        ProviderStatus {
            provider: ProviderId::Codex,
            health: if detected || signed_in {
                ProviderHealth::Active
            } else {
                ProviderHealth::Waiting
            },
            message: match (signed_in, detected) {
                (true, _) => "ChatGPT login found in ~/.codex/auth.json".to_string(),
                (false, true) => "Codex logs detected (not signed in)".to_string(),
                (false, false) => "Waiting for Codex CLI (ChatGPT)".to_string(),
            },
            checked_at: Utc::now(),
        }
    }

    fn compute_auth_state(&self, _ctx: &FetchContext) -> AuthState {
        let Some(a) = read_auth() else {
            return AuthState::none(ProviderId::Codex);
        };
        AuthState {
            provider: ProviderId::Codex,
            kind: AuthKind::Oauth,
            source_path: a.source_path,
            expires_at_unix_secs: a.expires_at,
            last_refresh_iso: None,
            has_refresh_token: a.has_refresh_token,
            last_error: None,
        }
    }
}
