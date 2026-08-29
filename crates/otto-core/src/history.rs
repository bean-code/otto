use std::path::PathBuf;

use async_trait::async_trait;
use serde::Serialize;

use crate::agent::AgentTurn;
use crate::error::{CoreError, CoreResult};

/// Local record of every command Otto has run, so the UI can show a history
/// panel and so runs are auditable after the fact.
#[async_trait]
pub trait HistoryStore: Send + Sync {
    async fn record(&self, input: &str, turn: &AgentTurn) -> CoreResult<()>;
}

#[derive(Serialize)]
struct HistoryEntry<'a> {
    timestamp: chrono::DateTime<chrono::Utc>,
    input: &'a str,
    reply: &'a str,
    steps: &'a [crate::agent::AgentStep],
}

/// Appends one JSON line per turn to a local file (`history.jsonl` in the
/// OS data directory by default).
pub struct JsonFileHistory {
    path: PathBuf,
}

impl JsonFileHistory {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> CoreResult<PathBuf> {
        let dirs = directories::ProjectDirs::from("dev", "ayimdomnic", "otto")
            .ok_or_else(|| CoreError::History("could not resolve a data directory for this platform".into()))?;
        let dir = dirs.data_dir();
        std::fs::create_dir_all(dir).map_err(|e| CoreError::History(e.to_string()))?;
        Ok(dir.join("history.jsonl"))
    }
}

#[async_trait]
impl HistoryStore for JsonFileHistory {
    async fn record(&self, input: &str, turn: &AgentTurn) -> CoreResult<()> {
        let entry = HistoryEntry {
            timestamp: chrono::Utc::now(),
            input,
            reply: &turn.reply,
            steps: &turn.steps,
        };
        let mut line = serde_json::to_string(&entry).map_err(|e| CoreError::History(e.to_string()))?;
        line.push('\n');

        let path = self.path.clone();
        tokio::task::spawn_blocking(move || -> CoreResult<()> {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| CoreError::History(e.to_string()))?;
            file.write_all(line.as_bytes()).map_err(|e| CoreError::History(e.to_string()))
        })
        .await
        .map_err(|e| CoreError::History(e.to_string()))??;

        Ok(())
    }
}
