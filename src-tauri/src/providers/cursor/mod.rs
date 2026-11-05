//! Cursor provider.
//!
//! Cursor keeps its signed-in session in the editor's own SQLite state DB
//! (`%APPDATA%\Cursor\User\globalStorage\state.vscdb` on Windows). We open it
//! read-only, take the session token, and ask `cursor.com` — the same endpoint
//! Cursor's dashboard uses — for the plan's request usage this billing cycle.
//! Cursor writes no per-request token logs to disk, so there is no local
//! token history for this provider.

use std::path::PathBuf;

use async_trait::async_trait;
use chrono::{DateTime, Months, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use tracing::info;

use crate::providers::auth::{AuthKind, AuthState};
use crate::providers::http::{self, jwt_claims, jwt_expires_at, now_unix};
use crate::providers::{
    run_blocking, FetchContext, LiveGate, ProviderDescriptor, UsageProvider, MIN_LIVE_INTERVAL,
};
use crate::usage::models::{
    CostEntry, DataProvenance, DataSource, PlanInfo, ProviderHealth, ProviderId, ProviderStatus,
    UsageSnapshot, UsageUnit, UsageWindow, WindowType,
};

pub struct CursorProvider {
    descriptor: ProviderDescriptor,
    gate: LiveGate,
}

impl CursorProvider {
    pub fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: ProviderId::Cursor,
                display_name: "Cursor",
                brand_color: "#7c8cff",
            },
            gate: LiveGate::default(),
        }
    }
}

fn state_db_path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb"),
    )
}

struct CursorSession {
    user_id: String,
    access_token: String,
    membership: Option<String>,
    expires_at: Option<u64>,
    source_path: String,
}

fn read_item(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM ItemTable WHERE key = ?1", [key], |row| {
        let v = row.get_ref(0)?;
        Ok(v.as_str()
            .map(str::to_string)
            .ok()
            .or_else(|| v.as_blob().ok().map(|b| String::from_utf8_lossy(b).into_owned())))
    })
    .optional()
    .ok()
    .flatten()
    .flatten()
    .map(|s| s.trim().trim_matches('"').to_string())
    .filter(|s| !s.is_empty())
}

fn read_session() -> Option<CursorSession> {
    match try_read_session() {
        Ok(session) => Some(session),
        Err(reason) => {
            // Reason only — never the token itself.
            tracing::debug!("[cursor] session unavailable: {reason}");
            LAST_SESSION_ERROR
                .lock()
                .map(|mut g| *g = Some(reason))
                .ok();
            None
        }
    }
}

static LAST_SESSION_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn last_session_error() -> String {
    LAST_SESSION_ERROR
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn try_read_session() -> Result<CursorSession, String> {
    let path = state_db_path().ok_or("no config dir")?;
    if !path.is_file() {
        return Err(format!("{} not found", path.display()));
    }
    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("cannot open state DB: {e}"))?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
    let access_token =
        read_item(&conn, "cursorAuth/accessToken").ok_or("key cursorAuth/accessToken missing (not signed in?)")?;
    let claims = jwt_claims(&access_token).ok_or_else(|| describe_token_shape(&access_token))?;
    let sub = claims
        .get("sub")
        .and_then(Value::as_str)
        .ok_or("token has no sub claim")?
        .to_string();
    // `sub` looks like "auth0|user_01ABC…"; the dashboard API wants the id part.
    let user_id = sub.rsplit('|').next().unwrap_or(&sub).to_string();
    Ok(CursorSession {
        expires_at: jwt_expires_at(&access_token),
        membership: read_item(&conn, "cursorAuth/stripeMembershipType"),
        user_id,
        access_token,
        source_path: path.display().to_string(),
    })
}

/// Explain why a token could not be decoded, without revealing it.
fn describe_token_shape(token: &str) -> String {
    use base64::Engine;
    let parts: Vec<&str> = token.split('.').collect();
    let decode = parts.get(1).map(|p| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(p.trim_end_matches('='))
            .map(|b| match serde_json::from_slice::<Value>(&b) {
                Ok(_) => "payload ok".to_string(),
                Err(e) => format!("payload not JSON: {e}"),
            })
            .unwrap_or_else(|e| format!("payload base64 error: {e}"))
    });
    format!(
        "access token not decodable (len={}, parts={}, starts_with_quote={}, {})",
        token.len(),
        parts.len(),
        token.starts_with('"') || token.starts_with('{'),
        decode.unwrap_or_else(|| "no payload segment".to_string())
    )
}

struct CursorUsage {
    requests: u64,
    limit: Option<u64>,
    cycle_start: Option<DateTime<Utc>>,
}

async fn fetch_usage(session: &CursorSession) -> Result<CursorUsage, String> {
    let url = format!("https://cursor.com/api/usage?user={}", session.user_id);
    let cookie = format!(
        "WorkosCursorSessionToken={}%3A%3A{}",
        session.user_id, session.access_token
    );
    let req = http::get(&url)?.header(reqwest::header::COOKIE, cookie);
    let json = http::send_json(req).await?;

    // Premium-model bucket ("gpt-4" key historically); otherwise the busiest
    // model bucket that reports a request count.
    let buckets: Vec<&Value> = json
        .as_object()
        .ok_or("unexpected usage payload")?
        .values()
        .filter(|v| v.get("numRequests").is_some())
        .collect();
    let premium = json
        .get("gpt-4")
        .filter(|v| v.get("numRequests").is_some())
        .or_else(|| {
            buckets
                .iter()
                .copied()
                .max_by_key(|v| v.get("numRequests").and_then(Value::as_u64).unwrap_or(0))
        })
        .ok_or("usage payload had no request counters")?;

    Ok(CursorUsage {
        requests: premium
            .get("numRequests")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        limit: premium.get("maxRequestUsage").and_then(Value::as_u64),
        cycle_start: json
            .get("startOfMonth")
            .and_then(Value::as_str)
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc)),
    })
}

