mod descriptor;
mod keychain;
mod log_parser;
mod oauth_fetcher;

use async_trait::async_trait;
use chrono::Utc;
use tracing::info;

use crate::providers::auth::{AuthKind, AuthState};
use crate::providers::{
    run_blocking, FetchContext, LiveGate, ProviderDescriptor, UsageProvider, MIN_LIVE_INTERVAL,
};
use crate::usage::models::{
    CostEntry, CreditsInfo, DataProvenance, DataSource, PlanInfo, ProviderHealth, ProviderId,
    ProviderStatus, UsageSnapshot, UsageWindow, WindowType,
};
use crate::usage_scanners::resolve_claude_project_roots;

/// Approximate Pro-plan token budgets used only when the live API is not
/// reachable and the meters fall back to local log totals.

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

/// Snapshot from local Claude Code logs when no live session is available.
///
/// Claude's plan limits are not published as token counts (they depend on
/// plan, model and seat type), so local tokens are shown as plain totals —
/// never as a percentage of an invented limit, which used to read e.g. ~87%
/// while the real weekly usage was 17%.
fn local_log_snapshot(note: Option<String>) -> UsageSnapshot {
    let (day_tokens, week_tokens, _) = log_parser::usage_windows();
    let now = Utc::now();
    let totals = format!(
        "Logs locales: {} tokens hoy · {} en 7 días.",
        compact_tokens(day_tokens),
        compact_tokens(week_tokens)
    );
    let note = match note {
        Some(n) => format!("{n} {totals}"),
        None => totals,
    };
    UsageSnapshot {
        provider: ProviderId::Claude,
        windows: Vec::new(),
        credits: None,
        plan: Some(PlanInfo {
            tier: None,
            note: Some(note),
        }),
        fetched_at: now,
        source: DataSource::LocalLog,
        provenance: DataProvenance::DerivedLocal,
        stale: false,
        badge: None,
        hint: None,
        sources: Vec::new(),
    }
}

fn compact_tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}K", n as f64 / 1_000.0),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

/// Spanish card hint for a session Claude Code ended itself.
fn ended_session_hint(s: &keychain::EndedSession) -> String {
    let when = s
        .refresh_expired_at
        .and_then(|t| chrono::DateTime::from_timestamp(t as i64, 0))
        .map(|d| format!(" el {}", d.with_timezone(&chrono::Local).format("%d/%m/%Y")))
        .unwrap_or_default();
    let why = match (s.reason.as_deref(), s.subscription.as_deref()) {
        (Some("time_limit"), Some(plan)) if plan.eq_ignore_ascii_case("team") || plan.eq_ignore_ascii_case("enterprise") => {
            "por el límite de duración de sesión de tu organización".to_string()
        }
        (Some("time_limit"), _) => "por límite de tiempo".to_string(),
        (Some(r), _) => format!("({r})"),
        (None, _) => "y borró los tokens".to_string(),
    };
    format!(
        "Claude Code cerró tu sesión{when} {why}. Pulsa «Iniciar sesión en Claude» (o ejecuta \
         `claude auth login`); la cuota en vivo volverá sola en cuanto termines."
    )
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
        hint: None,
        sources: Vec::new(),
    }
}

impl ClaudeProvider {
    async fn resolve(&self, creds: Option<keychain::ClaudeCreds>) -> Result<UsageSnapshot, String> {
        // `read_credentials` already prefers any fresh token (env, OS store,
        // file), and the file watcher re-reads them the moment Claude Code
        // renews its session. So "expired" here means every source is
        // expired: only Claude Code itself (or a long-lived setup-token) can
        // renew it, and the card says how.
        let mut hint = None;
        let note = match creds {
            None => match run_blocking(keychain::ended_session).await? {
                Some(ended) => {
                    info!(
                        "[claude] Claude Code session ended (loginEnded={:?}, refresh token valid until {:?}, plan {:?}) in {}; sign in again with `claude` → /login",
                        ended.reason, ended.refresh_expired_at, ended.subscription, ended.source_path
                    );
                    hint = Some(ended_session_hint(&ended));
                    Some("Sesión de Claude Code cerrada; vuelve a iniciar sesión para ver la cuota en vivo.".to_string())
                }
                None => {
                    info!("[claude] no Claude Code credentials found; using local logs");
                    hint = Some(
                        "No se encontró ninguna sesión de Claude Code. Pulsa «Iniciar sesión en Claude» (o ejecuta `claude auth login`) para ver la cuota en vivo."
                            .to_string(),
                    );
                    Some("Sin sesión de Claude Code; inicia sesión para ver la cuota en vivo.".to_string())
                }
            },
            Some(c) if c.is_expired() => {
                info!("[claude] OAuth token in {} is expired; using local logs", c.source_path);
                hint = Some(
                    concat!(
                        "Sesión de Claude caducada: pulsa «Iniciar sesión en Claude» (o abre ",
                        "Claude Code) y se renovará sola aquí. Para que no caduque, crea un token de ",
                        "larga duración con `claude setup-token` y guárdalo en la ",
                        "variable CLAUDE_CODE_OAUTH_TOKEN."
                    )
                    .to_string(),
                );
                Some("Sesión de Claude Code caducada; vuelve a iniciar sesión para ver la cuota en vivo.".to_string())
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
        let mut snapshot = run_blocking(move || local_log_snapshot(note)).await?;
        snapshot.hint = hint;
        Ok(snapshot)
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
