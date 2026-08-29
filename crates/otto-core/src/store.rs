use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::agent::{AgentStep, AgentTurn};
use crate::error::{CoreError, CoreResult};

/// What kind of thing an [`ActivityEntry`] records - not every activity is
/// a command the user typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    /// A natural-language command run through the agent loop.
    Command,
    /// A note from the background growth digest.
    GrowthDigest,
    /// A working-hours schedule transition (Otto turning itself off).
    Schedule,
}

/// One past activity, as read back from storage - a command, a growth-digest
/// note, or a schedule event, all in one feed so the UI (and, eventually, an
/// API layer over `otto-core`) has a single place to ask "what has Otto been
/// doing" instead of stitching several sources together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub kind: ActivityKind,
    /// The command text for a `Command`, or a short headline for anything
    /// else (e.g. "Growth digest", "Scheduled shutdown").
    pub title: String,
    /// The reply text for a `Command`, or the note/detail for anything else.
    pub detail: String,
    /// Only populated for `Command` activities - empty otherwise.
    pub steps: Vec<AgentStep>,
}

/// Every activity Otto has recorded - written as things happen and readable
/// back, so the UI can show an Activities feed and so a future API layer
/// over `otto-core` has real, queryable data to serve rather than a flat
/// log it has to re-parse.
#[async_trait]
pub trait ActivityLog: Send + Sync {
    async fn record_command(&self, input: &str, turn: &AgentTurn) -> CoreResult<()>;
    async fn record_event(&self, kind: ActivityKind, title: &str, detail: &str) -> CoreResult<()>;

    /// Most recent entries first, newest-to-oldest, capped at `limit`.
    async fn list_recent(&self, limit: usize) -> CoreResult<Vec<ActivityEntry>>;
}

/// Otto's local SQLite database (via `rusqlite`'s bundled SQLite - no
/// system library dependency): the activity feed, and small pieces of
/// app-level settings that need to persist across restarts (like which
/// model is selected). One local file, one connection, two tables - both
/// concerns live on this one type rather than two separate stores fighting
/// over the same file, but only `ActivityLog` is behind a trait: there's
/// only ever going to be one real implementation of "Otto's local
/// database," so a trait for the settings half would be speculative.
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open(path: PathBuf) -> CoreResult<Self> {
        let conn = Connection::open(&path).map_err(|e| CoreError::History(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS activities (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp  TEXT NOT NULL,
                kind       TEXT NOT NULL,
                title      TEXT NOT NULL,
                detail     TEXT NOT NULL,
                steps_json TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS activities_timestamp_idx ON activities (timestamp DESC);

             CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );",
        )
        .map_err(|e| CoreError::History(e.to_string()))?;

        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn default_path() -> CoreResult<PathBuf> {
        let dirs = directories::ProjectDirs::from("dev", "ayimdomnic", "otto")
            .ok_or_else(|| CoreError::History("could not resolve a data directory for this platform".into()))?;
        let dir = dirs.data_dir();
        std::fs::create_dir_all(dir).map_err(|e| CoreError::History(e.to_string()))?;
        Ok(dir.join("otto.sqlite3"))
    }

    /// Reads a persisted setting, if any - e.g. the last model the user
    /// picked. `None` if it was never set.
    pub async fn get_setting(&self, key: &str) -> CoreResult<Option<String>> {
        let conn = self.conn.clone();
        let key = key.to_string();

        tokio::task::spawn_blocking(move || -> CoreResult<Option<String>> {
            let conn = conn.lock().expect("store db mutex poisoned");
            conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
                .optional()
                .map_err(|e| CoreError::History(e.to_string()))
        })
        .await
        .map_err(|e| CoreError::History(e.to_string()))?
    }

    /// Persists a setting, overwriting any previous value.
    pub async fn set_setting(&self, key: &str, value: &str) -> CoreResult<()> {
        let conn = self.conn.clone();
        let key = key.to_string();
        let value = value.to_string();

        tokio::task::spawn_blocking(move || -> CoreResult<()> {
            let conn = conn.lock().expect("store db mutex poisoned");
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|e| CoreError::History(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| CoreError::History(e.to_string()))?
    }
}

fn kind_to_str(kind: ActivityKind) -> &'static str {
    match kind {
        ActivityKind::Command => "command",
        ActivityKind::GrowthDigest => "growth_digest",
        ActivityKind::Schedule => "schedule",
    }
}

fn kind_from_str(s: &str) -> CoreResult<ActivityKind> {
    match s {
        "command" => Ok(ActivityKind::Command),
        "growth_digest" => Ok(ActivityKind::GrowthDigest),
        "schedule" => Ok(ActivityKind::Schedule),
        other => Err(CoreError::History(format!("unknown activity kind '{other}' in database"))),
    }
}

#[async_trait]
impl ActivityLog for SqliteStore {
    async fn record_command(&self, input: &str, turn: &AgentTurn) -> CoreResult<()> {
        let steps_json = serde_json::to_string(&turn.steps).map_err(|e| CoreError::History(e.to_string()))?;
        self.insert("command", input, &turn.reply, &steps_json).await
    }

    async fn record_event(&self, kind: ActivityKind, title: &str, detail: &str) -> CoreResult<()> {
        self.insert(kind_to_str(kind), title, detail, "[]").await
    }

    async fn list_recent(&self, limit: usize) -> CoreResult<Vec<ActivityEntry>> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || -> CoreResult<Vec<ActivityEntry>> {
            let conn = conn.lock().expect("store db mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT id, timestamp, kind, title, detail, steps_json FROM activities ORDER BY id DESC LIMIT ?1")
                .map_err(|e| CoreError::History(e.to_string()))?;

            let rows = stmt
                .query_map(params![limit as i64], |row| {
                    let id: i64 = row.get(0)?;
                    let timestamp: String = row.get(1)?;
                    let kind: String = row.get(2)?;
                    let title: String = row.get(3)?;
                    let detail: String = row.get(4)?;
                    let steps_json: String = row.get(5)?;
                    Ok((id, timestamp, kind, title, detail, steps_json))
                })
                .map_err(|e| CoreError::History(e.to_string()))?;

            let mut entries = Vec::new();
            for row in rows {
                let (id, timestamp, kind, title, detail, steps_json) = row.map_err(|e| CoreError::History(e.to_string()))?;
                let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                    .map_err(|e| CoreError::History(e.to_string()))?
                    .with_timezone(&Utc);
                let kind = kind_from_str(&kind)?;
                let steps: Vec<AgentStep> =
                    serde_json::from_str(&steps_json).map_err(|e| CoreError::History(e.to_string()))?;
                entries.push(ActivityEntry { id, timestamp, kind, title, detail, steps });
            }
            Ok(entries)
        })
        .await
        .map_err(|e| CoreError::History(e.to_string()))?
    }
}

