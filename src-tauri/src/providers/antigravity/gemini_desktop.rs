//! Detection of Google's Gemini desktop app for Windows.
//!
//! The app is an Electron shell around gemini.google.com:
//! * program + logs: `%LOCALAPPDATA%\Google\Gemini` (`logs\gemini.log`)
//! * browser profile: `%APPDATA%\Gemini` (cookies, Local Storage…)
//!
//! Its log only records app lifecycle events (start, version, updates) and it
//! keeps no quota or token counters on disk; usage lives in the web session.
//! So this module reports what can honestly be known locally — installed,
//! running, version, last activity — and deliberately never reads the
//! browser profile (session cookies are not ours to use).

use std::path::PathBuf;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct GeminiDesktop {
    pub version: Option<String>,
    pub running: bool,
    pub last_activity: Option<DateTime<Utc>>,
}

fn install_dirs() -> Vec<PathBuf> {
    let mut dirs_out = Vec::new();
    if let Some(local) = dirs::data_local_dir() {
        dirs_out.push(local.join("Google").join("Gemini"));
    }
    if let Some(roaming) = dirs::data_dir() {
        dirs_out.push(roaming.join("Google").join("Gemini"));
    }
    dirs_out
}

fn profile_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Gemini"))
}

/// Electron keeps `lockfile` in the profile open with exclusive sharing while
/// the app runs, so failing to open it means the app is running.
#[cfg(target_os = "windows")]
fn profile_locked(profile: &std::path::Path) -> bool {
    let lock = profile.join("lockfile");
    lock.is_file()
        && std::fs::OpenOptions::new()
            .write(true)
            .open(&lock)
            .is_err()
}

#[cfg(not(target_os = "windows"))]
fn profile_locked(_profile: &std::path::Path) -> bool {
    false
}

/// Parse `[2026-11-04T18:47:25.855Z] [INFO] [main] Starting Gemini v1.17.0 …`.
fn parse_log(raw: &str) -> (Option<String>, Option<DateTime<Utc>>) {
    let mut version = None;
    let mut last = None;
    for line in raw.lines() {
        if let Some(ts) = line
            .strip_prefix('[')
            .and_then(|rest| rest.split(']').next())
            .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
        {
            last = Some(ts.with_timezone(&Utc));
        }
        if let Some(idx) = line.find("Starting Gemini v") {
            let v: String = line[idx + "Starting Gemini v".len()..]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !v.is_empty() {
                version = Some(v);
            }
        }
    }
    (version, last)
}

pub fn detect() -> Option<GeminiDesktop> {
    let install = install_dirs()
        .into_iter()
        .find(|d| d.join("Gemini.exe").is_file())?;
    let (version, last_activity) = std::fs::read_to_string(install.join("logs").join("gemini.log"))
        .map(|raw| parse_log(&raw))
        .unwrap_or((None, None));
    let running = profile_dir().is_some_and(|p| profile_locked(&p));
    Some(GeminiDesktop {
        version,
        running,
        last_activity,
    })
}

impl GeminiDesktop {
    pub fn label(&self) -> String {
        match &self.version {
            Some(v) => format!("Gemini app v{v}"),
            None => "Gemini app".to_string(),
        }
    }

    pub fn status_line(&self) -> String {
        let state = if self.running { "running" } else { "installed" };
        let last = self
            .last_activity
            .map(|t| format!(", last active {}", t.with_timezone(&chrono::Local).format("%d/%m %H:%M")))
            .unwrap_or_default();
        format!(
            "{} {state}{last}; the desktop app keeps no quota data locally",
            self.label()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_and_last_timestamp() {
        let log = "[2026-10-30T19:53:10.176Z] [INFO] [main] Starting Gemini v1.16.0 (channel: prod)\n\
                   [2026-11-04T18:47:25.855Z] [INFO] [main] Starting Gemini v1.17.0 (channel: prod)\n\
                   [2026-11-04T18:50:00Z] [INFO] [main.cc] something\n";
        let (version, last) = parse_log(log);
        assert_eq!(version.as_deref(), Some("1.17.0"));
        assert_eq!(last.unwrap().to_rfc3339(), "2026-11-04T18:50:00+00:00");
    }
}
