//! One-click sign-in for the CLIs QuotaPulse reads (Claude Code, Codex CLI).
//!
//! Opens a visible terminal running the CLI's own login command
//! (`claude auth login`, `codex login`), so the user signs in through the
//! tool's own flow. QuotaPulse never sees or handles the credentials: once the
//! CLI writes its credentials file, the file watcher refreshes the card.
//!
//! * Windows: a new console via `cmd /c start`, given foreground rights.
//! * macOS: Terminal.app through `osascript`.
//! * Linux: the first terminal emulator found (see
//!   [`crate::platform::LINUX_TERMINALS`]).
//!
//! On macOS / Linux the script extends `PATH` (Homebrew, npm, ~/.local/bin):
//! desktop apps start with a minimal `PATH`, and npm shims need `node`.
//! The command lines are built by pure functions in [`crate::platform`] and
//! here, all unit-tested for every OS.

use std::path::{Path, PathBuf};
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use std::process::Command;

use crate::platform;

/// A CLI login QuotaPulse can open.
#[derive(Debug, Clone, Copy)]
pub struct LoginTarget {
    /// Executable base name (`claude`, `codex`).
    pub cli: &'static str,
    pub args: &'static [&'static str],
    /// Window title (ASCII: it is passed through `cmd`). Windows only.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub title: &'static str,
    /// What the user is about to see (ASCII, no cmd metacharacters).
    pub intro: &'static str,
    pub not_found: &'static str,
}

pub const CLAUDE: LoginTarget = LoginTarget {
    cli: "claude",
    args: &["auth", "login"],
    title: "Claude Code - iniciar sesion",
    intro: "Se abrira el navegador para iniciar sesion en Claude. \
        Si la pagina te muestra un codigo, vuelve a esta ventana, haz clic dentro, \
        pegalo con Ctrl+V o clic derecho y presiona Enter.",
    not_found: "No se encontró el comando `claude`. Instala Claude Code \
        (https://docs.anthropic.com/claude-code) o añádelo al PATH y vuelve a intentarlo.",
};

pub const CODEX: LoginTarget = LoginTarget {
    cli: "codex",
    args: &["login"],
    title: "Codex CLI - iniciar sesion",
    intro: "Se abrira el navegador para iniciar sesion en ChatGPT con Codex CLI. \
        Completa el inicio de sesion alli; esta ventana se cerrara sola al terminar.",
    not_found: "No se encontró el comando `codex`. Instálalo con `npm i -g @openai/codex` \
        y vuelve a intentarlo.",
};

/// Locate a CLI: `PATH`, then the usual install folders for this OS; an npm
/// shim on Windows resolves to the native binary behind it when present.
fn find(target: &LoginTarget) -> Option<PathBuf> {
    platform::find_cli(target.cli).map(|found| native_behind_shim(&found, target.cli).unwrap_or(found))
}

/// npm installs `claude.cmd`, a batch shim that only starts the native
/// `claude.exe` shipped in the package. Running the exe directly drops one
/// batch layer between the console and the sign-in prompt.
fn native_behind_shim(found: &Path, cli: &str) -> Option<PathBuf> {
    let is_shim = found
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if !is_shim || cli != "claude" {
        return None;
    }
    let native = shim_native_path(found.parent()?);
    native.is_file().then_some(native)
}

fn shim_native_path(npm_dir: &Path) -> PathBuf {
    let exe = if cfg!(target_os = "windows") { "claude.exe" } else { "claude" };
    npm_dir
        .join("node_modules")
        .join("@anthropic-ai")
        .join("claude-code")
        .join("bin")
        .join(exe)
}

fn open_login(target: &LoginTarget) -> Result<(), String> {
    let program = find(target).ok_or_else(|| target.not_found.to_string())?;
    tracing::info!("[login] opening a terminal for `{} {}` ({})", target.cli, target.args.join(" "), program.display());
    spawn_terminal(target, &program).map_err(|e| format!("No se pudo abrir la terminal: {e}"))
}

/// Open a terminal window that runs `claude auth login`.
#[tauri::command]
pub fn trigger_claude_login() -> Result<(), String> {
    open_login(&CLAUDE)
}

/// Open a terminal window that runs `codex login`.
#[tauri::command]
pub fn trigger_codex_login() -> Result<(), String> {
    open_login(&CODEX)
}

// ── Windows ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn AllowSetForegroundWindow(dw_process_id: u32) -> i32;
}

#[cfg(target_os = "windows")]
fn spawn_terminal(target: &LoginTarget, program: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// Any process may take the foreground (until the next user input).
    const ASFW_ANY: u32 = u32::MAX;
    // `start` opens a new, visible console that owns real keyboard input
    // (verified: stdin/stdout are the console, typing and Ctrl+V arrive). It
    // explains what is about to happen, runs the login, and then:
    //   * success → a short confirmation, and the window closes by itself;
    //   * failure → the error stays on screen until a key is pressed.
    // The outer `cmd` gets no window of its own. The path comes from the file
    // system (no user input) and is quoted for paths with spaces.
    let path = program.display().to_string();
    if path.contains('"') {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "ruta no válida"));
    }
    // The console is created by the hidden background `cmd`, and Windows does
    // not let a background process take the foreground: the window used to
    // open without keyboard focus, so typing and pasting went elsewhere.
    // QuotaPulse is the foreground app (the user just clicked its button), so
    // it may hand that right on before spawning.
    // SAFETY: plain Win32 call with a constant argument; no pointers involved.
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
    }
    Command::new("cmd")
        .raw_arg(windows_command_line(target, &path))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

