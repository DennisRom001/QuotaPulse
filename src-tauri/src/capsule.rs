//! Floating capsule ("isla"): a small borderless, always-on-top strip near the
//! top of the screen that shows the monitored AIs and their real state.
//!
//! While the capsule is on (the default):
//! * the main window has no taskbar button and starts hidden; a click on the
//!   capsule shows / hides it;
//! * the tray collapses to the single QuotaPulse icon (no per-provider
//!   badges), which stays as the control and fallback (menu, show / hide).
//!
//! The capsule only *reads* what the backend already has: it renders the same
//! snapshots the main window gets (cached store + `usage-updated` events) and
//! never triggers quota queries of its own.
//!
//! On / off and the dragged position persist in `capsule.json` in the app
//! config directory, so the backend knows the state before any webview loads.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub const LABEL: &str = "capsule";
const FILE: &str = "capsule.json";
/// Initial logical size; the frontend fits the width to its content.
const DEFAULT_SIZE: (f64, f64) = (180.0, 36.0);
/// Gap between the top of the monitor and the capsule, in logical px.
const TOP_MARGIN: f64 = 8.0;
/// Bounds for `capsule_resize`, in logical px.
const WIDTH_RANGE: (f64, f64) = (60.0, 520.0);
const HEIGHT_RANGE: (f64, f64) = (28.0, 64.0);
/// Wait this long after the last move before writing the position.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapsuleConfig {
    pub enabled: bool,
    /// Last position (physical px, top-left of the window), if ever dragged.
    pub x: Option<i32>,
    pub y: Option<i32>,
}

impl Default for CapsuleConfig {
    fn default() -> Self {
        Self { enabled: true, x: None, y: None }
    }
}

pub struct CapsuleState {
    config: Mutex<CapsuleConfig>,
    /// Bumped on every move; a pending save only writes if it is still the latest.
    move_generation: AtomicU64,
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join(FILE))
}

fn load(app: &AppHandle) -> CapsuleConfig {
    config_path(app)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, config: CapsuleConfig) {
    let Some(path) = config_path(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(text) = serde_json::to_string_pretty(&config) else { return };
    // Write-then-rename so a crash never leaves a half-written file.
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

fn current(app: &AppHandle) -> Option<CapsuleConfig> {
    app.try_state::<CapsuleState>()
        .and_then(|s| s.config.lock().ok().map(|c| *c))
}

/// Whether the capsule mode is on (false before setup has run).
pub fn is_enabled(app: &AppHandle) -> bool {
    current(app).is_some_and(|c| c.enabled)
}

/// Top-centre of a monitor for a window `window_width` px wide.
fn centered_top(monitor_pos: (i32, i32), monitor_size: (u32, u32), window_width: u32, scale: f64) -> (i32, i32) {
    let x = monitor_pos.0 + (monitor_size.0 as i32 - window_width as i32) / 2;
    let y = monitor_pos.1 + (TOP_MARGIN * scale).round() as i32;
    (x, y)
}

/// Whether a point lies on any monitor `(x, y, width, height)`.
fn point_on_any(monitors: &[(i32, i32, u32, u32)], point: (i32, i32)) -> bool {
    monitors.iter().any(|&(mx, my, mw, mh)| {
        point.0 >= mx && point.1 >= my && point.0 < mx + mw as i32 && point.1 < my + mh as i32
    })
}

/// A monitor's work area — the screen minus the Windows taskbar, the macOS
/// menu bar (and the notch inside it) or a Linux desktop's top panel — as
/// `(x, y, width, height)` in physical px.
fn work_area(monitor: &tauri::Monitor) -> (i32, i32, u32, u32) {
    let area = monitor.work_area();
    (area.position.x, area.position.y, area.size.width, area.size.height)
}

/// Saved position if it is still inside a connected monitor's work area,
/// else top-centre of the primary monitor's work area (monitors can be
/// unplugged, and panels move, between sessions).
fn initial_position(window: &tauri::WebviewWindow, config: CapsuleConfig) -> Option<(i32, i32)> {
    let areas: Vec<(i32, i32, u32, u32)> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(work_area)
        .collect();
    if let (Some(x), Some(y)) = (config.x, config.y) {
        // Probe a point just inside the capsule, not its exact corner.
        if point_on_any(&areas, (x + 12, y + 12)) {
            return Some((x, y));
        }
    }
    let monitor = window.primary_monitor().ok().flatten()?;
    let width = window.outer_size().map(|s| s.width).unwrap_or(0);
    let (x, y, w, h) = work_area(&monitor);
    Some(centered_top((x, y), (w, h), width, monitor.scale_factor()))
}

/// Remember the position, writing it once the drag has settled.
fn schedule_save(app: &AppHandle, position: PhysicalPosition<i32>) {
    let Some(state) = app.try_state::<CapsuleState>() else { return };
    let config = match state.config.lock() {
        Ok(mut c) => {
            c.x = Some(position.x);
            c.y = Some(position.y);
            *c
        }
        Err(_) => return,
    };
    let generation = state.move_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SAVE_DEBOUNCE).await;
        let still_latest = app
            .try_state::<CapsuleState>()
            .is_some_and(|s| s.move_generation.load(Ordering::SeqCst) == generation);
        if still_latest {
            save(&app, config);
        }
    });
}

