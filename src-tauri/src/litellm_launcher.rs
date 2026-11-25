//! Starts the local LiteLLM Proxy (start-litellm.bat) when QuotaPulse opens.
//!
//! On launch QuotaPulse checks whether something listens on 127.0.0.1:4000.
//! If not, it runs `start-litellm.bat` hidden (CREATE_NO_WINDOW). The script
//! keeps LiteLLM in an isolated `.venv` next to itself (project root in
//! development, the install folder once installed), installs the
//! hash-pinned dependency set from `litellm-requirements.txt` there on first
//! use, and starts the proxy with that same interpreter. Global Python is
//! never modified. Progress goes to `%LOCALAPPDATA%\QuotaPulse\litellm.log`.
//!
//! Security model:
//! * The proxy is protected by a random per-install master key (32 bytes from
//!   the OS RNG), kept in the Windows Credential Manager (fallback: a file in
//!   the user's local app-data folder). A user-set LITELLM_MASTER_KEY wins.
//! * The proxy starts with a minimal environment: provider API keys and other
//!   secrets in the user's environment never reach it.
//! * Only the bundled script next to the executable is ever run.
//!
//! Opt out with `QP_DISABLE_LITELLM=1`. The proxy is left running when
//! QuotaPulse quits, so the next launch finds it already up.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tracing::{info, warn};

const SCRIPT: &str = "start-litellm.bat";
const KEYRING_SERVICE: &str = "QuotaPulse";
const KEYRING_ACCOUNT: &str = "litellm-master-key";
const KEY_FILE: &str = "litellm-master.key";

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
    dev_script_path()
}

/// `tauri dev` / `cargo run`: the copy in the project root. Never compiled
/// into release builds, so an installed app only runs its bundled script and
/// never a path from the build machine.
#[cfg(debug_assertions)]
fn dev_script_path() -> Option<PathBuf> {
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(SCRIPT);
    dev.is_file().then_some(dev)
}

#[cfg(not(debug_assertions))]
fn dev_script_path() -> Option<PathBuf> {
    None
}

// ── Per-install master key ──────────────────────────────────────────────────

fn key_file_path() -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join("QuotaPulse").join(KEY_FILE))
}

fn valid_key(key: &str) -> bool {
    key.starts_with("sk-qp-") && key.len() >= 6 + 64 && key[6..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// A new random key: `sk-qp-` + 64 hex chars (32 bytes from the OS RNG).
fn generate_key() -> Option<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(format!("sk-qp-{}", bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()))
}

fn read_stored_key() -> Option<String> {
    let from_keyring = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .ok()
        .and_then(|e| e.get_password().ok());
    from_keyring
        .or_else(|| std::fs::read_to_string(key_file_path()?).ok())
        .map(|k| k.trim().to_string())
        .filter(|k| valid_key(k))
}

fn store_key(key: &str) -> bool {
    if keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .and_then(|e| e.set_password(key))
        .is_ok()
    {
        return true;
    }
    // Fallback: the user's local app-data folder (per-user ACL on Windows).
    let Some(path) = key_file_path() else { return false };
    path.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&path, key).is_ok()
}

/// The key the proxy and the connector share, read-only: the user's own
/// LITELLM_MASTER_KEY, else the stored per-install key. Never a built-in
/// default.
pub fn master_key() -> Option<String> {
    crate::providers::env::lookup("LITELLM_MASTER_KEY")
        .map(|(v, _)| v)
        .filter(|v| !v.trim().is_empty())
        .or_else(read_stored_key)
}

/// Like [`master_key`], but creates and stores a per-install key on first use.
fn master_key_or_create() -> Option<String> {
    if let Some(key) = master_key() {
        return Some(key);
    }
    let key = generate_key()?;
    if store_key(&key) {
        info!("[litellm] generated a per-install master key");
        Some(key)
    } else {
        warn!("[litellm] could not store a master key; proxy not started");
        None
    }
}

// ── Launch ──────────────────────────────────────────────────────────────────