impl SqliteStore {
    async fn insert(&self, kind: &str, title: &str, detail: &str, steps_json: &str) -> CoreResult<()> {
        let conn = self.conn.clone();
        let kind = kind.to_string();
        let title = title.to_string();
        let detail = detail.to_string();
        let steps_json = steps_json.to_string();
        let timestamp = Utc::now().to_rfc3339();

        tokio::task::spawn_blocking(move || -> CoreResult<()> {
            let conn = conn.lock().expect("store db mutex poisoned");
            conn.execute(
                "INSERT INTO activities (timestamp, kind, title, detail, steps_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![timestamp, kind, title, detail, steps_json],
            )
            .map_err(|e| CoreError::History(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| CoreError::History(e.to_string()))??;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentStep;
    use serde_json::json;

    fn turn(reply: &str) -> AgentTurn {
        AgentTurn {
            reply: reply.to_string(),
            steps: vec![AgentStep {
                tool_name: "github_list_open_prs".to_string(),
                summary: "2 open PRs".to_string(),
                data: json!({ "repo": "acme/widgets" }),
            }],
        }
    }

    fn temp_store() -> SqliteStore {
        let dir = std::env::temp_dir().join(format!("otto-store-test-{}", uuid_like()));
        std::fs::create_dir_all(&dir).unwrap();
        SqliteStore::open(dir.join("otto.sqlite3")).unwrap()
    }

    #[tokio::test]
    async fn records_and_lists_commands_newest_first() {
        let store = temp_store();
        store.record_command("first command", &turn("first reply")).await.unwrap();
        store.record_command("second command", &turn("second reply")).await.unwrap();

        let entries = store.list_recent(10).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "second command");
        assert_eq!(entries[0].detail, "second reply");
        assert_eq!(entries[0].kind, ActivityKind::Command);
        assert_eq!(entries[1].title, "first command");
        assert_eq!(entries[0].steps.len(), 1);
        assert_eq!(entries[0].steps[0].tool_name, "github_list_open_prs");
    }

    #[tokio::test]
    async fn list_recent_respects_limit() {
        let store = temp_store();
        for i in 0..5 {
            store.record_command(&format!("command {i}"), &turn("reply")).await.unwrap();
        }

        let entries = store.list_recent(2).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "command 4");
        assert_eq!(entries[1].title, "command 3");
    }

    #[tokio::test]
    async fn mixes_commands_and_events_in_one_feed() {
        let store = temp_store();
        store.record_command("show my prs", &turn("2 open PRs")).await.unwrap();
        store
            .record_event(ActivityKind::GrowthDigest, "Growth digest", "You've merged 3 PRs without tests this week.")
            .await
            .unwrap();
        store.record_event(ActivityKind::Schedule, "Scheduled shutdown", "Active window ended at 18:00.").await.unwrap();

        let entries = store.list_recent(10).await.unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].kind, ActivityKind::Schedule);
        assert_eq!(entries[1].kind, ActivityKind::GrowthDigest);
        assert_eq!(entries[2].kind, ActivityKind::Command);
        assert!(entries[0].steps.is_empty());
    }

    #[tokio::test]
    async fn settings_round_trip_and_overwrite() {
        let store = temp_store();
        assert_eq!(store.get_setting("model").await.unwrap(), None);

        store.set_setting("model", "claude-opus-5").await.unwrap();
        assert_eq!(store.get_setting("model").await.unwrap(), Some("claude-opus-5".to_string()));

        store.set_setting("model", "claude-sonnet-5").await.unwrap();
        assert_eq!(store.get_setting("model").await.unwrap(), Some("claude-sonnet-5".to_string()));
    }

    fn uuid_like() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        format!("{}-{:?}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), std::thread::current().id())
            .replace(['(', ')', ' '], "")
    }
}
