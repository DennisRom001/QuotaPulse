//! Starts the local LiteLLM Proxy (start-litellm.bat) when QuotaPulse opens.
//!
//! On launch QuotaPulse checks whether something listens on 127.0.0.1:4000.
//! If not, it runs `start-litellm.bat` hidden (CREATE_NO_WINDOW). The script
//! keeps LiteLLM in an isolated `.venv` next to itself (project root in
//! development, the install folder once installed), installs the pinned
//! `litellm[proxy]` there on first use with `.venv\Scripts\python.exe -m pip`,
//! and starts the proxy with that same interpreter. Global Python is never
//! modified. Nothing is shown on screen; progress goes to
//! `%LOCALAPPDATA%\QuotaPulse\litellm.log`.
//!
//! Opt out with `QP_DISABLE_LITELLM=1`. The proxy is left running when
//! QuotaPulse quits, so the next launch finds it already up.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tracing::{info, warn};

/// Development-only default master key, shared by start-litellm.bat and the
/// connector when LITELLM_MASTER_KEY is not set. Not a secret: the proxy only
/// listens on 127.0.0.1. Set LITELLM_MASTER_KEY for anything beyond local use.
pub const DEFAULT_MASTER_KEY: &str = "sk-quota-pulse-local";

const SCRIPT: &str = "start-litellm.bat";

pub fn port_open() -> bool {
    let addr: SocketAddr = ([127, 0, 0, 1], 4000).into();
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

fn script_path(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(dir) = app.path().resource_dir() {
        let bundled = dir.join(SCRIPT);
        if bundled.is_file() {
            return Some(bundled);
        }
    }
    // `tauri dev` / running from target/: use the copy in the project root.
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(SCRIPT);
    dev.is_file().then_some(dev)
}

/// Check port 4000 and start the proxy in the background if it is closed.
pub fn ensure_running(app: &AppHandle) {
    if crate::providers::env::lookup("QP_DISABLE_LITELLM").is_some_and(|(v, _)| v != "0") {
        info!("[litellm] auto-start disabled by QP_DISABLE_LITELLM");
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if port_open() {
            info!("[litellm] proxy already listening on 127.0.0.1:4000");
            return;
        }
        let Some(script) = script_path(&app) else {
            warn!("[litellm] {SCRIPT} not found; proxy not started");
            return;
        };
        match spawn_hidden(&script) {
            Ok(()) => info!("[litellm] port 4000 closed; started {}", script.display()),
            Err(e) => warn!("[litellm] could not start {}: {e}", script.display()),
        }
    });
}

#[cfg(target_os = "windows")]
fn spawn_hidden(script: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/D", "/C"])
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    // Hand the script the same key the connector will use: the user's
    // LITELLM_MASTER_KEY (also when set with `setx` after QuotaPulse started),
    // otherwise the script falls back to DEFAULT_MASTER_KEY.
    if let Some((key, _)) = crate::providers::env::lookup("LITELLM_MASTER_KEY") {
        cmd.env("LITELLM_MASTER_KEY", key);
    }
    cmd.spawn().map(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn spawn_hidden(_script: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::Error::other("start-litellm.bat is Windows-only"))
}
