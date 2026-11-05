//! Event-driven refresh trigger for local CLI/IDE artifacts.
//!
//! A single OS-native watcher (ReadDirectoryChangesW on Windows, FSEvents on
//! macOS, inotify on Linux) covers every provider directory, and one async task
//! turns its raw events into per-provider refreshes:
//!
//! * events are filtered to content changes of `.json` / `.jsonl` files, so
//!   noisy IDE `.log` churn and metadata/access events cost nothing;
//! * each provider is debounced with a short trailing delay plus a hard cap,
//!   so a CLI streaming tokens into a transcript produces a refresh every few
//!   seconds rather than one per write — and the *final* write is never lost;
//! * directories that do not exist yet (tool installed after launch) are
//!   re-armed once a minute without any extra thread.
//!
//! When nothing is happening the task is parked on its channel and costs no
//! CPU.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::event::{EventKind, ModifyKind};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::usage::models::ProviderId;
use crate::usage_scanners::{auth_files, watch_roots};
use crate::watchers::activity;

/// Quiet period after the last relevant write before refreshing.
const DEBOUNCE: Duration = Duration::from_millis(1500);
/// Upper bound on how long continuous writes can postpone a refresh.
const MAX_DELAY: Duration = Duration::from_secs(5);
/// How often to try arming roots that did not exist yet.
const REARM_INTERVAL: Duration = Duration::from_secs(60);

struct PendingRefresh {
    first_seen: Instant,
    due: Instant,
}

/// Start watching every provider root. `callback` is invoked (from the async
/// runtime) with the provider whose local data changed.
pub fn start<F>(callback: F)
where
    F: Fn(ProviderId) + Send + Sync + 'static,
{
    let callback = Arc::new(callback);
    tauri::async_runtime::spawn(async move {
        if let Err(err) = run(callback).await {
            warn!("file watcher stopped: {err}");
        }
    });
}

async fn run(callback: Arc<dyn Fn(ProviderId) + Send + Sync>) -> Result<(), String> {
    let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            let Ok(event) = res else { return };
            if !is_content_change(&event.kind) {
                return;
            }
            for path in event.paths {
                if is_tracked_file(&path) {
                    let _ = tx.send(path);
                }
            }
        },
        Config::default(),
    )
    .map_err(|e| e.to_string())?;

    let roots = watch_roots();
    let mut armed = HashSet::<PathBuf>::new();
    arm_roots(&mut watcher, &roots, &mut armed);
    let auth = auth_files();
    let mut armed_auth = HashSet::<PathBuf>::new();
    arm_auth_dirs(&mut watcher, &auth, &mut armed_auth);

    let mut pending = HashMap::<ProviderId, PendingRefresh>::new();
    let mut rearm = tokio::time::interval(REARM_INTERVAL);
    rearm.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    rearm.tick().await; // first tick fires immediately; roots were just armed

    loop {
        let next_due = pending.values().map(|p| p.due).min();
        tokio::select! {
            maybe_path = rx.recv() => {
                let Some(path) = maybe_path else { break };
                // A CLI login/renewal: refresh that provider so its badge flips
                // from "local" to "live" right away.
                let provider = match auth_file_provider(&path, &auth) {
                    Some(provider) => provider,
                    None => {
                        let Some(provider) = provider_for(&path, &roots) else { continue };
                        activity::record(provider);
                        provider
                    }
                };
                let now = Instant::now();
                pending
                    .entry(provider)
                    .and_modify(|p| p.due = (now + DEBOUNCE).min(p.first_seen + MAX_DELAY))
                    .or_insert(PendingRefresh { first_seen: now, due: now + DEBOUNCE });
            }
            _ = sleep_until_opt(next_due), if next_due.is_some() => {
                let now = Instant::now();
                let ready: Vec<ProviderId> = pending
                    .iter()
                    .filter(|(_, p)| p.due <= now)
                    .map(|(id, _)| *id)
                    .collect();
                for provider in ready {
                    pending.remove(&provider);
                    debug!("[watcher] local data changed for {}", provider.as_str());
                    callback(provider);
                }
            }
            _ = rearm.tick() => {
                if armed.len() < roots.len() {
                    arm_roots(&mut watcher, &roots, &mut armed);
                }
                arm_auth_dirs(&mut watcher, &auth, &mut armed_auth);
            }
        }
    }

    drop(watcher);
    Ok(())
}