fn snapshot_from(usage: CursorUsage, membership: Option<String>) -> UsageSnapshot {
    let resets_at = usage.cycle_start.and_then(|s| s.checked_add_months(Months::new(1)));
    let window = match usage.limit.filter(|l| *l > 0) {
        Some(limit) => UsageWindow::exact(
            WindowType::Monthly,
            usage.requests,
            limit,
            resets_at,
            UsageUnit::Requests,
        ),
        None => UsageWindow::approximate(
            WindowType::Monthly,
            usage.requests,
            usage.requests.max(1),
            resets_at,
            UsageUnit::Requests,
            format!(
                "{} requests this cycle; this plan reports no fixed request cap.",
                usage.requests
            ),
        ),
    };
    UsageSnapshot {
        provider: ProviderId::Cursor,
        windows: vec![window],
        credits: None,
        plan: Some(PlanInfo {
            tier: membership,
            note: None,
        }),
        fetched_at: Utc::now(),
        source: DataSource::Cookie,
        provenance: DataProvenance::Internal,
        stale: false,
        badge: None,
        sources: Vec::new(),
    }
}

/// Snapshot without usage figures (no fake 0% bar). When Cursor.exe is
/// running the card says so with an "ACTIVE — LOCAL SESSION" badge.
fn placeholder(note: &str) -> UsageSnapshot {
    let running = cursor_running();
    UsageSnapshot {
        provider: ProviderId::Cursor,
        windows: Vec::new(),
        credits: None,
        plan: Some(PlanInfo {
            tier: Some(if running { "Cursor (open)" } else { "Cursor" }.to_string()),
            note: Some(note.to_string()),
        }),
        fetched_at: Utc::now(),
        source: DataSource::LocalLog,
        provenance: DataProvenance::DerivedLocal,
        stale: false,
        badge: running.then(|| "ACTIVE — LOCAL SESSION".to_string()),
        sources: Vec::new(),
    }
}

fn cursor_running() -> bool {
    crate::providers::process::is_running("Cursor.exe")
}

#[async_trait]
impl UsageProvider for CursorProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Cursor
    }

    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn fetch_usage(&self, _ctx: &FetchContext) -> Result<UsageSnapshot, String> {
        // A running LiteLLM Proxy with traffic for this provider wins; otherwise
        // fall through to the normal source cascade.
        if let Some((base, stats)) =
            crate::providers::litellm_proxy::stats_for(ProviderId::Cursor).await
        {
            return Ok(crate::providers::litellm_proxy::snapshot(ProviderId::Cursor, &base, &stats));
        }
        let Some(session) = run_blocking(read_session).await? else {
            let reason = last_session_error();
            info!("[cursor] no usable session: {reason}");
            let note = if reason.contains("not decodable") {
                "Cursor keeps its session encrypted on disk, so request counts cannot be read locally."
            } else {
                "Sign in to Cursor to see plan usage."
            };
            return run_blocking(move || placeholder(note)).await;
        };
        if session.expires_at.is_some_and(|exp| exp <= now_unix() + 30) {
            info!("[cursor] session token expired");
            return Ok(placeholder("Cursor session expired; open Cursor to renew it."));
        }
        if let Some(recent) = self.gate.recent(MIN_LIVE_INTERVAL) {
            return Ok(recent);
        }
        if !self.gate.may_call() {
            return Ok(self
                .gate
                .cached()
                .unwrap_or_else(|| placeholder("Cursor usage temporarily unavailable.")));
        }
        match fetch_usage(&session).await {
            Ok(usage) => {
                info!(
                    "[cursor] live usage OK: {} requests (limit {:?})",
                    usage.requests, usage.limit
                );
                let snapshot = snapshot_from(usage, session.membership);
                self.gate.ok(&snapshot);
                Ok(snapshot)
            }
            Err(e) => {
                self.gate.failed();
                info!("[cursor] usage API unavailable: {e}");
                Ok(self
                    .gate
                    .cached()
                    .unwrap_or_else(|| placeholder("Cursor usage temporarily unavailable.")))
            }
        }
    }

    async fn fetch_cost_history(&self, _days: u32) -> Result<Vec<CostEntry>, String> {
        Ok(Vec::new())
    }

    async fn check_status(&self) -> ProviderStatus {
        let (installed, signed_in, running) = run_blocking(|| {
            (
                state_db_path().is_some_and(|p| p.is_file()),
                read_session().is_some(),
                cursor_running(),
            )
        })
        .await
        .unwrap_or((false, false, false));
        ProviderStatus {
            provider: ProviderId::Cursor,
            health: if installed || running {
                ProviderHealth::Active
            } else {
                ProviderHealth::Waiting
            },
            message: match (signed_in, running, installed) {
                (true, _, _) => "Cursor session found".to_string(),
                (false, true, _) => "Cursor is running (local session)".to_string(),
                (false, false, true) => "Cursor installed (not running)".to_string(),
                _ => "Waiting for Cursor install".to_string(),
            },
            checked_at: Utc::now(),
        }
    }

    fn compute_auth_state(&self, _ctx: &FetchContext) -> AuthState {
        let Some(s) = read_session() else {
            return AuthState::none(ProviderId::Cursor);
        };
        AuthState {
            provider: ProviderId::Cursor,
            kind: AuthKind::Cookie,
            source_path: s.source_path,
            expires_at_unix_secs: s.expires_at,
            last_refresh_iso: None,
            has_refresh_token: false,
            last_error: None,
        }
    }
}
