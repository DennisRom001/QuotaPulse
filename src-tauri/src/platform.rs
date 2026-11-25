//! Per-OS knowledge in one place: where CLIs live, what their executables are
//! called, how a process name compares, and how to open a terminal that runs
//! an interactive login.
//!
//! Every function here is **pure** and takes the target OS as a value
//! ([`Os`]), so `cargo test` checks Windows, macOS and Linux behaviour on any
//! machine. The OS-specific call sites only pick `Os::current()` and do the
//! I/O (file checks, spawning).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    pub const fn current() -> Os {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

// ── Executables and install locations ────────────────────────────────────────

/// File names a CLI called `base` can have on `os` (npm installs `.cmd`
/// shims on Windows; Unix uses the bare name).
pub fn executable_names(os: Os, base: &str) -> Vec<String> {
    match os {
        Os::Windows => vec![format!("{base}.exe"), format!("{base}.cmd"), format!("{base}.bat")],
        Os::MacOs | Os::Linux => vec![base.to_string()],
    }
}

/// Install folders searched after `PATH`. Desktop apps on macOS and Linux get
/// a minimal `PATH` (no Homebrew, npm or `~/.local/bin`), so these are where
/// CLIs installed by npm, Homebrew, pipx, snap or the native installers live.
pub fn cli_search_dirs(os: Os, home: Option<&Path>, appdata: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home {
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".claude").join("local"));
        dirs.push(home.join(".npm-global").join("bin"));
    }
    match os {
        Os::Windows => {
            if let Some(appdata) = appdata {
                dirs.push(appdata.join("npm"));
            }
        }
        Os::MacOs => {
            dirs.push(PathBuf::from("/opt/homebrew/bin"));
            dirs.push(PathBuf::from("/usr/local/bin"));
            dirs.push(PathBuf::from("/usr/bin"));
        }
        Os::Linux => {
            dirs.push(PathBuf::from("/usr/local/bin"));
            dirs.push(PathBuf::from("/usr/bin"));
            dirs.push(PathBuf::from("/snap/bin"));
        }
    }
    dirs
}

/// Candidate paths for CLI `base`: every `PATH` entry first, then the
/// install folders, each with every executable name for the OS.
pub fn cli_candidates(os: Os, base: &str, path_dirs: &[PathBuf], home: Option<&Path>, appdata: Option<&Path>) -> Vec<PathBuf> {
    let names = executable_names(os, base);
    path_dirs
        .iter()
        .cloned()
        .chain(cli_search_dirs(os, home, appdata))
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)).collect::<Vec<_>>())
        .collect()
}

/// First existing candidate for CLI `base` on this machine.
pub fn find_cli(base: &str) -> Option<PathBuf> {
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    cli_candidates(Os::current(), base, &path_dirs, dirs::home_dir().as_deref(), appdata.as_deref())
        .into_iter()
        .find(|c| c.is_file())
}

#[cfg(any(not(target_os = "windows"), test))]
/// `PATH` entries to prepend for a login shell started from a desktop app on
/// macOS / Linux, so `#!/usr/bin/env node` shims find `node`.
pub fn extended_path_entries(os: Os) -> &'static [&'static str] {
    match os {
        Os::Windows => &[],
        Os::MacOs => &["$HOME/.local/bin", "$HOME/.npm-global/bin", "/opt/homebrew/bin", "/usr/local/bin"],
        Os::Linux => &["$HOME/.local/bin", "$HOME/.npm-global/bin", "/usr/local/bin"],
    }
}

// ── Process names ────────────────────────────────────────────────────────────

/// Comparable process name: base name only, without `.exe` / `.app`,
/// lowercase. `Cursor.exe`, `/Applications/Cursor.app/Contents/MacOS/Cursor`
/// and `cursor` all become `cursor`.
pub fn normalize_process_name(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches(['/', '\\']);
    let base = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
    let lower = base.to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .or_else(|| lower.strip_suffix(".app"))
        .unwrap_or(&lower)
        .to_string()
}

/// Whether any of `names` is the process `wanted` (both normalised).
pub fn process_list_contains<'a>(names: impl IntoIterator<Item = &'a str>, wanted: &str) -> bool {
    let wanted = normalize_process_name(wanted);
    !wanted.is_empty() && names.into_iter().any(|n| normalize_process_name(n) == wanted)
}

#[cfg(any(target_os = "macos", test))]
/// Process names from `ps -A -o comm=` output (macOS prints full paths).
pub fn parse_ps_output(out: &str) -> Vec<String> {
    out.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(any(target_os = "linux", test))]
