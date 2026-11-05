//! Last time each provider's local session transcripts changed.
//!
//! The file watcher records every relevant write here (before debouncing);
//! the poll scheduler reads it to switch a provider to fast polling while a
//! CLI session is actively running, and back to the normal cadence when idle.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use crate::usage::models::ProviderId;

static LAST_ACTIVITY: [AtomicI64; 4] = [
    AtomicI64::new(0),
    AtomicI64::new(0),
    AtomicI64::new(0),
    AtomicI64::new(0),
];

fn slot(provider: ProviderId) -> &'static AtomicI64 {
    let index = ProviderId::all()
        .iter()
        .position(|p| *p == provider)
        .unwrap_or(0);
    &LAST_ACTIVITY[index.min(LAST_ACTIVITY.len() - 1)]
}

pub fn record(provider: ProviderId) {
    slot(provider).store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
}

/// Whether the provider's transcripts changed within `window`.
pub fn is_active(provider: ProviderId, window: Duration) -> bool {
    let last = slot(provider).load(Ordering::Relaxed);
    last > 0 && chrono::Utc::now().timestamp() - last <= window.as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_expires_activity() {
        assert!(!is_active(ProviderId::Cursor, Duration::from_secs(60)));
        record(ProviderId::Cursor);
        assert!(is_active(ProviderId::Cursor, Duration::from_secs(60)));
        slot(ProviderId::Cursor).store(chrono::Utc::now().timestamp() - 600, Ordering::Relaxed);
        assert!(!is_active(ProviderId::Cursor, Duration::from_secs(60)));
    }
}