/// Check port 4000 and start the proxy in the background if it is closed.
pub fn ensure_running(app: &AppHandle) {
    if crate::providers::env::lookup("QP_DISABLE_LITELLM").is_some_and(|(v, _)| v != "0") {
        info!("[litellm] auto-start disabled by QP_DISABLE_LITELLM");
        return;
    }
    // start-litellm.bat is a Windows script. On macOS / Linux QuotaPulse still
    // reads a proxy the user runs on 127.0.0.1:4000; it just does not start it
    // (and does not create a master key for it).
    if !cfg!(target_os = "windows") {
        info!("[litellm] auto-start is Windows-only; start `litellm` yourself to use the proxy");
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if port_open() {
            info!("[litellm] proxy already listening on 127.0.0.1:4000");
            return;
        }
        let Some(script) = script_path(&app) else {
            warn!("[litellm] {SCRIPT} not found next to QuotaPulse; proxy not started");
            return;
        };
        let Some(key) = master_key_or_create() else { return };
        match spawn_hidden(&script, &key) {
            Ok(()) => info!("[litellm] port 4000 closed; started {}", script.display()),
            Err(e) => warn!("[litellm] could not start {}: {e}", script.display()),
        }
    });
}

/// Environment variables the proxy may inherit. Everything else — notably
/// provider API keys such as GEMINI_API_KEY / OPENAI_API_KEY — is withheld,
/// so the proxy cannot spend them on behalf of whoever calls it.
#[cfg(any(target_os = "windows", test))]
const INHERITED_ENV: &[&str] = &[
    "SystemRoot", "SystemDrive", "windir", "ComSpec", "PATH", "PATHEXT", "TEMP", "TMP",
    "USERPROFILE", "USERNAME", "HOMEDRIVE", "HOMEPATH", "LOCALAPPDATA", "APPDATA",
    "PROGRAMDATA", "ProgramFiles", "ProgramFiles(x86)", "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE", "OS",
];

/// LiteLLM tuning variables (LITELLM_*) pass through, except those carrying a
/// credential; the master key is set explicitly.
#[cfg(any(target_os = "windows", test))]
fn inherit_var(name: &str) -> bool {
    INHERITED_ENV.iter().any(|v| v.eq_ignore_ascii_case(name))
        || (name.to_ascii_uppercase().starts_with("LITELLM_")
            && !matches!(name.to_ascii_uppercase().as_str(), "LITELLM_MASTER_KEY" | "LITELLM_API_KEY" | "LITELLM_SALT_KEY"))
}

#[cfg(target_os = "windows")]
fn spawn_hidden(script: &std::path::Path, key: &str) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    // Absolute cmd.exe so a copy beside the executable is never preferred.
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let cmd_exe = PathBuf::from(system_root).join("System32").join("cmd.exe");
    let mut cmd = Command::new(cmd_exe);
    cmd.env_clear()
        .envs(std::env::vars_os().filter(|(k, _)| k.to_str().is_some_and(inherit_var)))
        .env("LITELLM_MASTER_KEY", key)
        .args(["/D", "/C"])
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn spawn_hidden(_script: &std::path::Path, _key: &str) -> std::io::Result<()> {
    Err(std::io::Error::other("start-litellm.bat is Windows-only"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_keys_are_random_and_well_formed() {
        let a = generate_key().unwrap();
        let b = generate_key().unwrap();
        assert!(valid_key(&a) && valid_key(&b));
        assert_ne!(a, b);
        assert_eq!(a.len(), 6 + 64);
        assert!(!valid_key("sk-quota-pulse-local"));
    }

    #[test]
    fn provider_keys_are_not_inherited_by_the_proxy() {
        for name in ["GEMINI_API_KEY", "GOOGLE_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "LITELLM_MASTER_KEY", "LITELLM_API_KEY"] {
            assert!(!inherit_var(name), "{name} must not reach the proxy");
        }
        for name in ["PATH", "SystemRoot", "LOCALAPPDATA", "LITELLM_LOG", "litellm_telemetry"] {
            assert!(inherit_var(name), "{name} should pass through");
        }
    }
}