async fn sleep_until_opt(deadline: Option<Instant>) {
    if let Some(deadline) = deadline {
        tokio::time::sleep_until(deadline).await;
    }
}

fn arm_roots(
    watcher: &mut RecommendedWatcher,
    roots: &[(ProviderId, PathBuf)],
    armed: &mut HashSet<PathBuf>,
) {
    for (provider, dir) in roots {
        if armed.contains(dir) || !dir.is_dir() {
            continue;
        }
        match watcher.watch(dir, RecursiveMode::Recursive) {
            Ok(()) => {
                info!("[watcher] watching {} for {}", dir.display(), provider.as_str());
                armed.insert(dir.clone());
            }
            Err(err) => warn!("[watcher] cannot watch {}: {err}", dir.display()),
        }
    }
}

/// Watch the directory of each credential file (non-recursively; events for
/// other files in it are dropped by `auth_file_provider`/`provider_for`).
fn arm_auth_dirs(
    watcher: &mut RecommendedWatcher,
    files: &[(ProviderId, PathBuf)],
    armed: &mut HashSet<PathBuf>,
) {
    for (_, file) in files {
        let Some(dir) = file.parent() else { continue };
        if armed.contains(dir) || !dir.is_dir() {
            continue;
        }
        if watcher.watch(dir, RecursiveMode::NonRecursive).is_ok() {
            debug!("[watcher] watching credential dir {}", dir.display());
            armed.insert(dir.to_path_buf());
        }
    }
}

fn auth_file_provider(path: &Path, files: &[(ProviderId, PathBuf)]) -> Option<ProviderId> {
    let wanted = path.to_string_lossy();
    files
        .iter()
        .find(|(_, file)| file.to_string_lossy().eq_ignore_ascii_case(&wanted))
        .map(|(provider, _)| *provider)
}

fn is_content_change(kind: &EventKind) -> bool {
    match kind {
        EventKind::Create(_) | EventKind::Any => true,
        EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Modify(_) => true,
        _ => false,
    }
}

/// Only session transcripts / chat logs feed the scanners.
fn is_tracked_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("json") | Some("jsonl")
    )
}

/// Map a changed path to its provider via the longest matching root.
fn provider_for(path: &Path, roots: &[(ProviderId, PathBuf)]) -> Option<ProviderId> {
    roots
        .iter()
        .filter(|(_, root)| path.starts_with(root))
        .max_by_key(|(_, root)| root.components().count())
        .map(|(provider, _)| *provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_paths_to_longest_root() {
        let roots = vec![
            (ProviderId::Antigravity, PathBuf::from("/h/.gemini/tmp")),
            (ProviderId::Codex, PathBuf::from("/h/.codex/sessions")),
        ];
        assert_eq!(
            provider_for(Path::new("/h/.codex/sessions/2026/a.jsonl"), &roots),
            Some(ProviderId::Codex)
        );
        assert_eq!(
            provider_for(Path::new("/h/.gemini/tmp/abc/chats/s.json"), &roots),
            Some(ProviderId::Antigravity)
        );
        assert_eq!(provider_for(Path::new("/elsewhere/x.json"), &roots), None);
    }

    #[test]
    fn ignores_non_transcript_files() {
        assert!(is_tracked_file(Path::new("a/b.jsonl")));
        assert!(is_tracked_file(Path::new("a/logs.json")));
        assert!(!is_tracked_file(Path::new("a/Antigravity.log")));
        assert!(!is_tracked_file(Path::new("a/.credentials")));
    }
}
