use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

const CURRENT_PROFILE_KEY: &str = "current_profile_id";
/// The id of the profile an existing (pre-multi-profile) install is
/// auto-migrated into - see [`ProfileStore::open`]. Its data stays at
/// exactly the paths it already used (`SqliteStore::default_path()`, the
/// unsuffixed `"otto-assistant"` keychain service), so upgrading needs no
/// data migration and no re-entered credentials.
pub const LOCAL_PROFILE_ID: &str = "local";

/// Where all of Otto's per-installation (OS-account-scoped, via
/// `directories::ProjectDirs`) data lives - `profiles.sqlite3` sits here
/// directly; each profile's own activity/settings database lives in a
/// `profiles/{id}/` subdirectory (see `SqliteStore::path_for_profile`).
/// `OTTO_DATA_DIR` overrides it, same override pattern as
/// `otto_voice::default_model_path`'s `OTTO_WHISPER_MODEL` - also what
/// this module's own tests use to avoid touching the real user data dir.
pub(crate) fn data_dir() -> CoreResult<PathBuf> {
    if let Ok(dir) = std::env::var("OTTO_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let dirs = directories::ProjectDirs::from("dev", "ayimdomnic", "otto").ok_or_else(|| {
        CoreError::History("could not resolve a data directory for this platform".into())
    })?;
    Ok(dirs.data_dir().to_path_buf())
}

/// A local sign-in - not an account on any Otto-operated server, just a
/// named identity (usually a GitHub one) that a device/loopback OAuth flow
/// or a pasted token resolved, used to pick which scoped
/// `SqliteStore`/`KeyringStore` to use. See `crates/otto-skill-github` for
/// how one gets created (`fetch_identity`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// The GitHub login for a GitHub-authenticated profile, or
    /// `LOCAL_PROFILE_ID` for the auto-migrated pre-profiles install.
    pub id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub github_login: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// The top-level profile registry - `{data_dir}/profiles.sqlite3`, always
/// unscoped (it has to be readable before any profile is "entered").
/// Mirrors `SqliteStore`'s own connection/error-handling pattern
/// (`crates/otto-core/src/store.rs`) rather than introducing a new one.
pub struct ProfileStore {
    conn: Arc<Mutex<Connection>>,
}

impl ProfileStore {
    /// Opens (creating if needed) the profile registry at the real,
    /// OS-resolved data directory - what every real caller uses.
    pub fn open() -> CoreResult<Self> {
        Self::open_at(&data_dir()?)
    }