fn create_window(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(LABEL).is_some() {
        return Ok(());
    }
    // Same bundle as the main window; the frontend renders the capsule view
    // when its window label is `capsule`.
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("QuotaPulse")
        .inner_size(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .focused(false)
        .visible(false)
        // macOS needs the `macos-private-api` feature + `app.macOSPrivateApi`
        // (both enabled) for a transparent window; Linux needs a compositor.
        .transparent(true);
    let window = builder.build()?;

    let config = current(app).unwrap_or_default();
    if let Some((x, y)) = initial_position(&window, config) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Moved(position) = event {
            schedule_save(&handle, *position);
        }
    });
    window.show()?;
    Ok(())
}

/// Switch capsule mode on or off: capsule window, main window taskbar button
/// and tray badges all follow.
fn apply(app: &AppHandle, enabled: bool) {
    let main = app.get_webview_window("main");
    if enabled {
        if let Err(err) = create_window(app) {
            tracing::warn!("capsule window failed: {err}");
            // Without a capsule the main window must stay reachable.
            if let Some(main) = &main {
                let _ = main.set_skip_taskbar(false);
            }
            return;
        }
        if let Some(main) = &main {
            let _ = main.set_skip_taskbar(true);
        }
    } else {
        if let Some(window) = app.get_webview_window(LABEL) {
            let _ = window.close();
        }
        if let Some(main) = &main {
            let _ = main.set_skip_taskbar(false);
            let _ = main.show();
            let _ = main.unminimize();
            let _ = main.set_focus();
        }
    }
    // Per-provider badges only when the capsule is off.
    crate::post_refresh::rerender_tray_title(app);
}

/// Load the saved state and, if on, open the capsule. Returns whether the
/// capsule is on (the caller then keeps the main window hidden).
pub fn setup(app: &tauri::App) -> bool {
    let config = load(app.handle());
    app.manage(CapsuleState {
        config: Mutex::new(config),
        move_generation: AtomicU64::new(0),
    });
    if config.enabled {
        apply(app.handle(), true);
    }
    is_enabled(app.handle()) && app.get_webview_window(LABEL).is_some()
}

#[tauri::command]
pub fn get_capsule_enabled(app: AppHandle) -> bool {
    is_enabled(&app)
}

#[tauri::command]
pub fn set_capsule_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.try_state::<CapsuleState>().ok_or("capsule not ready")?;
    let config = {
        let mut c = state.config.lock().map_err(|e| e.to_string())?;
        c.enabled = enabled;
        *c
    };
    save(&app, config);
    apply(&app, enabled);
    Ok(())
}

/// Click on the capsule: show the main window, or hide it if it is on screen.
#[tauri::command]
pub fn capsule_toggle_main(app: AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let on_screen = window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false);
    if on_screen {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Fit the capsule to its content (logical px, clamped).
#[tauri::command]
pub fn capsule_resize(app: AppHandle, width: f64, height: f64) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    let w = width.clamp(WIDTH_RANGE.0, WIDTH_RANGE.1);
    let h = height.clamp(HEIGHT_RANGE.0, HEIGHT_RANGE.1);
    let _ = window.set_size(LogicalSize::new(w, h));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_fields_fall_back_to_defaults() {
        let c: CapsuleConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(c, CapsuleConfig { enabled: true, x: None, y: None });
        let c: CapsuleConfig = serde_json::from_str(r#"{"enabled":false}"#).unwrap();
        assert!(!c.enabled);
    }

    #[test]
    fn config_round_trips() {
        let c = CapsuleConfig { enabled: true, x: Some(-1200), y: Some(8) };
        let back: CapsuleConfig = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn default_position_is_top_centre() {
        // 1920 px monitor at 125 %: 300 px window centred, 10 px from the top.
        assert_eq!(centered_top((0, 0), (1920, 1080), 300, 1.25), (810, 10));
        // Secondary monitor to the left.
        assert_eq!(centered_top((-2560, 0), (2560, 1440), 200, 1.0), (-1380, 8));
    }

    #[test]
    fn default_position_respects_top_bars() {
        // macOS: a 1512×982 pt display at 2× whose menu bar (with the notch)
        // takes the top 37 pt → the work area starts at y = 74 px.
        assert_eq!(centered_top((0, 74), (3024, 1890), 360, 2.0), (1332, 90));
        // GNOME: a 32 px top panel on a 1920×1080 monitor.
        assert_eq!(centered_top((0, 32), (1920, 1048), 300, 1.0), (810, 40));
        // Windows: taskbar at the bottom, work area starts at the top.
        assert_eq!(centered_top((0, 0), (1920, 1040), 300, 1.0), (810, 8));
    }

    #[test]
    fn saved_position_under_a_top_panel_is_not_inside_the_work_area() {
        // Linux/macOS work area that starts below a 32 px panel.
        let areas = [(0, 32, 1920, 1048)];
        assert!(!point_on_any(&areas, (900, 10)), "inside the panel");
        assert!(point_on_any(&areas, (900, 40)));
    }

    #[test]
    fn saved_position_must_be_on_a_monitor() {
        let monitors = [(0, 0, 1920, 1080), (-2560, 0, 2560, 1440)];
        assert!(point_on_any(&monitors, (900, 20)));
        assert!(point_on_any(&monitors, (-100, 20)));
        assert!(!point_on_any(&monitors, (5000, 20)));
        assert!(!point_on_any(&monitors, (100, 1300)));
    }
}
