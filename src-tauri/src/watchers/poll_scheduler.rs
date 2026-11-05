use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::async_runtime::JoinHandle;

use crate::usage::models::ProviderId;

#[derive(Clone, Default)]
pub struct PollScheduler {
    per_provider: Arc<Mutex<HashMap<ProviderId, JoinHandle<()>>>>,
}

impl PollScheduler {
    pub fn new() -> Self {
        Self {
            per_provider: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn one poll timer per provider.
    ///
    /// Unlike [`Self::restart`], which drives a single global timer, this keeps
    /// an independent timer for each provider so different providers can run at
    /// different cadences. The `per_provider` closure is invoked on every tick
    /// of the provider whose id it is, so it is responsible for resolving that
    /// provider's effective cadence (per-provider override, falling back to the
    /// global default) and deciding whether a refresh is due. A 5s base is
    /// used as the shared wake-up resolution (fine enough for the 30s
    /// active-session cadence); the closure's own throttle keeps slower
    /// providers from over-firing, and an idle tick costs only a mutex read.
    pub fn restart_per_provider<F>(&self, per_provider: F)
    where
        F: Fn(ProviderId) + Send + Sync + 'static,
    {
        if let Ok(mut guard) = self.per_provider.lock() {
            for handle in guard.drain() {
                handle.1.abort();
            }

            let callback = Arc::new(per_provider);
            for provider in ProviderId::all() {
                let cb = Arc::clone(&callback);
                let handle = tauri::async_runtime::spawn(async move {
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        interval.tick().await;
                        cb(provider);
                    }
                });
                guard.insert(provider, handle);
            }
        }
    }
}
