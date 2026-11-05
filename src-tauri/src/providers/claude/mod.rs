mod descriptor;
mod keychain;
mod log_parser;
mod oauth_fetcher;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use tracing::info;

use crate::providers::auth::{AuthKind, AuthState};
use crate::providers::{
    run_blocking, FetchContext, LiveGate, ProviderDescriptor, UsageProvider, MIN_LIVE_INTERVAL,
};
use crate::usage::models::{
    CostEntry, CreditsInfo, DataProvenance, DataSource, PlanInfo, ProviderHealth, ProviderId,
    ProviderStatus, UsageSnapshot, UsageUnit, UsageWindow, WindowType,
};
use crate::usage_scanners::resolve_claude_project_roots;

/// Approximate Pro-plan token budgets used only when the live API is not
/// reachable and the meters fall back to local log totals.
const FIVE_HOUR_TOKEN_LIMIT: u64 = 500_000;
const SEVEN_DAY_TOKEN_LIMIT: u64 = 5_000_000;

/// Claude Code provider.
///
/// 1. Live: reads Claude Code's own OAuth token from `.credentials.json` and
///    asks `api.anthropic.com` for the subscriber 5-hour / 7-day utilization.
/// 2. Local: token counts and costs always come from the JSONL transcripts in
///    `~/.claude/projects`, which the file watcher tracks in real time.
pub struct ClaudeProvider {
    descriptor: ProviderDescriptor,
    gate: LiveGate,
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self {
            descriptor: descriptor::descriptor(),
            gate: LiveGate::default(),
        }
    }
}

fn has_local_logs() -> bool {
    resolve_claude_project_roots().iter().any(|p| p.is_dir())
}

