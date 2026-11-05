use std::sync::Arc;

use async_trait::async_trait;

use crate::usage::models::{CostEntry, ProviderId, ProviderStatus, UsageSnapshot};

pub mod antigravity;
pub mod auth;
pub mod claude;
pub mod codex;
pub mod cursor;
pub(crate) mod env;
pub(crate) mod process;
pub mod http;
pub(crate) mod litellm_proxy;
pub mod registry;

#[derive(Debug, Clone)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub display_name: &'static str,
    pub brand_color: &'static str,
}

/// Per-refresh context handed to providers. Credentials are discovered by each
/// provider from the tool's own local files, so nothing needs to be carried
/// here; the struct is kept as an extension point for the provider trait.
#[derive(Clone, Default, Debug)]
pub struct FetchContext {
    /// Set for refreshes triggered by a file change: those must always rescan
    /// so the newest transcript lines are picked up, so they skip coalescing.
    pub from_watcher: bool,
}

/// Run synchronous filesystem work (log scanning, JSON parsing) on tokio's
/// blocking pool so it never stalls the async runtime that drives IPC, the
/// tray and the file watcher.
pub(crate) async fn run_blocking<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| format!("background scan task failed: {e}"))
}

/// Remembers the last good live snapshot and a cool-down after a failed live
/// call. There are no retry loops: after a failure the provider serves the
/// cached/local data until the cool-down passes, then tries once again on the
/// next scheduled refresh.
#[derive(Default)]
pub(crate) struct LiveGate {
    inner: std::sync::Mutex<LiveGateState>,
}

#[derive(Default)]
struct LiveGateState {
    last_ok: Option<UsageSnapshot>,
    failed_at: Option<std::time::Instant>,
}

impl LiveGate {
    const COOLDOWN: std::time::Duration = std::time::Duration::from_secs(120);

    /// `false` while cooling down after a failure.
    pub fn may_call(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.failed_at.is_none_or(|t| t.elapsed() >= Self::COOLDOWN))
            .unwrap_or(true)
    }

    pub fn ok(&self, snapshot: &UsageSnapshot) {
        if let Ok(mut g) = self.inner.lock() {
            g.last_ok = Some(snapshot.clone());
            g.failed_at = None;
        }
    }

    pub fn failed(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.failed_at = Some(std::time::Instant::now());
        }
    }

    /// Last good live snapshot, unchanged, if it was fetched within
    /// `max_age`. Used to cap how often a provider API is called no matter how
    /// many triggers (file watcher, scheduler, UI) fire in between.
    pub fn recent(&self, max_age: std::time::Duration) -> Option<UsageSnapshot> {
        let g = self.inner.lock().ok()?;
        let snap = g.last_ok.as_ref()?;
        let age = (chrono::Utc::now() - snap.fetched_at).to_std().ok()?;
        (age < max_age).then(|| snap.clone())
    }

    /// Last good live snapshot (marked stale) if it is at most
    /// `STALE_LIVE_MAX_MINUTES` old; older figures are not shown as live.
    pub fn cached(&self) -> Option<UsageSnapshot> {
        let g = self.inner.lock().ok()?;
        let snap = g.last_ok.as_ref()?;
        if chrono::Utc::now() - snap.fetched_at
            > chrono::Duration::minutes(crate::usage::aggregator::STALE_LIVE_MAX_MINUTES)
        {
            return None;
        }
        let mut stale = snap.clone();
        stale.stale = true;
        Some(stale)
    }
}

/// Minimum spacing between two live API calls for the same provider.
pub(crate) const MIN_LIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

// `async_trait` expands each method with a `#[must_use]` on a type that is
// already must-use; newer clippy flags that inside the macro output.
#[allow(clippy::double_must_use)]
#[async_trait]
// Every provider implementation exposes the same small contract: fetch current
// usage, fetch cost history, and report health. The registry and aggregator rely
// on this trait to keep provider-specific branching out of the command layer.
pub trait UsageProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn descriptor(&self) -> &ProviderDescriptor;
    async fn fetch_usage(&self, ctx: &FetchContext) -> Result<UsageSnapshot, String>;
    async fn fetch_cost_history(&self, days: u32) -> Result<Vec<CostEntry>, String>;
    async fn check_status(&self) -> ProviderStatus;
    fn compute_auth_state(&self, ctx: &FetchContext) -> auth::AuthState;
}

pub type DynProvider = Arc<dyn UsageProvider>;