#[cfg(any(target_os = "windows", test))]
/// Command line for the outer `cmd` (see the Windows `spawn_terminal`).
/// Messages avoid accents so they render in any console code page.
fn windows_command_line(target: &LoginTarget, path: &str) -> String {
    const DONE: &str = "Sesion iniciada. Ya puedes volver a QuotaPulse; esta ventana se cerrara sola.";
    const FAILED: &str = "No se pudo iniciar sesion. Revisa el mensaje de arriba.";
    let (title, intro, args) = (target.title, target.intro, target.args.join(" "));
    format!(
        r#"/c start "{title}" cmd /c "echo {intro} & echo. & "{path}" {args} && (echo. & echo {DONE} & timeout /t 3 >nul) || (echo. & echo {FAILED} & pause)""#
    )
}

// ── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn spawn_terminal(target: &LoginTarget, program: &Path) -> std::io::Result<()> {
    let script = platform::unix_login_script(platform::Os::MacOs, program, target.args, target.intro);
    Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(platform::macos_osascript(&script))
        .spawn()
        .map(|_| ())
}

// ── Linux ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn spawn_terminal(target: &LoginTarget, program: &Path) -> std::io::Result<()> {
    let script = platform::unix_login_script(platform::Os::Linux, program, target.args, target.intro);
    let mut last = std::io::Error::new(std::io::ErrorKind::NotFound, "no se encontró ninguna terminal");
    for terminal in platform::LINUX_TERMINALS {
        // Only try emulators that exist, so a missing one is not "spawned".
        let Some(exe) = platform::find_cli(terminal) else { continue };
        match Command::new(exe).args(platform::linux_terminal_args(terminal, &script)).spawn() {
            Ok(_) => return Ok(()),
            Err(e) => last = e,
        }
    }
    Err(last)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn spawn_terminal(_target: &LoginTarget, _program: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("plataforma no soportada"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_command_closes_on_success_and_pauses_on_error() {
        let line = windows_command_line(&CLAUDE, "C:/Users/A B/AppData/Roaming/npm/claude.cmd");
        assert!(line.starts_with(r#"/c start "Claude Code - iniciar sesion" cmd /c ""#));
        assert!(line.contains(r#""C:/Users/A B/AppData/Roaming/npm/claude.cmd" auth login &&"#));
        assert!(line.contains("timeout /t 3") && line.contains("|| (echo. & echo No se pudo"));
        assert!(!line.contains("cmd /k"));
        // Balanced quotes, so cmd strips only the outer pair of the inner command.
        assert_eq!(line.matches('"').count() % 2, 0);
        // The intro tells the user how to paste, and holds no cmd metacharacters.
        assert!(line.contains("Ctrl+V"));
        for target in [CLAUDE, CODEX] {
            assert!(!target.intro.contains(['|', '<', '>', '^', '%', '&', '"']), "unsafe intro: {}", target.intro);
            assert!(target.title.is_ascii() && target.intro.is_ascii());
        }
    }

    #[test]
    fn codex_login_runs_codex_login_on_every_os() {
        let win = windows_command_line(&CODEX, "C:/Users/A/AppData/Roaming/npm/codex.cmd");
        assert!(win.starts_with(r#"/c start "Codex CLI - iniciar sesion""#));
        assert!(win.contains(r#""C:/Users/A/AppData/Roaming/npm/codex.cmd" login &&"#));
        let mac = platform::unix_login_script(platform::Os::MacOs, Path::new("/opt/homebrew/bin/codex"), CODEX.args, CODEX.intro);
        assert!(mac.contains("'/opt/homebrew/bin/codex' 'login'"));
        let linux = platform::unix_login_script(platform::Os::Linux, Path::new("/home/a/.npm-global/bin/claude"), CLAUDE.args, CLAUDE.intro);
        assert!(linux.contains("'/home/a/.npm-global/bin/claude' 'auth' 'login'"));
        let osa = platform::macos_osascript(&mac);
        assert!(osa.contains("tell application \"Terminal\""));
    }

    #[test]
    fn npm_shim_resolves_to_the_native_executable() {
        let dir = std::env::temp_dir().join(format!("qp-shim-{}", std::process::id()));
        let shim = dir.join("claude.cmd");
        let native = shim_native_path(&dir);
        std::fs::create_dir_all(native.parent().unwrap()).unwrap();
        std::fs::write(&shim, "@echo off").unwrap();
        // Shim without the native binary next to it: keep the shim.
        assert_eq!(native_behind_shim(&shim, "claude"), None);
        std::fs::write(&native, "").unwrap();
        assert_eq!(native_behind_shim(&shim, "claude"), Some(native.clone()));
        // A native binary found on PATH is used as is; other CLIs are untouched.
        assert_eq!(native_behind_shim(&native, "claude"), None);
        assert_eq!(native_behind_shim(&dir.join("codex.cmd"), "codex"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
