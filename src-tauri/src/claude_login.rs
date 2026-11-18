//! One-click Claude Code sign-in.
//!
//! Opens a visible terminal window running `claude auth login` (Claude
//! Code's dedicated sign-in command), so the user signs in through Claude
//! Code's own flow. QuotaPulse never sees or handles
//! the credentials: once Claude Code writes `~/.claude/.credentials.json`,
//! the file watcher refreshes the Claude card with live quota.

use std::path::PathBuf;
use std::process::Command;

/// Locate the `claude` executable: every `PATH` entry, then the usual
/// install locations of the native installer and npm.
fn find_claude() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let names: &[&str] = &["claude.exe", "claude.cmd", "claude.bat"];
    #[cfg(not(target_os = "windows"))]
    let names: &[&str] = &["claude"];

    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".claude").join("local"));
        dirs.push(home.join(".npm-global").join("bin"));
    }
    #[cfg(target_os = "windows")]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }

    dirs.iter()
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}

const NOT_FOUND: &str = "No se encontró el comando `claude`. Instala Claude Code \
     (https://docs.anthropic.com/claude-code) o añádelo al PATH y vuelve a intentarlo.";

/// Open a terminal window that runs `claude auth login`.
#[tauri::command]
pub fn trigger_claude_login() -> Result<(), String> {
    let claude = find_claude().ok_or_else(|| NOT_FOUND.to_string())?;
    tracing::info!("[claude] opening a terminal for `claude auth login` ({})", claude.display());
    spawn_terminal(&claude).map_err(|e| format!("No se pudo abrir la terminal: {e}"))
}

#[cfg(target_os = "windows")]
fn spawn_terminal(claude: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // `start` opens a new, visible console running `cmd /k`, which stays
    // open after the login so the user can read Claude Code's output. The
    // outer `cmd` gets no window of its own. The path comes from the file
    // system (no user input) and is quoted for paths with spaces.
    let path = claude.display().to_string();
    if path.contains('"') {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "ruta no válida"));
    }
    Command::new("cmd")
        .raw_arg(format!(r#"/c start "Claude Code - iniciar sesión" cmd /k ""{path}" auth login""#))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "macos")]
fn spawn_terminal(claude: &std::path::Path) -> std::io::Result<()> {
    let path = claude.display().to_string().replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"tell application "Terminal" to do script "\"{path}\" auth login"
tell application "Terminal" to activate"#
    );
    Command::new("osascript").arg("-e").arg(script).spawn().map(|_| ())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_terminal(claude: &std::path::Path) -> std::io::Result<()> {
    // First terminal emulator that is installed wins.
    let path = claude.display().to_string();
    let attempts: [(&str, &[&str]); 5] = [
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-x"]),
        ("xterm", &["-e"]),
    ];
    let mut last = std::io::Error::new(std::io::ErrorKind::NotFound, "no se encontró ninguna terminal");
    for (term, flag) in attempts {
        match Command::new(term).args(flag).arg(&path).args(["auth", "login"]).spawn() {
            Ok(_) => return Ok(()),
            Err(e) => last = e,
        }
    }
    Err(last)
}