    /// Opens (creating if needed) the registry at an explicit directory -
    /// `open()`'s real logic, factored out so tests can point it at a temp
    /// directory directly instead of racing a process-global env var
    /// across Rust's parallel-by-default test execution. Also - the one
    /// time this matters - auto-migrates a pre-profiles install: if the
    /// registry is brand new (no profiles yet) and the old unscoped
    /// `otto.sqlite3` already exists in `dir`, creates `LOCAL_PROFILE_ID`
    /// pointing at it and makes it current, so upgrading needs no data
    /// migration or forced re-authentication.
    pub fn open_at(dir: &std::path::Path) -> CoreResult<Self> {
        std::fs::create_dir_all(dir).map_err(|e| CoreError::History(e.to_string()))?;
        let conn = Connection::open(dir.join("profiles.sqlite3"))
            .map_err(|e| CoreError::History(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profiles (
                id           TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                avatar_url   TEXT,
                github_login TEXT,
                created_at   TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );",
        )
        .map_err(|e| CoreError::History(e.to_string()))?;

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        let is_empty: bool = {
            let conn = store.conn.lock().expect("profile store db mutex poisoned");
            conn.query_row("SELECT COUNT(*) FROM profiles", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| CoreError::History(e.to_string()))?
                == 0
        };
        if is_empty && dir.join("otto.sqlite3").exists() {
            let local = Profile {
                id: LOCAL_PROFILE_ID.to_string(),
                display_name: "Local".to_string(),
                avatar_url: None,
                github_login: None,
                created_at: Utc::now(),
            };
            store.upsert(&local)?;
            store.set_current(LOCAL_PROFILE_ID)?;
        }

        Ok(store)
    }

    pub fn list(&self) -> CoreResult<Vec<Profile>> {
        let conn = self.conn.lock().expect("profile store db mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT id, display_name, avatar_url, github_login, created_at FROM profiles ORDER BY created_at")
            .map_err(|e| CoreError::History(e.to_string()))?;
        let rows = stmt
            .query_map([], row_to_profile)
            .map_err(|e| CoreError::History(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| CoreError::History(e.to_string()))
    }

    pub fn get(&self, id: &str) -> CoreResult<Option<Profile>> {
        let conn = self.conn.lock().expect("profile store db mutex poisoned");
        conn.query_row(
            "SELECT id, display_name, avatar_url, github_login, created_at FROM profiles WHERE id = ?1",
            params![id],
            row_to_profile,
        )
        .optional()
        .map_err(|e| CoreError::History(e.to_string()))
    }

    /// The signed-in profile, or `None` if `set_current` was never called
    /// (a true fresh install, before any sign-in) or the id it points at
    /// no longer exists.
    pub fn current(&self) -> CoreResult<Option<Profile>> {
        let id: Option<String> = {
            let conn = self.conn.lock().expect("profile store db mutex poisoned");
            conn.query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![CURRENT_PROFILE_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| CoreError::History(e.to_string()))?
        };
        match id {
            Some(id) => self.get(&id),
            None => Ok(None),
        }
    }

    pub fn set_current(&self, id: &str) -> CoreResult<()> {
        let conn = self.conn.lock().expect("profile store db mutex poisoned");
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![CURRENT_PROFILE_KEY, id],
        )
        .map_err(|e| CoreError::History(e.to_string()))?;
        Ok(())
    }

    /// Signs out without deleting the profile - `current()` goes back to
    /// `None` (the sign-in gate/picker shows again), but the profile and
    /// its data stay exactly as they were for the next sign-in.
    pub fn clear_current(&self) -> CoreResult<()> {
        let conn = self.conn.lock().expect("profile store db mutex poisoned");
        conn.execute(
            "DELETE FROM meta WHERE key = ?1",
            params![CURRENT_PROFILE_KEY],
        )
        .map_err(|e| CoreError::History(e.to_string()))?;
        Ok(())
    }

    /// Inserts a new profile, or updates an existing one with the same id
    /// (e.g. a display name/avatar changing on re-sign-in).
    pub fn upsert(&self, profile: &Profile) -> CoreResult<()> {
        let conn = self.conn.lock().expect("profile store db mutex poisoned");
        conn.execute(
            "INSERT INTO profiles (id, display_name, avatar_url, github_login, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                avatar_url = excluded.avatar_url,
                github_login = excluded.github_login",
            params![
                profile.id,
                profile.display_name,
                profile.avatar_url,
                profile.github_login,
                profile.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CoreError::History(e.to_string()))?;
        Ok(())
    }
}

fn row_to_profile(row: &rusqlite::Row) -> rusqlite::Result<Profile> {
    let created_at: String = row.get(4)?;
    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    Ok(Profile {
        id: row.get(0)?,
        display_name: row.get(1)?,
        avatar_url: row.get(2)?,
        github_login: row.get(3)?,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A fresh temp directory per test - `ProfileStore::open_at` takes the
    /// directory explicitly (rather than reading it from an env var
    /// `open()` resolves internally), so parallel tests never share any
    /// process-global state to race on.
    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "otto-profiles-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fresh_install_has_no_current_profile() {
        let store = ProfileStore::open_at(&temp_dir()).unwrap();
        assert!(store.current().unwrap().is_none());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn upgrade_from_pre_profiles_install_creates_local_profile() {
        // Simulate an existing pre-profiles install: just the old unscoped
        // otto.sqlite3 file, no profiles.sqlite3 yet.
        let dir = temp_dir();
        std::fs::write(dir.join("otto.sqlite3"), b"").unwrap();

        let store = ProfileStore::open_at(&dir).unwrap();
        let current = store
            .current()
            .unwrap()
            .expect("local profile should be current");
        assert_eq!(current.id, LOCAL_PROFILE_ID);
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn upsert_then_switch_profiles() {
        let store = ProfileStore::open_at(&temp_dir()).unwrap();
        let a = Profile {
            id: "octocat".into(),
            display_name: "The Octocat".into(),
            avatar_url: Some("https://example.com/a.png".into()),
            github_login: Some("octocat".into()),
            created_at: Utc::now(),
        };
        store.upsert(&a).unwrap();
        store.set_current("octocat").unwrap();

        assert_eq!(
            store.current().unwrap().unwrap().display_name,
            "The Octocat"
        );

        // Re-upsert with a changed display name - should update, not duplicate.
        let a2 = Profile {
            display_name: "Renamed".into(),
            ..a
        };
        store.upsert(&a2).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(
            store.get("octocat").unwrap().unwrap().display_name,
            "Renamed"
        );
    }

    #[test]
    fn clear_current_signs_out_without_deleting_the_profile() {
        let store = ProfileStore::open_at(&temp_dir()).unwrap();
        let a = Profile {
            id: "octocat".into(),
            display_name: "The Octocat".into(),
            avatar_url: None,
            github_login: Some("octocat".into()),
            created_at: Utc::now(),
        };
        store.upsert(&a).unwrap();
        store.set_current("octocat").unwrap();
        assert!(store.current().unwrap().is_some());

        store.clear_current().unwrap();
        assert!(store.current().unwrap().is_none());
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(store.get("octocat").unwrap().is_some());
    }
}
