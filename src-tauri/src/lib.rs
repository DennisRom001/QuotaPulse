//! Backend composition root.
//!
//! This file wires the Tauri app together: it owns [`AppState`], registers
//! Tauri commands, starts the system tray, attaches file watchers, and starts
//! the background poll scheduler. The actual implementation of each piece
//! lives in a dedicated module:
//!
//! | Concern                       | Module           |
//! |-------------------------------|------------------|
//! | Tauri command handlers        | [`commands`]     |
//! | Tray icon, menu, tooltip      | [`tray`]         |
//! | Alert generation              | [`alerts`]       |
//! | OS launch-at-startup wrapper  | [`autostart`]    |
//! | Provider implementations      | [`providers`]    |
//! | Snapshot persistence (SQLite) | [`usage::store`] |
//! | Background refresh logic      | [`usage::aggregator`] |
//! | Local CLI artifact scanning   | [`usage_scanners`]|
//! | Filesystem + poll watchers    | [`watchers`]     |

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter, Registry};

mod alert_engine;
mod alerts;
mod autostart;
mod budgets;
mod commands;
mod exporter;
mod limit_forecast;
mod litellm_launcher;
mod notifications;
mod post_refresh;
mod pricing;
mod providers;
mod session_usage;
mod tray;
mod tray_badges;
#[cfg(target_os = "windows")]
mod tray_promote;
mod usage;
mod usage_scanners;
mod watchers;

use crate::alerts::ThresholdConfig;
use post_refresh::on_snapshots_updated;
use providers::registry::ProviderRegistry;
use providers::FetchContext;
use usage::aggregator;
use usage::models::{ProviderId, RefreshCadence};
use usage::store::UsageStore;
use watchers::poll_scheduler::PollScheduler;

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// A provider counts as "in an active session" if its transcripts changed
/// within this window.
const ACTIVE_SESSION_WINDOW: std::time::Duration = std::time::Duration::from_secs(120);
/// Poll interval used while a Claude session is active. 30s keeps the figure
/// current without tripping Anthropic's rate limit (15s produced HTTP 429s).
const ACTIVE_SESSION_POLL_SECS: u64 = 30;

/// Backend composition root: registered providers, persisted store, transient
/// auth keys, and the active poll scheduler. One instance is created at
/// startup and stored in Tauri-managed state so commands can access it via
/// `State<'_, AppState>`.
pub struct AppState {
    pub registry: ProviderRegistry,
    pub store: UsageStore,
    pub cadence: Mutex<RefreshCadence>,
    pub per_provider_cadence: Mutex<HashMap<ProviderId, RefreshCadence>>,
    pub per_provider_thresholds: Mutex<HashMap<ProviderId, ThresholdConfig>>,
    pub scheduler: PollScheduler,
    pub tray_title_mode: Mutex<tray::TrayTitleMode>,
    pub notifications_enabled: Mutex<bool>,
    pub alert_memory: Mutex<alert_engine::AlertMemory>,
    pub budget_notified: Mutex<HashMap<ProviderId, bool>>,
}

impl AppState {
    fn new(app: &AppHandle) -> Result<Self, String> {
        let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let db_path = data_dir.join("usage.db");
        let store = UsageStore::open(&db_path)?;
        let registry = ProviderRegistry::new();

        let provider_banner = registry
            .descriptors()
            .into_iter()
            .map(|d| format!("{}:{}({})", d.id.as_str(), d.display_name, d.brand_color))
            .collect::<Vec<_>>()
            .join(", ");
        info!("Loaded providers: {provider_banner}");

        Ok(Self {
            registry,
            store,
            cadence: Mutex::new(RefreshCadence::Every2m),
            per_provider_cadence: Mutex::new(HashMap::new()),
            per_provider_thresholds: Mutex::new(HashMap::new()),
            scheduler: PollScheduler::new(),
            tray_title_mode: Mutex::new(tray::TrayTitleMode::default()),
            notifications_enabled: Mutex::new(true),
            alert_memory: Mutex::new(HashMap::new()),
            budget_notified: Mutex::new(HashMap::new()),
        })
    }

    /// Build the [`FetchContext`] handed to providers. Providers are purely
    /// local readers, so the context carries no credentials.
    pub fn fetch_context(&self) -> FetchContext {
        FetchContext::default()
    }

    /// Context for refreshes triggered by the file watcher.
    pub fn file_watcher_context(&self) -> FetchContext {
        FetchContext { from_watcher: true }
    }
}

// ─────────────────────────── tracing & panic hook ───────────────────────────

pub(crate) fn resolve_log_dir(app: Option<&AppHandle>) -> PathBuf {
    if let Some(app) = app {
        if let Ok(path) = app.path().app_data_dir() {
            return path.join("logs");
        }
    }
    if let Some(path) = dirs::data_dir() {
        return path.join("QuotaPulse").join("logs");
    }
    std::env::temp_dir().join("QuotaPulse").join("logs")
}

