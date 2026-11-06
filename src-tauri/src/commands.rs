//! Tauri command handlers — the public surface area called from the React
//! frontend via `invoke(...)`.
//!
//! Each handler is a thin coordinator: it pulls shared state, delegates to
//! the relevant module (provider registry, usage store, aggregator), and
//! emits `usage-updated` events when snapshots change so background polls and
//! manual refreshes both feed the same UI listener.
//!
//! Handlers in this file are registered in `lib.rs::run` via
//! `tauri::generate_handler![...]`.

use chrono::Utc;
use tauri::{AppHandle, Emitter, State};

use crate::alerts::{build_alerts_with_thresholds, ThresholdConfig};
use crate::autostart::{launch_at_startup_enabled, set_launch_at_startup_enabled};
use crate::post_refresh::on_snapshots_updated;
use crate::providers::auth::AuthState;
use crate::tray::TrayTitleMode;
use crate::usage::aggregator;
use crate::usage::models::{
    CacheInsights, CostEntry, ModelBreakdownEntry, ProviderId, ProviderStatus, RecentActivityEntry,
    RefreshCadence, TrendData, UsageReport, UsageSnapshot,
};
use crate::usage_scanners;
use crate::{resolve_log_dir, restart_per_provider_scheduler, AppState};

// ───────────────────────── Snapshot reads ─────────────────────────

#[tauri::command]
pub async fn get_usage_snapshot(
    provider: ProviderId,
    state: State<'_, AppState>,
) -> Result<UsageSnapshot, String> {
    // Cached snapshots keep the UI responsive; the explicit refresh commands
    // are the paths that force a live backend fetch.
    if let Some(snapshot) = state.store.get_snapshot(provider)? {
        return Ok(snapshot);
    }
    aggregator::refresh_provider(
        &state.registry,
        &state.store,
        provider,
        &state.fetch_context(),
    )
    .await
}

#[tauri::command]
pub async fn get_all_snapshots(state: State<'_, AppState>) -> Result<Vec<UsageSnapshot>, String> {
    let snapshots = state.store.get_all_snapshots()?;
    // First boot has no cache yet, so bootstrap by refreshing providers once.
    if snapshots.is_empty() {
        return aggregator::refresh_all(&state.registry, &state.store, &state.fetch_context())
            .await;
    }
    Ok(snapshots)
}

// ───────────────────────── History / trends ─────────────────────────

#[tauri::command]
pub async fn get_cost_history(
    provider: ProviderId,
    days: u32,
    state: State<'_, AppState>,
) -> Result<Vec<CostEntry>, String> {
    state.store.get_cost_history(provider, days)
}

#[tauri::command]
pub async fn get_usage_trends(
    provider: ProviderId,
    days: u32,
    state: State<'_, AppState>,
) -> Result<TrendData, String> {
    state.store.get_usage_trends(provider, days.max(1))
}

#[tauri::command]
pub async fn get_model_breakdown(
    provider: ProviderId,
    days: u32,
    state: State<'_, AppState>,
) -> Result<Vec<ModelBreakdownEntry>, String> {
    state.store.get_model_breakdown(provider, days)
}

#[tauri::command]
pub async fn get_cache_insights(
    provider: ProviderId,
    state: State<'_, AppState>,
) -> Result<CacheInsights, String> {
    state
        .store
        .get_cache_insights(provider, Utc::now().date_naive())
}

#[tauri::command]
pub async fn get_recent_activity(
    provider: ProviderId,
    limit: u32,
) -> Result<Vec<RecentActivityEntry>, String> {
    Ok(usage_scanners::scan_recent_activity(
        provider,
        limit.max(1) as usize,
    ))
}

#[tauri::command]
pub async fn export_usage_report(
    days: u32,
    state: State<'_, AppState>,
) -> Result<UsageReport, String> {
    let snapshots = state.store.get_all_snapshots()?;
    let mut model_breakdowns = Vec::new();
    for provider in ProviderId::all() {
        model_breakdowns.extend(state.store.get_model_breakdown(provider, days.max(1))?);
    }

    // Build alerts per-provider so a user's custom threshold ladder applies to
    // reports (falling back to the fixed 75/90/95 ladder when a provider has no
    // configured thresholds).
    let thresholds = state
        .per_provider_thresholds
        .lock()
        .map_err(|_| "thresholds lock poisoned")?;
    let mut alerts = Vec::new();
    for provider in ProviderId::all() {
        let provider_snapshots: Vec<_> = snapshots
            .iter()
            .filter(|snapshot| snapshot.provider == provider)
            .cloned()
            .collect();
        let config = thresholds
            .get(&provider)
            .cloned()
            .unwrap_or_else(ThresholdConfig::default);
        alerts.extend(build_alerts_with_thresholds(&provider_snapshots, &config));
    }

    Ok(UsageReport {
        generated_at: Utc::now(),
        alerts,
        snapshots,
        model_breakdowns,
    })
}

// ───────────────────────── Refresh ─────────────────────────

