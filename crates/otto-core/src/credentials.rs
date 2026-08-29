use async_trait::async_trait;

use crate::error::{CoreError, CoreResult};
use crate::profiles::LOCAL_PROFILE_ID;

/// Secret storage for the GitHub token, Claude API key, and calendar OAuth
/// credentials. `KeyringStore` is the default (OS keychain via the
/// `keyring` crate); nothing is ever written to a plaintext config file.
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn get(&self, key: &str) -> CoreResult<String>;
    async fn set(&self, key: &str, value: &str) -> CoreResult<()>;
}

const BASE_SERVICE: &str = "otto-assistant";

/// OS-keychain-backed store, one per profile - `service` is
/// `"otto-assistant"` unsuffixed for [`LOCAL_PROFILE_ID`] (the
/// auto-migrated pre-profiles install, so existing credentials keep
/// working with zero re-entry) and `"otto-assistant-{profile_id}"` for
/// every other profile, keeping each profile's secrets in a genuinely
/// separate OS keychain entry. On platforms/sessions with no keychain
/// daemon available (e.g. a headless WSL box with no Secret Service
/// running), a keychain miss falls back to reading `OTTO_<KEY>` from the
/// process environment for local development - still never touching disk,
/// and deliberately *not* profile-scoped (a single process only ever acts
/// as one profile at a time, so there's nothing to disambiguate).
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn for_profile(profile_id: &str) -> Self {
        let service = if profile_id == LOCAL_PROFILE_ID {
            BASE_SERVICE.to_string()
        } else {
            format!("{BASE_SERVICE}-{profile_id}")
        };
        Self { service }
    }
}

fn env_key(key: &str) -> String {
    format!("OTTO_{}", key.to_uppercase().replace('-', "_"))
}

#[async_trait]
impl CredentialStore for KeyringStore {
    async fn get(&self, key: &str) -> CoreResult<String> {
        let service = self.service.clone();
        let key_owned = key.to_string();
        let keyring_result = tokio::task::spawn_blocking(move || {
            keyring::Entry::new(&service, &key_owned).and_then(|entry| entry.get_password())
        })
        .await
        .map_err(|e| CoreError::Credential(e.to_string()))?;

        match keyring_result {
            Ok(secret) => Ok(secret),
            Err(_) => std::env::var(env_key(key))
                .map_err(|_| CoreError::CredentialNotFound(key.to_string())),
        }
    }

    async fn set(&self, key: &str, value: &str) -> CoreResult<()> {
        let service = self.service.clone();
        let key_owned = key.to_string();
        let value_owned = value.to_string();
        tokio::task::spawn_blocking(move || {
            keyring::Entry::new(&service, &key_owned)
                .and_then(|entry| entry.set_password(&value_owned))
        })
        .await
        .map_err(|e| CoreError::Credential(e.to_string()))?
        .map_err(|e| CoreError::Credential(e.to_string()))
    }
}