fn init_tracing() {
    let log_dir = resolve_log_dir(None);
    if let Err(err) = fs::create_dir_all(&log_dir) {
        let _ = writeln!(
            std::io::stderr(),
            "failed to create log dir {}: {err}",
            log_dir.display()
        );
        return;
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "quotapulse.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,quotapulse=info"));
    let console_layer = fmt::layer()
        .compact()
        .with_writer(std::io::stderr)
        .with_target(true);
    let file_layer = fmt::layer()
        .compact()
        .with_ansi(false)
        .with_writer(non_blocking);
    let subscriber = Registry::default()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer);
    let _ = tracing::subscriber::set_global_default(subscriber);
    info!("tracing initialized; log_dir={}", log_dir.display());
}

fn panic_log_path() -> PathBuf {
    if let Some(dir) = dirs::data_local_dir() {
        return dir.join("QuotaPulse").join("last_panic.log");
    }
    if let Some(home) = dirs::home_dir() {
        return home
            .join(".local")
            .join("share")
            .join("QuotaPulse")
            .join("last_panic.log");
    }
    std::env::temp_dir().join("QuotaPulse_last_panic.log")
}

fn append_panic_log(message: &str) {
    let path = panic_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{message}");
    }
}

// ─────────────────────────── scheduler & watchers ───────────────────────────

/// Restart the per-provider poll scheduler.
///
/// Each provider gets its own timer. On every tick the closure resolves that
/// provider's effective cadence — a per-provider override from
/// [`AppState::per_provider_cadence`], or the global [`AppState::cadence`]
/// fallback — and refreshes only that provider once the interval has elapsed.
/// Unlike the manual-refresh commands it intentionally skips
/// `usage_scanners::invalidate_activity_cache()` so background polls stay
/// lightweight.
pub fn restart_per_provider_scheduler(app: &AppHandle, state: &AppState) {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let app_handle = app.clone();
    let last_refresh: Arc<Mutex<HashMap<ProviderId, Instant>>> =
        Arc::new(Mutex::new(HashMap::new()));
    state.scheduler.restart_per_provider(move |provider| {
        let app_inner = app_handle.clone();
        let last_refresh = last_refresh.clone();
        tauri::async_runtime::spawn(async move {
            let state = app_inner.state::<AppState>();
            // Read each mutex separately so the two guards are never held together.
            let per_provider = state
                .per_provider_cadence
                .lock()
                .map_err(|e| e.to_string())?
                .get(&provider)
                .copied();
            let cadence = match per_provider {
                Some(c) => c,
                None => *state.cadence.lock().map_err(|e| e.to_string())?,
            };
            if cadence == RefreshCadence::Manual {
                return Ok::<(), String>(());
            }

            // While a Claude Code session is writing transcripts, poll every 30s
            // so the live 5-hour figure tracks the session; otherwise use
            // the configured cadence to save API calls.
            let seconds = if provider == ProviderId::Claude
                && watchers::activity::is_active(provider, ACTIVE_SESSION_WINDOW)
            {
                ACTIVE_SESSION_POLL_SECS.min(cadence.seconds().unwrap_or(u64::MAX))
            } else {
                cadence.seconds().unwrap_or_default()
            };
            let now = Instant::now();
            let eligible = last_refresh
                .lock()
                .map_err(|e| e.to_string())?
                .get(&provider)
                .map(|last| now.duration_since(*last) >= Duration::from_secs(seconds))
                .unwrap_or(true);
            if !eligible {
                return Ok::<(), String>(());
            }
            if let Ok(mut guard) = last_refresh.lock() {
                guard.insert(provider, now);
            }

            if let Ok(snapshot) = aggregator::refresh_provider(
                &state.registry,
                &state.store,
                provider,
                &state.fetch_context(),
            )
            .await
            {
                if let Ok(all) = state.store.get_all_snapshots() {
                    on_snapshots_updated(&app_inner, &all);
                }
                let _ = app_inner.emit("usage-updated", snapshot);
            }
            Ok(())
        });
    });
}

/// Attach the filesystem watcher for every provider's local artifact
/// directories.
///
/// File-system events are treated the same as manual refreshes: refresh the
/// affected provider, update the tray, then emit the new snapshot. Events are
/// already debounced per provider by [`watchers::file_watcher`].
fn start_file_watchers(app: &AppHandle) {
    let app_handle = app.clone();
    watchers::file_watcher::start(move |provider| {
        let app_inner = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            usage_scanners::invalidate_activity_cache();
            let state = app_inner.state::<AppState>();
            if let Ok(snapshot) = aggregator::refresh_provider(
                &state.registry,
                &state.store,
                provider,
                &state.file_watcher_context(),
            )
            .await
            {
                if let Ok(all) = state.store.get_all_snapshots() {
                    on_snapshots_updated(&app_inner, &all);
                }
                let _ = app_inner.emit("usage-updated", snapshot);
            }
        });
    });
}