fn local_log_snapshot(note: Option<String>) -> UsageSnapshot {
    let (day_tokens, week_tokens, _) = log_parser::usage_windows();
    let now = Utc::now();
    let suffix = note.map(|n| format!(" {n}")).unwrap_or_default();
    UsageSnapshot {
        provider: ProviderId::Claude,
        windows: vec![
            UsageWindow::approximate(
                WindowType::FiveHour,
                day_tokens,
                FIVE_HOUR_TOKEN_LIMIT,
                Some(now + Duration::hours(5)),
                UsageUnit::Tokens,
                format!("Estimated from local Claude Code logs.{suffix}"),
            ),
            UsageWindow::approximate(
                WindowType::SevenDay,
                week_tokens,
                SEVEN_DAY_TOKEN_LIMIT,
                Some(now + Duration::days(7)),
                UsageUnit::Tokens,
                "Seven-day total from local Claude Code logs.",
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

fn snapshot_from_api(oauth: oauth_fetcher::ClaudeOauthWindow) -> UsageSnapshot {
    let mut windows = vec![
        UsageWindow::percent(
            WindowType::FiveHour,
            oauth.five_hour_utilization,
            oauth.five_hour_resets_at,
            "Live 5-hour utilization from api.anthropic.com.",
        ),
        UsageWindow::percent(
            WindowType::SevenDay,
            oauth.seven_day_utilization,
            oauth.seven_day_resets_at,
            "Live 7-day utilization from api.anthropic.com.",
        ),
    ];
    if oauth.has_opus_window {
        windows.push(UsageWindow::percent(
            WindowType::Weekly,
            oauth.seven_day_opus_utilization,
            oauth.seven_day_resets_at,
            "Opus weekly utilization from api.anthropic.com.",
        ));
    }
    let credits = oauth.extra_usage.as_ref().map(|eu| CreditsInfo {
        balance_usd: Some((eu.monthly_limit_usd - eu.used_credits_usd).max(0.0)),
        spent_usd: Some(eu.used_credits_usd),
    });
    UsageSnapshot {
        provider: ProviderId::Claude,
        windows,
        credits,
        plan: None,
        fetched_at: Utc::now(),
        source: DataSource::Oauth,
        provenance: DataProvenance::Internal,
        stale: false,
        badge: None,
        sources: Vec::new(),
    }
}

impl ClaudeProvider {
    async fn resolve(&self, creds: Option<keychain::ClaudeCreds>) -> Result<UsageSnapshot, String> {
        let note = match creds {
            None => {
                info!("[claude] no Claude Code credentials found; using local logs");
                Some("Sign in to Claude Code for live plan usage.".to_string())
            }
            Some(c) if c.is_expired() => {
                info!("[claude] OAuth token in {} is expired; using local logs", c.source_path);
                Some("Claude Code token expired; run `claude` once to renew it.".to_string())
            }
            Some(_) if !self.gate.may_call() => {
                if let Some(cached) = self.gate.cached() {
                    return Ok(cached);
                }
                None
            }
            Some(_) => {
                let candidates = run_blocking(keychain::read_valid_candidates).await?;
                let mut last_err = String::from("no usable credential");
                for cred in candidates {
                    if recently_rejected(&cred.access_token) {
                        continue;
                    }
                    match oauth_fetcher::fetch_usage(&cred.access_token).await {
                        Ok(oauth) => {
                            tracing::debug!("[claude] live usage via {}", cred.source_path);
                            let snapshot = snapshot_from_api(oauth);
                            self.gate.ok(&snapshot);
                            return Ok(snapshot);
                        }
                        // Token refused (expired early / missing scope): try the
                        // next credential instead of giving up.
                        Err(e) if e.contains("401") || e.contains("403") => {
                            info!(
                                "[claude] {} was rejected by the usage API; trying the next credential ({e})",
                                cred.source_path
                            );
                            mark_rejected(&cred.access_token);
                            last_err = e;
                        }
                        // Rate limit / network: same for every token, stop here.
                        Err(e) => {
                            last_err = e;
                            break;
                        }
                    }
                }
                self.gate.failed();
                if let Some(cached) = self.gate.cached() {
                    return Ok(cached);
                }
                info!("[claude] live usage unavailable, using local logs: {last_err}");
                None
            }
        };
        run_blocking(move || local_log_snapshot(note)).await
    }
}

/// Tokens the usage API refused recently (by hash), skipped for an hour so a
/// scope-less token does not cost a request on every poll.
static REJECTED: std::sync::Mutex<Vec<(u64, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());
const REJECTED_FOR: std::time::Duration = std::time::Duration::from_secs(3600);

fn token_hash(token: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut h);
    h.finish()
}

fn recently_rejected(token: &str) -> bool {
    let hash = token_hash(token);
    REJECTED.lock().is_ok_and(|mut list| {
        list.retain(|(_, at)| at.elapsed() < REJECTED_FOR);
        list.iter().any(|(h, _)| *h == hash)
    })
}

fn mark_rejected(token: &str) {
    if let Ok(mut list) = REJECTED.lock() {
        list.push((token_hash(token), std::time::Instant::now()));
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
impl UsageProvider for ClaudeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Claude
    }

    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn fetch_usage(&self, _ctx: &FetchContext) -> Result<UsageSnapshot, String> {
        // A running LiteLLM Proxy with traffic for this provider wins; otherwise
        // fall through to the normal source cascade.
        if let Some((base, stats)) =
            crate::providers::litellm_proxy::stats_for(ProviderId::Claude).await
        {
            return Ok(crate::providers::litellm_proxy::snapshot(ProviderId::Claude, &base, &stats));
        }
        let creds = run_blocking(keychain::read_credentials).await?;
        let plan = creds
            .as_ref()
            .and_then(|c| c.subscription.as_deref())
            .map(|s| PlanInfo {
                tier: Some(title_case(s)),
                note: None,
            });
        let valid = creds.as_ref().is_some_and(|c| !c.is_expired());
        let recent = if valid { self.gate.recent(MIN_LIVE_INTERVAL) } else { None };
        let mut snapshot = match recent {
            Some(snapshot) => snapshot,
            None => self.resolve(creds).await?,
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
        let (detected, creds) = run_blocking(|| (has_local_logs(), keychain::read_credentials()))
            .await
            .unwrap_or((false, None));
        let message = match (&creds, detected) {
            (Some(c), _) => format!("Claude Code signed in ({})", c.source_path),
            (None, true) => "Claude Code logs detected (not signed in)".to_string(),
            (None, false) => "Waiting for Claude Code".to_string(),
        };
        ProviderStatus {
            provider: ProviderId::Claude,
            health: if detected || creds.is_some() {
                ProviderHealth::Active
            } else {
                ProviderHealth::Waiting
            },
            message,
            checked_at: Utc::now(),
        }
    }

    fn compute_auth_state(&self, _ctx: &FetchContext) -> AuthState {
        let Some(c) = keychain::read_credentials() else {
            return AuthState::none(ProviderId::Claude);
        };
        AuthState {
            provider: ProviderId::Claude,
            kind: AuthKind::Oauth,
            source_path: c.source_path,
            expires_at_unix_secs: c.expires_at,
            last_refresh_iso: None,
            has_refresh_token: c.has_refresh_token,
            last_error: None,
        }
    }
}
