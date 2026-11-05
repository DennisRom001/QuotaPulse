mod descriptor;
mod gemini_api_key;
mod gemini_desktop;
mod gemini_quota;
mod live_fetcher;

use async_trait::async_trait;
use chrono::Utc;
use std::cmp::Ordering;

// Local-log fallback caps (placeholders; the live server supplies real limits).
const ANTIGRAVITY_CLI_DAILY_REQUEST_CAP: u64 = 200;

use crate::providers::auth::{AuthKind, AuthState};
use crate::providers::{
    run_blocking, FetchContext, LiveGate, ProviderDescriptor, UsageProvider, MIN_LIVE_INTERVAL,
};
use crate::usage::models::{
    CostEntry, DataProvenance, DataSource, PlanInfo, ProviderHealth, ProviderId, ProviderStatus,
    SourceView, UsageSnapshot, UsageUnit, UsageWindow, WindowType,
};
use crate::usage_scanners::{scan_antigravity_daily_usage, scan_antigravity_model_daily_usage};

/// Google Gemini provider: Antigravity IDE/CLI (live quota from its local
/// language server), Gemini CLI (quota from Google's Code Assist API using
/// the CLI's own login) and local Gemini CLI chat logs for token counts.
pub struct AntigravityProvider {
    descriptor: ProviderDescriptor,
    gate: LiveGate,
}

impl AntigravityProvider {
    pub fn new() -> Self {
        Self {
            descriptor: descriptor::descriptor(),
            gate: LiveGate::default(),
        }
    }
}

fn antigravity_installed() -> bool {
    let home_installed = dirs::home_dir()
        .map(|h| {
            h.join(".antigravity").exists()
                || h.join(".gemini").join("antigravity").exists()
                || h.join(".gemini").join("antigravity-cli").exists()
                || h.join(".gemini").join("oauth_creds.json").exists()
                || h.join("Library")
                    .join("Application Support")
                    .join("Antigravity")
                    .exists()
                || h.join("Library")
                    .join("Application Support")
                    .join("Antigravity IDE")
                    .exists()
        })
        .unwrap_or(false);

    let local_installed = dirs::data_local_dir()
        .map(|d| {
            d.join("Programs").join("Antigravity").exists()
                || d.join("Programs").join("Antigravity IDE").exists()
        })
        .unwrap_or(false);

    let roaming_installed = dirs::data_dir()
        .map(|d| d.join("Antigravity").exists() || d.join("Antigravity IDE").exists())
        .unwrap_or(false);

    home_installed || local_installed || roaming_installed
}