/// Process name from a Linux `/proc/<pid>/cmdline` (NUL-separated argv):
/// the base name of argv[0]. `comm` is cut to 15 bytes, so both are checked.
pub fn proc_cmdline_name(cmdline: &[u8]) -> Option<String> {
    let argv0 = cmdline.split(|&b| b == 0).next()?;
    let s = String::from_utf8_lossy(argv0);
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

// ── Interactive login in a terminal ──────────────────────────────────────────

#[cfg(any(not(target_os = "windows"), test))]
/// Quote for POSIX `sh`: `'…'`, with embedded `'` as `'\''`.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(any(not(target_os = "windows"), test))]
/// The `sh` script run inside the terminal on macOS / Linux: extend `PATH`,
/// explain, run the login, then close on success or wait on failure.
pub fn unix_login_script(os: Os, program: &Path, args: &[&str], intro: &str) -> String {
    let path_prefix = extended_path_entries(os).join(":");
    let cmd = std::iter::once(sh_quote(&program.display().to_string()))
        .chain(args.iter().map(|a| sh_quote(a)))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "export PATH=\"{path_prefix}:$PATH\"; printf '%s\\n\\n' {intro}; \
         if {cmd}; then printf '\\n%s\\n' {done}; sleep 3; \
         else printf '\\n%s\\n' {failed}; printf '%s' {press}; read -r _; fi",
        intro = sh_quote(intro),
        done = sh_quote("Sesion iniciada. Ya puedes volver a QuotaPulse; esta ventana se cerrara sola."),
        failed = sh_quote("No se pudo iniciar sesion. Revisa el mensaje de arriba."),
        press = sh_quote("Pulsa Enter para cerrar..."),
    )
}

#[cfg(any(target_os = "macos", test))]
/// Escape for an AppleScript string literal.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(any(target_os = "macos", test))]
/// AppleScript that opens Terminal.app, runs `script` and brings it forward.
/// `exit` lets Terminal close the window once the script ends (default
/// profile: close if the shell exited cleanly).
pub fn macos_osascript(script: &str) -> String {
    format!(
        "tell application \"Terminal\"\n  do script {}\n  activate\nend tell",
        applescript_string(&format!("{script}; exit"))
    )
}

#[cfg(any(target_os = "linux", test))]
/// Linux terminal emulators, in order of preference.
pub const LINUX_TERMINALS: &[&str] = &[
    "x-terminal-emulator",
    "gnome-terminal",
    "konsole",
    "xfce4-terminal",
    "alacritty",
    "kitty",
    "xterm",
];

