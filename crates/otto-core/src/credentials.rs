use async_trait::async_trait;

use crate::error::{CoreError, CoreResult};

/// Secret storage for the GitHub token, Claude API key, and calendar OAuth
/// credentials. `KeyringStore` is the default (OS keychain via the
/// `keyring` crate); nothing is ever written to a plaintext config file.
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn get(&self, key: &str) -> CoreResult<String>;
    async fn set(&self, key: &str, value: &str) -> CoreResult<()>;
}

const SERVICE: &str = "otto-assistant";

/// OS-keychain-backed store. On platforms/sessions with no keychain daemon
/// available (e.g. a headless WSL box with no Secret Service running), a
/// keychain miss falls back to reading `OTTO_<KEY>` from the process
/// environment for local development - still never touching disk.
pub struct KeyringStore;

impl KeyringStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

fn env_key(key: &str) -> String {
    format!("OTTO_{}", key.to_uppercase().replace('-', "_"))
}

#[async_trait]
impl CredentialStore for KeyringStore {
    async fn get(&self, key: &str) -> CoreResult<String> {
        let key_owned = key.to_string();
        let keyring_result = tokio::task::spawn_blocking(move || {
            keyring::Entry::new(SERVICE, &key_owned).and_then(|entry| entry.get_password())
        })
        .await
        .map_err(|e| CoreError::Credential(e.to_string()))?;

        match keyring_result {
            Ok(secret) => Ok(secret),
            Err(_) => std::env::var(env_key(key)).map_err(|_| CoreError::CredentialNotFound(key.to_string())),
        }
    }

    async fn set(&self, key: &str, value: &str) -> CoreResult<()> {
        let key_owned = key.to_string();
        let value_owned = value.to_string();
        tokio::task::spawn_blocking(move || {
            keyring::Entry::new(SERVICE, &key_owned).and_then(|entry| entry.set_password(&value_owned))
        })
        .await
        .map_err(|e| CoreError::Credential(e.to_string()))?
        .map_err(|e| CoreError::Credential(e.to_string()))
    }
}