impl AntigravityProvider {
    /// Best available snapshot, following the source cascade.
    async fn primary(&self) -> Result<UsageSnapshot, String> {
        match live_fetcher::fetch_quota().await {
            Ok(live) => {
                tracing::info!(
                    "[antigravity] live fetch OK: plan={:?}, prompt_credits_available={:?}, monthly_credits={:?}, flow_credits_available={:?}, models_count={}",
                    live.plan,
                    live.available_prompt_credits,
                    live.monthly_prompt_credits,
                    live.available_flow_credits,
                    live.models.len()
                );
                let mut windows = Vec::new();

                // Headline the most-constrained model's rolling quota — the limit the
                // user hits first. The API exposes a remaining fraction + reset time.
                if let Some(model) = live.models.iter().min_by(|a, b| {
                    a.remaining_fraction
                        .partial_cmp(&b.remaining_fraction)
                        .unwrap_or(Ordering::Equal)
                }) {
                    let pct_used = (1.0 - model.remaining_fraction) * 100.0;
                    windows.push(UsageWindow::percent(
                        WindowType::FiveHour,
                        pct_used,
                        model.resets_at,
                        format!("{} — rolling model quota", model.name),
                    ));
                }

                // Monthly prompt credits as an exact meter.
                if let Some(monthly) = live.monthly_prompt_credits {
                    let available = live.available_prompt_credits.unwrap_or(monthly);
                    tracing::info!(
                        "[antigravity] monthly meter details: monthly={}, available={}, used={}",
                        monthly,
                        available,
                        monthly.saturating_sub(available)
                    );
                    windows.push(UsageWindow::exact(
                        WindowType::Monthly,
                        monthly.saturating_sub(available),
                        monthly,
                        None,
                        UsageUnit::Requests,
                    ));
                }

                if !windows.is_empty() {
                    let note = match (live.available_prompt_credits, live.available_flow_credits) {
                        (Some(p), Some(f)) => Some(format!("{p} prompt / {f} flow credits left")),
                        _ => None,
                    };
                    return Ok(UsageSnapshot {
                        provider: ProviderId::Antigravity,
                        windows,
                        credits: None,
                        plan: Some(PlanInfo {
                            tier: live.plan,
                            note,
                        }),
                        fetched_at: Utc::now(),
                        source: DataSource::Cli,
                        provenance: DataProvenance::Official,
                        stale: false,
                        badge: None,
                        sources: Vec::new(),
                    });
                }
            }
            Err(e) => {
                tracing::warn!("[antigravity] live fetch FAILED: {e}. Falling back to local logs.");
            }
        }

        // Gemini CLI signed in with Google: ask Google's Code Assist quota API.
        if let Ok(Some(creds)) = run_blocking(gemini_quota::read_creds).await {
            if gemini_quota::token_is_fresh(&creds) {
                if let Some(recent) = self.gate.recent(MIN_LIVE_INTERVAL) {
                    return Ok(recent);
                }
            }
            if gemini_quota::token_is_fresh(&creds) && self.gate.may_call() {
                match gemini_quota::fetch_quota(&creds.access_token).await {
                    Ok(buckets) => {
                        let mut windows = Vec::new();
                        if let Some(b) = buckets.iter().min_by(|a, b| {
                            a.remaining_fraction
                                .partial_cmp(&b.remaining_fraction)
                                .unwrap_or(Ordering::Equal)
                        }) {
                            windows.push(UsageWindow::percent(
                                WindowType::Daily,
                                (1.0 - b.remaining_fraction) * 100.0,
                                b.resets_at,
                                format!("{} — Gemini quota from Google", b.model),
                            ));
                        }
                        let snapshot = UsageSnapshot {
                            provider: ProviderId::Antigravity,
                            windows,
                            credits: None,
                            plan: Some(PlanInfo {
                                tier: Some("Gemini CLI".to_string()),
                                note: Some(format!("{} model quotas", buckets.len())),
                            }),
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
                        tracing::info!("[gemini] quota API unavailable: {e}");
                    }
                }
            }
            if let Some(cached) = self.gate.cached() {
                return Ok(cached);
            }
        }

        // Cascade after the live sources (Antigravity server, Gemini CLI quota):
        //   API key  -> "ACTIVE — API KEY" (Google exposes no usage for keys)
        //   desktop  -> "ACTIVE — DESKTOP APP" while the Gemini app runs
        //   local    -> token estimate from Gemini CLI chat logs, if any.
        // No placeholder bars: a window is only shown when there is real data.
        let api_key = run_blocking(gemini_api_key::find).await?;
        let key_valid = match &api_key {
            Some(key) => gemini_api_key::is_valid(key).await,
            None => false,
        };
        let (daily, desktop) =
            run_blocking(|| (scan_antigravity_daily_usage(), gemini_desktop::detect())).await?;

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let today_requests = daily
            .iter()
            .find(|p| p.day == today)
            .map(|p| p.total_tokens.min(ANTIGRAVITY_CLI_DAILY_REQUEST_CAP))
            .unwrap_or(0);
        let mut windows = Vec::new();
        if today_requests > 0 {
            windows.push(UsageWindow::approximate(
                WindowType::Daily,
                today_requests,
                ANTIGRAVITY_CLI_DAILY_REQUEST_CAP,
                None,
                UsageUnit::Requests,
                "Estimated from local Gemini CLI session logs.",
            ));
        }

        let (badge, plan) = if key_valid {
            let source = api_key.as_ref().map(|k| k.source.clone()).unwrap_or_default();
            (
                Some("ACTIVE — API KEY".to_string()),
                Some(PlanInfo {
                    tier: Some("Gemini API key".to_string()),
                    note: Some(format!(
                        "Key from {source} is valid. Google does not report quota usage for API keys."
                    )),
                }),
            )
        } else if let Some(d) = desktop.as_ref() {
            (
                d.running.then(|| "ACTIVE — DESKTOP APP".to_string()),
                Some(PlanInfo {
                    tier: Some(d.label()),
                    note: Some(d.status_line()),
                }),
            )
        } else {
            (None, None)
        };
        if api_key.is_some() && !key_valid {
            tracing::info!("[gemini] API key found but rejected by Google");
        }

        Ok(UsageSnapshot {
            provider: ProviderId::Antigravity,
            windows,
            credits: None,
            plan,
            fetched_at: Utc::now(),
            source: DataSource::LocalLog,
            provenance: DataProvenance::DerivedLocal,
            stale: false,
            badge,
            sources: Vec::new(),
        })
    }
}

/// State of every Gemini source, for the Antigravity / Gemini App toggle.
/// `main` is the snapshot the cascade picked; its figures are attached to the
/// source that produced it.
fn collect_sources(main: &UsageSnapshot) -> Vec<SourceView> {
    let from_antigravity =
        matches!(main.source, DataSource::Cli) && matches!(main.provenance, DataProvenance::Official);
    let from_cli = matches!(main.source, DataSource::Oauth);
    let mut out = Vec::new();

    out.push(if from_antigravity {
        SourceView {
            id: "antigravity".into(),
            label: "Antigravity".into(),
            state: "live".into(),
            badge: "LIVE".into(),
            windows: main.windows.clone(),
            plan: main.plan.clone(),
        }
    } else {
        SourceView {
            id: "antigravity".into(),
            label: "Antigravity".into(),
            state: "off".into(),
            badge: "OFF".into(),
            windows: Vec::new(),
            plan: Some(PlanInfo {
                tier: Some("Antigravity".into()),
                note: Some(if antigravity_installed() {
                    "Antigravity is installed but not running. Open it to get live quota.".into()
                } else {
                    "Antigravity is not installed.".into()
                }),
            }),
        }
    });

    if from_cli || gemini_quota::read_creds().is_some() {
        out.push(SourceView {
            id: "gemini_cli".into(),
            label: "Gemini CLI".into(),
            state: if from_cli { "live" } else { "off" }.into(),
            badge: if from_cli { "LIVE" } else { "OFF" }.into(),
            windows: if from_cli { main.windows.clone() } else { Vec::new() },
            plan: Some(PlanInfo {
                tier: Some("Gemini CLI".into()),
                note: Some(if from_cli {
                    "Quota from Google's Code Assist API.".into()
                } else {
                    "Gemini CLI login found but its token is expired; run `gemini` once.".into()
                }),
            }),
        });
    }

    if main.badge.as_deref() == Some("ACTIVE — API KEY") {
        out.push(SourceView {
            id: "api_key".into(),
            label: "API key".into(),
            state: "active".into(),
            badge: "ACTIVE — API KEY".into(),
            windows: Vec::new(),
            plan: main.plan.clone(),
        });
    }

    if let Some(desktop) = gemini_desktop::detect() {
        out.push(SourceView {
            id: "desktop".into(),
            label: "Gemini App".into(),
            state: if desktop.running { "active" } else { "off" }.into(),
            badge: if desktop.running { "ACTIVE — DESKTOP APP" } else { "OFF" }.into(),
            windows: Vec::new(),
            plan: Some(PlanInfo {
                tier: Some(desktop.label()),
                note: Some(desktop.status_line()),
            }),
        });
    }
    out
}

#[async_trait]
impl UsageProvider for AntigravityProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Antigravity
    }

    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn fetch_usage(&self, _ctx: &FetchContext) -> Result<UsageSnapshot, String> {
        let litellm = crate::providers::litellm_proxy::stats_for(ProviderId::Antigravity).await;
        let cascade = self.primary().await?;
        let mut sources = run_blocking({
            let main = cascade.clone();
            move || collect_sources(&main)
        })
        .await
        .unwrap_or_default();
        // LiteLLM (when it has Gemini traffic) is shown first and becomes one
        // more entry in the Antigravity / Gemini App toggle.
        let mut snapshot = match &litellm {
            Some((base, stats)) => {
                sources.insert(0, crate::providers::litellm_proxy::source_view(base, stats));
                crate::providers::litellm_proxy::snapshot(ProviderId::Antigravity, base, stats)
            }
            None => cascade,
        };
        snapshot.sources = sources;
        Ok(snapshot)
    }

    async fn fetch_cost_history(&self, _days: u32) -> Result<Vec<CostEntry>, String> {
        let points = run_blocking(scan_antigravity_model_daily_usage).await?;
        Ok(points
            .into_iter()
            .map(|p| CostEntry {
                date: p.day,
                provider: ProviderId::Antigravity,
                model: p.model,
                input_tokens: p.input_tokens,
                output_tokens: p.output_tokens,
                cache_read_tokens: p.cache_read_tokens,
                cache_write_tokens: 0,
                estimated_cost_usd: p.cost_usd,
            })
            .collect())
    }

    async fn check_status(&self) -> ProviderStatus {
        let (live, installed, desktop) = run_blocking(|| {
            (
                live_fetcher::discover_port().is_some(),
                antigravity_installed(),
                gemini_desktop::detect(),
            )
        })
        .await
        .unwrap_or((false, false, None));

        let (health, message) = if live {
            (
                ProviderHealth::Active,
                "Antigravity running — live quota".to_string(),
            )
        } else if let Some(desktop) = desktop {
            (ProviderHealth::Active, desktop.status_line())
        } else if installed {
            (
                ProviderHealth::Active,
                "Antigravity installed — start it for live quota".to_string(),
            )
        } else {
            (
                ProviderHealth::Waiting,
                "Waiting for Antigravity install".to_string(),
            )
        };

        ProviderStatus {
            provider: ProviderId::Antigravity,
            health,
            message,
            checked_at: Utc::now(),
        }
    }

    fn compute_auth_state(&self, _ctx: &FetchContext) -> AuthState {
        // Gemini CLI login (oauth_creds.json) takes precedence: it is the one
        // credential that can expire, so the UI can say "auth expired".
        if let Some(creds) = gemini_quota::read_creds() {
            return AuthState {
                provider: ProviderId::Antigravity,
                kind: AuthKind::Oauth,
                source_path: "~/.gemini/oauth_creds.json".to_string(),
                expires_at_unix_secs: creds.expires_at,
                last_refresh_iso: None,
                has_refresh_token: false,
                last_error: (!gemini_quota::token_is_fresh(&creds))
                    .then(|| "Gemini CLI token expired; run `gemini` once to renew it.".to_string()),
            };
        }
        if let Some(key) = gemini_api_key::find() {
            return AuthState {
                provider: ProviderId::Antigravity,
                kind: AuthKind::ApiKey,
                source_path: key.source,
                expires_at_unix_secs: None,
                last_refresh_iso: None,
                has_refresh_token: false,
                last_error: None,
            };
        }
        // Otherwise auth is handled by the local Antigravity server.
        if antigravity_installed() {
            return AuthState {
                provider: ProviderId::Antigravity,
                kind: AuthKind::Cli,
                source_path: "Antigravity local server".to_string(),
                expires_at_unix_secs: None,
                last_refresh_iso: None,
                has_refresh_token: false,
                last_error: if live_fetcher::discover_port().is_some() {
                    None
                } else {
                    Some("Start the Antigravity CLI/IDE for live quota.".to_string())
                },
            };
        }
        AuthState::none(ProviderId::Antigravity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_id() {
        let p = AntigravityProvider::new();
        assert_eq!(p.id(), ProviderId::Antigravity);
    }
}
