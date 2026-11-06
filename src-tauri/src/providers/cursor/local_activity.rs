//! Local, read-only Cursor activity.
//!
//! Recent Cursor builds keep the session token encrypted, so the plan quota
//! can't be fetched. The same state DB (`globalStorage/state.vscdb`) still
//! holds plain metadata about what the user did:
//!
//! - `cursorDiskKV` rows `bubbleId:*`: one row per chat message, with
//!   `type` (1 = user prompt, 2 = AI reply) and `createdAt`;
//! - `cursorDiskKV` rows `composerData:*`: one per chat, with
//!   `modelConfig.modelName`;
//! - `ItemTable` row `aiCodeTracking.dailyStats.<ver>.<YYYY-MM-DD>`: lines
//!   suggested / accepted by Tab and by the agent today;
//! - `ItemTable` rows `cursorAuth/stripeMembershipType` and
//!   `cursorAuth/stripeSubscriptionStatus`: plan name and status.
//!
//! Only those fields are read. JSON fields are pulled out with SQLite's
//! `json_extract`, so message text, code and keys never enter this process.
//! The DB is opened read-only, nothing leaves the machine, and the result is
//! cached until the DB or its WAL file changes.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use chrono::{DateTime, Duration, Local, Utc};
use rusqlite::{types::ValueRef, Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CursorActivity {
    pub prompts_today: u32,
    pub prompts_7d: u32,
    pub replies_today: u32,
    pub last_prompt_at: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub tab_suggested_lines: u64,
    pub tab_accepted_lines: u64,
    pub agent_suggested_lines: u64,
    pub agent_accepted_lines: u64,
    pub membership: Option<String>,
    pub subscription_status: Option<String>,
    /// Timestamps of every user prompt still in the DB, oldest first.
    pub prompt_times: Vec<DateTime<Utc>>,
}

impl CursorActivity {
    /// Prompts sent at or after `since`.
    pub fn prompts_since(&self, since: DateTime<Utc>) -> u32 {
        let first = self.prompt_times.partition_point(|t| *t < since);
        (self.prompt_times.len() - first) as u32
    }

    /// True when the user sent a prompt within `window`.
    pub fn prompted_within(&self, window: Duration) -> bool {
        self.last_prompt_at
            .is_some_and(|t| Utc::now().signed_duration_since(t) <= window)
    }

    /// One-line summary for the card, e.g.
    /// "Today: 4 prompts · 185 lines suggested · grok-4.7 · last prompt 6m ago".
    pub fn summary(&self) -> String {
        let mut parts = vec![format!(
            "Today: {} prompt{}",
            self.prompts_today,
            if self.prompts_today == 1 { "" } else { "s" }
        )];
        let suggested = self.tab_suggested_lines + self.agent_suggested_lines;
        if suggested > 0 {
            let accepted = self.tab_accepted_lines + self.agent_accepted_lines;
            parts.push(format!("{accepted}/{suggested} lines accepted"));
        }
        if self.prompts_7d > self.prompts_today {
            parts.push(format!("7d: {}", self.prompts_7d));
        }
        if let Some(model) = &self.model {
            parts.push(model.clone());
        }
        if let Some(t) = self.last_prompt_at {
            parts.push(format!("last prompt {}", ago(t)));
        }
        parts.join(" · ")
    }
}

fn ago(t: DateTime<Utc>) -> String {
    let secs = Utc::now().signed_duration_since(t).num_seconds().max(0);
    match secs {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86_399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

/// Cache key: modification times of the DB and its WAL file.
type Stamp = (Option<SystemTime>, Option<SystemTime>);

static CACHE: Mutex<Option<(Stamp, CursorActivity)>> = Mutex::new(None);

fn stamp(db: &Path) -> Stamp {
    let mtime = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut wal = db.as_os_str().to_owned();
    wal.push("-wal");
    (mtime(db.to_path_buf()), mtime(PathBuf::from(wal)))
}

/// Read activity from `db`, reusing the previous result while the files are
/// unchanged. Blocking: call through `run_blocking`.
pub fn read(db: &Path) -> Result<CursorActivity, String> {
    let key = stamp(db);
    if let Ok(guard) = CACHE.lock() {
        if let Some((cached_key, activity)) = guard.as_ref() {
            if *cached_key == key {
                return Ok(activity.clone());
            }
        }
    }
    let activity = read_uncached(db, Local::now().date_naive())?;
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((key, activity.clone()));
    }
    Ok(activity)
}

fn read_uncached(db: &Path, today: chrono::NaiveDate) -> Result<CursorActivity, String> {
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("cannot open state DB: {e}"))?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
    read_from(&conn, today)
}

fn read_from(conn: &Connection, today: chrono::NaiveDate) -> Result<CursorActivity, String> {
    let mut activity = CursorActivity::default();
    let week_ago = Utc::now() - Duration::days(7);

    // Chat messages: type + timestamp only.
    if has_table(conn, "cursorDiskKV") {
        let mut stmt = conn
            .prepare(
                "SELECT json_extract(CAST(value AS TEXT), '$.type'),
                        json_extract(CAST(value AS TEXT), '$.createdAt')
                 FROM cursorDiskKV
                 WHERE key LIKE 'bubbleId:%' AND json_valid(CAST(value AS TEXT))",
            )
            .map_err(|e| format!("bubble query: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, Option<i64>>(0)?, timestamp(row.get_ref(1)?)))
            })
            .map_err(|e| format!("bubble query: {e}"))?;
        for (kind, created) in rows.flatten() {
            let Some(created) = created else { continue };
            let is_today = created.with_timezone(&Local).date_naive() == today;
            match kind {
                Some(1) => {
                    if is_today {
                        activity.prompts_today += 1;
                    }
                    if created >= week_ago {
                        activity.prompts_7d += 1;
                    }
                    if activity.last_prompt_at.is_none_or(|t| created > t) {
                        activity.last_prompt_at = Some(created);
                    }
                    activity.prompt_times.push(created);
                }
                Some(2) if is_today => activity.replies_today += 1,
                _ => {}
            }
        }

        activity.prompt_times.sort_unstable();

        // Model of the most recently created chat.
        activity.model = conn
            .query_row(
                "SELECT json_extract(CAST(value AS TEXT), '$.modelConfig.modelName')
                 FROM cursorDiskKV
                 WHERE key LIKE 'composerData:%' AND json_valid(CAST(value AS TEXT))
                   AND json_extract(CAST(value AS TEXT), '$.modelConfig.modelName') IS NOT NULL
                 ORDER BY json_extract(CAST(value AS TEXT), '$.createdAt') DESC
                 LIMIT 1",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten()
            .filter(|m| !m.is_empty() && m != "default");
    }

    // Today's line counters.
    if let Some(stats) = item(
        conn,
        "SELECT CAST(value AS TEXT) FROM ItemTable
         WHERE key LIKE 'aiCodeTracking.dailyStats.%' AND key LIKE ?1
         ORDER BY key DESC LIMIT 1",
        &format!("%.{}", today.format("%Y-%m-%d")),
    )
    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
    {
        let n = |k: &str| stats.get(k).and_then(Value::as_u64).unwrap_or(0);
        activity.tab_suggested_lines = n("tabSuggestedLines");
        activity.tab_accepted_lines = n("tabAcceptedLines");
        activity.agent_suggested_lines = n("composerSuggestedLines");
        activity.agent_accepted_lines = n("composerAcceptedLines");
    }

    let plain = |key: &str| {
        item(conn, "SELECT CAST(value AS TEXT) FROM ItemTable WHERE key = ?1", key)
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
    };
    activity.membership = plain("cursorAuth/stripeMembershipType");
    activity.subscription_status = plain("cursorAuth/stripeSubscriptionStatus");
    Ok(activity)
}

fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .is_ok()
}

fn item(conn: &Connection, sql: &str, param: &str) -> Option<String> {
    conn.query_row(sql, [param], |row| row.get::<_, Option<String>>(0))
        .optional()
        .ok()
        .flatten()
        .flatten()
}

/// `createdAt` is an ISO-8601 string in current builds and epoch millis in
/// older ones.
fn timestamp(value: ValueRef<'_>) -> Option<DateTime<Utc>> {
    match value {
        ValueRef::Text(t) => DateTime::parse_from_rfc3339(std::str::from_utf8(t).ok()?)
            .ok()
            .map(|d| d.with_timezone(&Utc)),
        ValueRef::Integer(ms) => DateTime::from_timestamp_millis(ms),
        ValueRef::Real(ms) => DateTime::from_timestamp_millis(ms as i64),
        _ => None,
    }
}

/// Plan name as shown on the card: "plus" → "Cursor Plus".
pub fn plan_label(activity: &CursorActivity) -> String {
    match activity.membership.as_deref() {
        Some(m) if !m.is_empty() && m != "free" => {
            let mut chars = m.chars();
            let first = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
            format!("Cursor {first}{}", chars.as_str())
        }
        Some("free") => "Cursor Free".to_string(),
        _ => "Cursor".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE ItemTable (key TEXT UNIQUE, value BLOB);
             CREATE TABLE cursorDiskKV (key TEXT UNIQUE, value BLOB);",
        )
        .unwrap();
        conn
    }

    fn put(conn: &Connection, table: &str, key: &str, value: &str) {
        conn.execute(
            &format!("INSERT INTO {table} (key, value) VALUES (?1, ?2)"),
            [key, value],
        )
        .unwrap();
    }

    #[test]
    fn counts_prompts_lines_and_plan_without_reading_text() {
        let conn = fixture();
        let now = Utc::now();
        let today = now.with_timezone(&Local).date_naive();
        let iso = |d: DateTime<Utc>| d.to_rfc3339();
        put(&conn, "cursorDiskKV", "bubbleId:a:1",
            &format!(r#"{{"type":1,"createdAt":"{}","text":"secret prompt"}}"#, iso(now - Duration::minutes(3))));
        put(&conn, "cursorDiskKV", "bubbleId:a:2",
            &format!(r#"{{"type":2,"createdAt":"{}","text":"reply"}}"#, iso(now - Duration::minutes(2))));
        put(&conn, "cursorDiskKV", "bubbleId:b:1",
            &format!(r#"{{"type":1,"createdAt":{}}}"#, (now - Duration::days(3)).timestamp_millis()));
        put(&conn, "cursorDiskKV", "bubbleId:c:1",
            &format!(r#"{{"type":1,"createdAt":"{}"}}"#, iso(now - Duration::days(30))));
        put(&conn, "cursorDiskKV", "bubbleId:broken", "not json");
        put(&conn, "cursorDiskKV", "composerData:a",
            r#"{"createdAt":2,"modelConfig":{"modelName":"grok-4.7"}}"#);
        put(&conn, "cursorDiskKV", "composerData:old",
            r#"{"createdAt":1,"modelConfig":{"modelName":"old-model"}}"#);
        put(&conn, "ItemTable", &format!("aiCodeTracking.dailyStats.v1.5.{}", today.format("%Y-%m-%d")),
            r#"{"tabSuggestedLines":10,"tabAcceptedLines":4,"composerSuggestedLines":185,"composerAcceptedLines":6}"#);
        put(&conn, "ItemTable", "cursorAuth/stripeMembershipType", "plus");

        let a = read_from(&conn, today).unwrap();
        assert_eq!(a.prompts_today, 1);
        assert_eq!(a.prompts_7d, 2);
        assert_eq!(a.prompt_times.len(), 3);
        assert_eq!(a.prompts_since(now - Duration::days(10)), 2);
        assert_eq!(a.replies_today, 1);
        assert_eq!(a.model.as_deref(), Some("grok-4.7"));
        assert_eq!((a.tab_suggested_lines, a.agent_suggested_lines), (10, 185));
        assert_eq!(a.agent_accepted_lines + a.tab_accepted_lines, 10);
        assert!(a.prompted_within(Duration::minutes(5)));
        assert_eq!(plan_label(&a), "Cursor Plus");
        let s = a.summary();
        assert!(s.starts_with("Today: 1 prompt ·"), "{s}");
        assert!(s.contains("10/195 lines accepted") && s.contains("grok-4.7"), "{s}");
        assert!(!s.contains("secret"));
    }

    #[test]
    fn empty_db_is_fine() {
        let conn = fixture();
        let a = read_from(&conn, Local::now().date_naive()).unwrap();
        assert_eq!(a, CursorActivity::default());
        assert_eq!(a.summary(), "Today: 0 prompts");
        assert_eq!(plan_label(&a), "Cursor");
    }
}
