//! The single hook every refresh path calls once it has fresh snapshots:
//! bootstrap, poll schedulers, file watchers, tray "Refresh All" and the
//! manual refresh commands. Keeps the tray tooltip, the menu-bar badges and the
//! backend alert engine in step no matter which path produced the data.

use chrono::Utc;
use tauri::{AppHandle, Manager};

use crate::usage::models::{ProviderId, UsageSnapshot};
use crate::{alert_engine, tray, AppState};

pub fn on_snapshots_updated(app: &AppHandle, snapshots: &[UsageSnapshot]) {
    tray::update_tray_tooltip(app, snapshots);
    render_tray_title(app, snapshots);
    alert_engine::evaluate(app, snapshots);
    notify_claude_session_change(app, snapshots);
}

/// Claude session as last seen: 0 = unknown, 1 = live, 2 = no session.
static CLAUDE_SESSION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// One desktop notification when the Claude session goes from live to
/// ended/expired while the window is hidden or minimised. It fires only on
/// that transition, never on every poll, and re-arms once the session is
/// live again.
fn notify_claude_session_change(app: &AppHandle, snapshots: &[UsageSnapshot]) {
    use std::sync::atomic::Ordering;
    let Some(claude) = snapshots.iter().find(|s| s.provider == ProviderId::Claude) else {
        return;
    };
    let live = !matches!(claude.source, crate::usage::models::DataSource::LocalLog);
    let ended = !live && claude.hint.is_some();
    let now = if live { 1 } else if ended { 2 } else { return };
    let before = CLAUDE_SESSION.swap(now, Ordering::Relaxed);
    if !(before == 1 && now == 2) {
        return;
    }
    let enabled = app
        .try_state::<AppState>()
        .and_then(|s| s.notifications_enabled.lock().ok().map(|g| *g))
        .unwrap_or(false);
    let on_screen = app.get_webview_window("main").is_some_and(|w| {
        w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false)
    });
    if enabled && !on_screen {
        crate::notifications::show_notification(
            app,
            "QuotaPulse: sesión de Claude caducada",
            "Claude Code cerró la sesión. Abre QuotaPulse y pulsa «Iniciar sesión en Claude» para recuperar la cuota en vivo.",
        );
    }
}

/// Re-render the menu-bar title from the stored snapshots, e.g. after the
/// title mode changes.
pub fn rerender_tray_title(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Ok(snapshots) = state.store.get_all_snapshots() {
        render_tray_title(app, &snapshots);
    }
}

fn render_tray_title(app: &AppHandle, snapshots: &[UsageSnapshot]) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let mode = state.tray_title_mode.lock().map(|g| *g).unwrap_or_default();
    let title = tray::strip_title(mode, today_cost_usd(&state));
    // With the floating capsule on, the tray is the single app icon: the
    // capsule already shows every provider, so no per-provider badges.
    let badges = if crate::capsule::is_enabled(app) {
        Vec::new()
    } else {
        tray::build_badges(mode, snapshots)
    };
    tray::sync_provider_badges(app, &badges, title);
}

/// Today's estimated spend across providers. Cost rows are keyed by UTC
/// day, matching how the log scanners bucket usage.
fn today_cost_usd(state: &AppState) -> f64 {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    ProviderId::all()
        .into_iter()
        .filter_map(|p| state.store.get_cost_history(p, 1).ok())
        .flatten()
        .filter(|e| e.date == today)
        .map(|e| e.estimated_cost_usd)
        .sum()
}
