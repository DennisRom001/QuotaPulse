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
        .map(|found| native_behind_shim(&found).unwrap_or(found))
}

/// npm installs `claude.cmd`, a batch shim that only starts the native
/// `claude.exe` shipped in the package. Running the exe directly drops one
/// batch layer between the console and the sign-in prompt.
fn native_behind_shim(found: &std::path::Path) -> Option<PathBuf> {
    let is_shim = found
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if !is_shim {
        return None;
    }
    let native = shim_native_path(found.parent()?);
    native.is_file().then_some(native)
}

fn shim_native_path(npm_dir: &std::path::Path) -> PathBuf {
    let exe = if cfg!(target_os = "windows") { "claude.exe" } else { "claude" };
    npm_dir
        .join("node_modules")
        .join("@anthropic-ai")
        .join("claude-code")
        .join("bin")
        .join(exe)
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
#[link(name = "user32")]
extern "system" {
    fn AllowSetForegroundWindow(dw_process_id: u32) -> i32;
}

#[cfg(target_os = "windows")]
fn spawn_terminal(claude: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// Any process may take the foreground (until the next user input).
    const ASFW_ANY: u32 = u32::MAX;
    // `start` opens a new, visible console that owns real keyboard input
    // (verified: stdin/stdout are the console, typing and Ctrl+V arrive). It
    // explains what is about to happen, runs `claude auth login`, and then:
    //   * success → a short confirmation, and the window closes by itself;
    //   * failure → the error stays on screen until a key is pressed.
    // The outer `cmd` gets no window of its own. The path comes from the file
    // system (no user input) and is quoted for paths with spaces.
    let path = claude.display().to_string();
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
        .raw_arg(windows_command_line(&path))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

/// Command line for the outer `cmd` (see `spawn_terminal`). Messages avoid
/// accents so they render in any console code page.
#[cfg(any(target_os = "windows", test))]
fn windows_command_line(path: &str) -> String {
    const INTRO: &str = "Se abrira el navegador para iniciar sesion en Claude. \
        Si la pagina te muestra un codigo, vuelve a esta ventana, haz clic dentro, \
        pegalo con Ctrl+V o clic derecho y presiona Enter.";
    const DONE: &str = "Sesion iniciada. Ya puedes volver a QuotaPulse; esta ventana se cerrara sola.";
    const FAILED: &str = "No se pudo iniciar sesion. Revisa el mensaje de arriba.";
    format!(
        r#"/c start "Claude Code - iniciar sesion" cmd /c "echo {INTRO} & echo. & "{path}" auth login && (echo. & echo {DONE} & timeout /t 3 >nul) || (echo. & echo {FAILED} & pause)""#
    )
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

#[cfg(test)]
mod tests {
    #[test]
    fn windows_command_closes_on_success_and_pauses_on_error() {
        let line = super::windows_command_line("C:/Users/A B/AppData/Roaming/npm/claude.cmd");
        assert!(line.starts_with(r#"/c start "Claude Code - iniciar sesion" cmd /c ""#));
        assert!(line.contains(r#""C:/Users/A B/AppData/Roaming/npm/claude.cmd" auth login &&"#));
        assert!(line.contains("timeout /t 3") && line.contains("|| (echo. & echo No se pudo"));
        assert!(!line.contains("cmd /k"));
        // Balanced quotes, so cmd strips only the outer pair of the inner command.
        assert_eq!(line.matches('"').count() % 2, 0);
        // The intro tells the user how to paste, and holds no cmd metacharacters.
        assert!(line.contains("Ctrl+V"));
        let intro = line.split(" & echo.").next().unwrap();
        assert!(!intro.contains(['|', '<', '>', '^', '%']), "unsafe intro: {intro}");
    }

    #[test]
    fn npm_shim_resolves_to_the_native_executable() {
        let dir = std::env::temp_dir().join(format!("qp-shim-{}", std::process::id()));
        let shim = dir.join("claude.cmd");
        let native = super::shim_native_path(&dir);
        std::fs::create_dir_all(native.parent().unwrap()).unwrap();
        std::fs::write(&shim, "@echo off").unwrap();
        // Shim without the native binary next to it: keep the shim.
        assert_eq!(super::native_behind_shim(&shim), None);
        std::fs::write(&native, "").unwrap();
        assert_eq!(super::native_behind_shim(&shim), Some(native.clone()));
        // A native binary found on PATH is used as is.
        assert_eq!(super::native_behind_shim(&native), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
