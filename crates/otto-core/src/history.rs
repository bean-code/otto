use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::agent::{AgentStep, AgentTurn};
use crate::error::{CoreError, CoreResult};

/// One past command, as read back from storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub input: String,
    pub reply: String,
    pub steps: Vec<AgentStep>,
}

/// Local record of every command Otto has run - written on every turn and
/// readable back, so the UI can show a history panel and so a future API
/// layer over `otto-core` has real, queryable data to serve rather than a
/// flat log it has to re-parse.
#[async_trait]
pub trait HistoryStore: Send + Sync {
    async fn record(&self, input: &str, turn: &AgentTurn) -> CoreResult<()>;

    /// Most recent entries first, newest-to-oldest, capped at `limit`.
    async fn list_recent(&self, limit: usize) -> CoreResult<Vec<HistoryEntry>>;
}

/// SQLite-backed history (via `rusqlite`'s bundled SQLite - no system
/// library dependency). Chosen over a flat JSONL log specifically so this
/// is ready to serve a future API layer: real queries (`ORDER BY`,
/// `LIMIT`, eventually `WHERE` on tool name or date range) instead of
/// reading and re-parsing an ever-growing file on every request.
pub struct SqliteHistory {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteHistory {
    pub fn open(path: PathBuf) -> CoreResult<Self> {
        let conn = Connection::open(&path).map_err(|e| CoreError::History(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp  TEXT NOT NULL,
                input      TEXT NOT NULL,
                reply      TEXT NOT NULL,
                steps_json TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS history_timestamp_idx ON history (timestamp DESC);",
        )
        .map_err(|e| CoreError::History(e.to_string()))?;

        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn default_path() -> CoreResult<PathBuf> {
        let dirs = directories::ProjectDirs::from("dev", "ayimdomnic", "otto")
            .ok_or_else(|| CoreError::History("could not resolve a data directory for this platform".into()))?;
        let dir = dirs.data_dir();
        std::fs::create_dir_all(dir).map_err(|e| CoreError::History(e.to_string()))?;
        Ok(dir.join("history.sqlite3"))
    }
}

#[async_trait]
impl HistoryStore for SqliteHistory {
    async fn record(&self, input: &str, turn: &AgentTurn) -> CoreResult<()> {
        let conn = self.conn.clone();
        let input = input.to_string();
        let reply = turn.reply.clone();
        let timestamp = Utc::now().to_rfc3339();
        let steps_json = serde_json::to_string(&turn.steps).map_err(|e| CoreError::History(e.to_string()))?;

        tokio::task::spawn_blocking(move || -> CoreResult<()> {
            let conn = conn.lock().expect("history db mutex poisoned");
            conn.execute(
                "INSERT INTO history (timestamp, input, reply, steps_json) VALUES (?1, ?2, ?3, ?4)",
                params![timestamp, input, reply, steps_json],
            )
            .map_err(|e| CoreError::History(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| CoreError::History(e.to_string()))??;

        Ok(())
    }

    async fn list_recent(&self, limit: usize) -> CoreResult<Vec<HistoryEntry>> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || -> CoreResult<Vec<HistoryEntry>> {
            let conn = conn.lock().expect("history db mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT id, timestamp, input, reply, steps_json FROM history ORDER BY id DESC LIMIT ?1")
                .map_err(|e| CoreError::History(e.to_string()))?;

            let rows = stmt
                .query_map(params![limit as i64], |row| {
                    let id: i64 = row.get(0)?;
                    let timestamp: String = row.get(1)?;
                    let input: String = row.get(2)?;
                    let reply: String = row.get(3)?;
                    let steps_json: String = row.get(4)?;
                    Ok((id, timestamp, input, reply, steps_json))
                })
                .map_err(|e| CoreError::History(e.to_string()))?;

            let mut entries = Vec::new();
            for row in rows {
                let (id, timestamp, input, reply, steps_json) = row.map_err(|e| CoreError::History(e.to_string()))?;
                let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                    .map_err(|e| CoreError::History(e.to_string()))?
                    .with_timezone(&Utc);
                let steps: Vec<AgentStep> =
                    serde_json::from_str(&steps_json).map_err(|e| CoreError::History(e.to_string()))?;
                entries.push(HistoryEntry { id, timestamp, input, reply, steps });
            }
            Ok(entries)
        })
        .await
        .map_err(|e| CoreError::History(e.to_string()))?
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

    #[tokio::test]
    async fn records_and_lists_entries_newest_first() {
        let dir = std::env::temp_dir().join(format!("otto-history-test-{}", uuid_like()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("history.sqlite3");

        let store = SqliteHistory::open(db_path).unwrap();
        store.record("first command", &turn("first reply")).await.unwrap();
        store.record("second command", &turn("second reply")).await.unwrap();

        let entries = store.list_recent(10).await.unwrap();
        assert_eq!(entries.len(), 2);
        // Newest first.
        assert_eq!(entries[0].input, "second command");
        assert_eq!(entries[0].reply, "second reply");
        assert_eq!(entries[1].input, "first command");
        assert_eq!(entries[0].steps.len(), 1);
        assert_eq!(entries[0].steps[0].tool_name, "github_list_open_prs");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn list_recent_respects_limit() {
        let dir = std::env::temp_dir().join(format!("otto-history-test-{}", uuid_like()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = SqliteHistory::open(dir.join("history.sqlite3")).unwrap();

        for i in 0..5 {
            store.record(&format!("command {i}"), &turn("reply")).await.unwrap();
        }

        let entries = store.list_recent(2).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].input, "command 4");
        assert_eq!(entries[1].input, "command 3");

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn uuid_like() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        format!("{}-{:?}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), std::thread::current().id())
            .replace(['(', ')', ' '], "")
    }
}