// ─────────────────────────── data migration ───────────────────────────

/// Bundle identifier used by builds before the QuotaPulse rename.
const PREVIOUS_IDENTIFIER: &str = "com.opentokenmonitor.desktop";
const IDENTIFIER: &str = "com.quotapulse.desktop";

fn copy_dir_recursive(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            // Locked/transient files (caches) are skipped rather than fatal.
            let _ = fs::copy(entry.path(), &target);
        }
    }
    Ok(())
}

/// One-time copy of settings, usage history and the WebView profile from the
/// previous identifier's folders, so the rename loses nothing. Runs before
/// any window or store is opened; the old folders are left untouched.
fn migrate_previous_identifier() {
    for base in [dirs::data_dir(), dirs::data_local_dir()].into_iter().flatten() {
        let old = base.join(PREVIOUS_IDENTIFIER);
        let new = base.join(IDENTIFIER);
        if old.is_dir() && !new.exists() {
            match copy_dir_recursive(&old, &new) {
                Ok(()) => info!("migrated app data {} -> {}", old.display(), new.display()),
                Err(err) => error!("app data migration failed for {}: {err}", old.display()),
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::panic::set_hook(Box::new(|panic_info| {
        let now = chrono::Utc::now().to_rfc3339();
        let thread = std::thread::current()
            .name()
            .map(str::to_string)
            .unwrap_or_else(|| "unnamed".to_string());
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "non-string panic payload".to_string()
        };
        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture();
        let log_line = format!(
            "[{now}] panic thread={thread} location={location} payload={payload}\nbacktrace:\n{backtrace}\n"
        );
        append_panic_log(&log_line);
        eprintln!("{log_line}");
    }));

    init_tracing();
    migrate_previous_identifier();

    let mut builder = tauri::Builder::default();

    // Single-instance enforcement: when a second copy launches (e.g. user
    // clicks the shortcut while the autostart copy is already running in the
    // tray), focus the existing instance instead of spawning a duplicate.
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));
    }

    let run_result = builder
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
                use tauri_plugin_autostart::MacosLauncher;

                app.handle().plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    Some(vec!["--autostart"]),
                ))?;

                // Self-heal: if autostart is enabled, rewrite the entry to point
                // at this (installed/release) binary. Corrects a stale path left
                // by a prior `tauri dev` session, which otherwise launches a dev
                // build with no UI server at boot and appears to crash.
                autostart::heal_autostart_path(app.handle());
            }

            let state = AppState::new(app.handle())
                .map_err(|e| tauri::Error::Io(std::io::Error::other(e)))?;
            app.manage(state);

            let launched_from_autostart = autostart::is_autostart_launch();
            if launched_from_autostart {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            tray::setup_tray(app)?;
            litellm_launcher::ensure_running(app.handle());
            start_file_watchers(app.handle());

            {
                let state_ref = app.state::<AppState>();
                restart_per_provider_scheduler(app.handle(), &state_ref);
            }

            // Initial bootstrap refresh so the UI has data on first paint.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                if let Ok(snapshots) =
                    aggregator::refresh_all(&state.registry, &state.store, &state.fetch_context())
                        .await
                {
                    on_snapshots_updated(&app_handle, &snapshots);
                    let _ = app_handle.emit("usage-updated", snapshots);
                }
            });

            // For non-autostart launches, show and focus the main window right
            // away (no artificial delay).
            if !launched_from_autostart {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_usage_snapshot,
            commands::get_all_snapshots,
            commands::get_cost_history,
            commands::get_usage_trends,
            commands::get_model_breakdown,
            commands::get_cache_insights,
            commands::get_recent_activity,
            commands::export_usage_report,
            commands::refresh_provider,
            commands::refresh_all,
            commands::get_provider_status,
            commands::get_auth_state,
            commands::get_log_directory,
            commands::open_log_directory,
            commands::get_litellm_status,
            commands::set_refresh_cadence,
            commands::get_launch_at_startup,
            commands::set_launch_at_startup,
            commands::quit_app,
            notifications::notify_threshold_alert,
            notifications::notify_usage_alerts,
            notifications::set_thresholds,
            budgets::get_budgets,
            budgets::set_budget,
            budgets::get_forecast,
            exporter::export_report,
            commands::set_per_provider_cadence,
            commands::get_per_provider_cadence,
            commands::set_tray_title_mode,
            commands::get_tray_title_mode,
            commands::set_notifications_enabled,
            limit_forecast::get_limit_forecasts,
            session_usage::get_session_usage,
            session_usage::get_project_usage,
        ])
        .run(tauri::generate_context!());

    if let Err(err) = run_result {
        let now = chrono::Utc::now().to_rfc3339();
        let log_line = format!("[{now}] tauri runtime error: {err}");
        append_panic_log(&log_line);
        error!("{log_line}");
    }
}