#[tauri::command]
pub async fn refresh_provider(
    provider: ProviderId,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UsageSnapshot, String> {
    usage_scanners::invalidate_activity_cache();
    let snapshot = aggregator::refresh_provider(
        &state.registry,
        &state.store,
        provider,
        &state.fetch_context(),
    )
    .await?;
    let _ = app.emit("usage-updated", snapshot.clone());
    if let Ok(all) = state.store.get_all_snapshots() {
        on_snapshots_updated(&app, &all);
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn refresh_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<UsageSnapshot>, String> {
    usage_scanners::invalidate_activity_cache();
    let snapshots =
        aggregator::refresh_all(&state.registry, &state.store, &state.fetch_context()).await?;
    on_snapshots_updated(&app, &snapshots);
    let _ = app.emit("usage-updated", snapshots.clone());
    Ok(snapshots)
}

// ───────────────────────── Settings / lifecycle ─────────────────────────

#[tauri::command]
pub async fn get_provider_status(
    provider: ProviderId,
    state: State<'_, AppState>,
) -> Result<ProviderStatus, String> {
    let p = state
        .registry
        .get(provider)
        .ok_or_else(|| format!("Provider {provider:?} not found"))?;
    Ok(p.check_status().await)
}

#[tauri::command]
pub async fn get_auth_state(
    provider: ProviderId,
    state: State<'_, AppState>,
) -> Result<AuthState, String> {
    let p = state
        .registry
        .get(provider)
        .ok_or_else(|| format!("Provider {provider:?} not found"))?;
    Ok(p.compute_auth_state(&state.fetch_context()))
}

#[tauri::command]
pub fn get_log_directory(app: AppHandle) -> Result<String, String> {
    Ok(resolve_log_dir(Some(&app)).to_string_lossy().to_string())
}

/// Open the app's own log folder in the OS file manager. Done from Rust so the
/// webview needs no shell/filesystem permission at all.
#[tauri::command]
pub fn open_log_directory(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = resolve_log_dir(Some(&app));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_refresh_cadence(
    cadence: RefreshCadence,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut cadence_slot = state
            .cadence
            .lock()
            .map_err(|_| "cadence lock poisoned".to_string())?;
        *cadence_slot = cadence;
    }
    // The per-provider scheduler reads the global cadence on every tick;
    // restarting it applies the new value immediately.
    restart_per_provider_scheduler(&app, &state);
    Ok(())
}

#[tauri::command]
pub async fn set_per_provider_cadence(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: ProviderId,
    cadence: RefreshCadence,
) -> Result<(), String> {
    {
        let mut cadence_slot = state
            .per_provider_cadence
            .lock()
            .map_err(|_| "cadence lock poisoned".to_string())?;
        cadence_slot.insert(provider, cadence);
    }
    let app_handle = app.clone();
    restart_per_provider_scheduler(&app_handle, &state);
    Ok(())
}

#[tauri::command]
pub async fn get_per_provider_cadence(
    provider: ProviderId,
    state: State<'_, AppState>,
) -> Result<RefreshCadence, String> {
    let per_provider = state
        .per_provider_cadence
        .lock()
        .map_err(|_| "cadence lock poisoned".to_string())?
        .get(&provider)
        .copied();
    match per_provider {
        Some(cadence) => Ok(cadence),
        None => Ok(*state
            .cadence
            .lock()
            .map_err(|_| "cadence lock poisoned".to_string())?),
    }
}

#[tauri::command]
pub async fn get_launch_at_startup(app: AppHandle) -> Result<bool, String> {
    launch_at_startup_enabled(&app)
}

#[tauri::command]
pub async fn set_launch_at_startup(enabled: bool, app: AppHandle) -> Result<bool, String> {
    set_launch_at_startup_enabled(&app, enabled)
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

// ───────────────────────── Menu bar & notifications ─────────────────────────

/// Async so it runs off the main thread: re-rendering the tray waits on the
/// main thread, which must stay free.
#[tauri::command]
pub async fn set_tray_title_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: TrayTitleMode,
) -> Result<(), String> {
    *state
        .tray_title_mode
        .lock()
        .map_err(|_| "tray title lock poisoned")? = mode;
    crate::post_refresh::rerender_tray_title(&app);
    Ok(())
}

#[tauri::command]
pub fn get_tray_title_mode(state: State<'_, AppState>) -> Result<TrayTitleMode, String> {
    Ok(*state
        .tray_title_mode
        .lock()
        .map_err(|_| "tray title lock poisoned")?)
}

/// Backend notifications (threshold escalations, pace and budget warnings)
/// honour this switch; see `alert_engine`.
#[tauri::command]
pub fn set_notifications_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    *state
        .notifications_enabled
        .lock()
        .map_err(|_| "notifications lock poisoned")? = enabled;
    Ok(())
}

/// Whether the local LiteLLM Proxy answers, and which providers it has data for.
#[derive(serde::Serialize)]
pub struct LitellmStatus {
    pub up: bool,
    pub base_url: Option<String>,
    pub providers: Vec<ProviderId>,
}

#[tauri::command]
pub async fn get_litellm_status() -> Result<LitellmStatus, String> {
    Ok(match crate::providers::litellm_proxy::report().await {
        Some(report) => LitellmStatus {
            up: true,
            base_url: Some(report.base_url),
            providers: report.by_provider.keys().copied().collect(),
        },
        None => LitellmStatus {
            up: false,
            base_url: None,
            providers: Vec::new(),
        },
    })
}

/// Budget and billing-cycle day for the estimated Cursor usage. A missing or
/// zero budget turns the estimate off.
#[tauri::command]
pub fn set_cursor_estimate(monthly_prompts: Option<u32>, cycle_start_day: u8) {
    crate::providers::cursor::estimate::set_config(crate::providers::cursor::estimate::EstimateConfig {
        monthly_prompts,
        cycle_start_day,
    });
}

/// Compute the prompt budget that makes the estimate match the percentage
/// shown on cursor.com/dashboard. Reads Cursor's state DB locally, read-only.
#[tauri::command]
pub async fn calibrate_cursor_estimate(actual_percent: f64, cycle_start_day: u8) -> Result<u32, String> {
    crate::providers::run_blocking(move || {
        crate::providers::cursor::calibrate_estimate(actual_percent, cycle_start_day)
    })
    .await?
}