#[cfg(any(target_os = "linux", test))]
/// Arguments that make `terminal` run `sh -c script` (each emulator spells
/// "execute this" differently).
pub fn linux_terminal_args(terminal: &str, script: &str) -> Vec<String> {
    let run: Vec<String> = vec!["sh".into(), "-c".into(), script.into()];
    let mut args: Vec<String> = match terminal {
        "gnome-terminal" => vec!["--".into()],
        "xfce4-terminal" => vec!["-x".into()],
        "kitty" => Vec::new(),
        // x-terminal-emulator, konsole, alacritty, xterm
        _ => vec!["-e".into()],
    };
    args.extend(run);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Os; 3] = [Os::Windows, Os::MacOs, Os::Linux];

    #[test]
    fn executable_names_per_os() {
        assert_eq!(executable_names(Os::Windows, "claude"), ["claude.exe", "claude.cmd", "claude.bat"]);
        assert_eq!(executable_names(Os::MacOs, "codex"), ["codex"]);
        assert_eq!(executable_names(Os::Linux, "codex"), ["codex"]);
    }

    #[test]
    fn search_dirs_cover_each_platforms_install_locations() {
        let home = Path::new("/home/ana");
        let win = cli_search_dirs(Os::Windows, Some(Path::new("C:/Users/Ana")), Some(Path::new("C:/Users/Ana/AppData/Roaming")));
        assert!(win.contains(&PathBuf::from("C:/Users/Ana/AppData/Roaming/npm")));
        assert!(!win.iter().any(|d| d.starts_with("/usr")));

        let mac = cli_search_dirs(Os::MacOs, Some(Path::new("/Users/ana")), None);
        for d in ["/opt/homebrew/bin", "/usr/local/bin", "/Users/ana/.local/bin", "/Users/ana/.npm-global/bin"] {
            assert!(mac.contains(&PathBuf::from(d)), "macOS misses {d}");
        }

        let linux = cli_search_dirs(Os::Linux, Some(home), None);
        for d in ["/home/ana/.local/bin", "/home/ana/.npm-global/bin", "/usr/local/bin", "/usr/bin", "/snap/bin"] {
            assert!(linux.contains(&PathBuf::from(d)), "Linux misses {d}");
        }
        assert!(!linux.iter().any(|d| d.ends_with("npm") && d.to_string_lossy().contains("AppData")));
    }

    #[test]
    fn path_entries_come_before_install_dirs() {
        let path = vec![PathBuf::from("/custom/bin")];
        let c = cli_candidates(Os::Linux, "claude", &path, Some(Path::new("/home/ana")), None);
        assert_eq!(c[0], PathBuf::from("/custom/bin/claude"));
        assert!(c.contains(&PathBuf::from("/home/ana/.local/bin/claude")));
        let w = cli_candidates(Os::Windows, "codex", &[], None, Some(Path::new("C:/AppData")));
        assert!(w.contains(&PathBuf::from("C:/AppData/npm/codex.cmd")));
    }

    #[test]
    fn process_names_normalise_across_platforms() {
        assert_eq!(normalize_process_name("Cursor.exe"), "cursor");
        assert_eq!(normalize_process_name("/Applications/Cursor.app/Contents/MacOS/Cursor"), "cursor");
        assert_eq!(normalize_process_name("/opt/cursor/cursor"), "cursor");
        assert_eq!(normalize_process_name(r"C:\Program Files\Cursor\Cursor.exe"), "cursor");
        assert_eq!(normalize_process_name("Cursor.app"), "cursor");
        assert_eq!(normalize_process_name("  claude \n"), "claude");
    }

    #[test]
    fn ps_output_finds_apps_by_logical_name() {
        // macOS `ps -A -o comm=` prints full paths; Linux prints short names.
        let mac = "/sbin/launchd\n/Applications/Cursor.app/Contents/MacOS/Cursor\n/usr/local/bin/node\n";
        let names = parse_ps_output(mac);
        assert!(process_list_contains(names.iter().map(String::as_str), "Cursor.exe"));
        assert!(!process_list_contains(names.iter().map(String::as_str), "Gemini"));
        let linux = "systemd\nkworker/0:1\ncursor\n";
        assert!(process_list_contains(parse_ps_output(linux).iter().map(String::as_str), "Cursor"));
        assert!(!process_list_contains(["cursorless-helper"], "cursor"));
        assert!(!process_list_contains(["anything"], ""));
    }

    #[test]
    fn linux_cmdline_takes_argv0() {
        assert_eq!(proc_cmdline_name(b"/opt/Cursor/cursor\0--no-sandbox\0").as_deref(), Some("/opt/Cursor/cursor"));
        assert_eq!(proc_cmdline_name(b""), None);
        assert!(process_list_contains(["/tmp/.mount_Cursor/cursor"], "Cursor.exe"));
    }

    #[test]
    fn sh_quote_survives_spaces_and_quotes() {
        assert_eq!(sh_quote("/Users/A B/bin/claude"), "'/Users/A B/bin/claude'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn unix_login_script_extends_path_and_closes_or_waits() {
        for os in [Os::MacOs, Os::Linux] {
            let s = unix_login_script(os, Path::new("/home/a b/.local/bin/claude"), &["auth", "login"], "Hola");
            assert!(s.starts_with("export PATH=\"$HOME/.local/bin:$HOME/.npm-global/bin:"));
            assert!(s.contains(":$PATH\";"));
            assert!(s.contains("if '/home/a b/.local/bin/claude' 'auth' 'login'; then"));
            assert!(s.contains("sleep 3") && s.contains("read -r _"));
        }
        let mac = unix_login_script(Os::MacOs, Path::new("/opt/homebrew/bin/codex"), &["login"], "x");
        assert!(mac.contains("/opt/homebrew/bin") && mac.contains("'/opt/homebrew/bin/codex' 'login'"));
        let linux = unix_login_script(Os::Linux, Path::new("/usr/bin/codex"), &["login"], "x");
        assert!(!linux.contains("/opt/homebrew"));
    }

    #[test]
    fn macos_applescript_escapes_the_script() {
        let script = unix_login_script(Os::MacOs, Path::new("/Users/ana/.npm-global/bin/claude"), &["auth", "login"], "Hola \"x\"");
        let osa = macos_osascript(&script);
        assert!(osa.starts_with("tell application \"Terminal\"\n  do script \""));
        assert!(osa.contains("activate") && osa.ends_with("end tell"));
        assert!(osa.contains(r#"export PATH=\"$HOME"#), "inner quotes must be escaped");
        assert!(osa.contains("; exit\""));
        // Every double quote inside the literal is escaped: unescaped ones are
        // only the delimiters of the two string literals.
        let unescaped = osa.match_indices('"').filter(|(i, _)| *i == 0 || &osa[i - 1..*i] != "\\").count();
        assert_eq!(unescaped, 4);
    }

    #[test]
    fn linux_terminals_get_the_right_exec_flag() {
        let s = "echo hi";
        assert_eq!(linux_terminal_args("gnome-terminal", s), ["--", "sh", "-c", s]);
        assert_eq!(linux_terminal_args("xfce4-terminal", s), ["-x", "sh", "-c", s]);
        assert_eq!(linux_terminal_args("kitty", s), ["sh", "-c", s]);
        for t in ["x-terminal-emulator", "konsole", "alacritty", "xterm"] {
            assert_eq!(linux_terminal_args(t, s), ["-e", "sh", "-c", s], "{t}");
        }
        assert_eq!(LINUX_TERMINALS.len(), 7);
    }

    #[test]
    fn windows_needs_no_path_prefix() {
        assert!(extended_path_entries(Os::Windows).is_empty());
        for os in ALL {
            let _ = extended_path_entries(os);
        }
    }
}
