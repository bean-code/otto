//! GitHub sign-in: the OAuth Device Flow (no client secret, no local
//! redirect listener - just a code the user enters at github.com/login/device,
//! polled until they do) plus identity lookup, used by both `otto-ui` and
//! `otto-cli` to authenticate a `otto_core::Profile`. Deliberately built on
//! plain `reqwest`, not `octocrab`: these are `github.com` OAuth endpoints,
//! not the `api.github.com` REST surface `octocrab` targets, and
//! `fetch_identity` runs *before* any token exists for `octocrab` to
//! authenticate with.
//!
//! Verified against GitHub's own docs, not assumed: both endpoints need
//! `Accept: application/json` (they default to form-encoded), no
//! `client_secret` for the device flow specifically, and Device Flow must
//! be enabled on the OAuth App's own settings page before any of this
//! works - see the README for the setup steps.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
/// `repo` for the PR/CI tools `GithubSkill` already has; `workflow` for
/// `DeploySkill`'s Actions trigger/status tools - one sign-in covers both,
/// since it's the same credential either skill reads.
const SCOPE: &str = "repo workflow";

/// What `device_flow_start` gets back - `user_code`/`verification_uri` are
/// what to show the user; `device_code`/`interval`/`expires_in` are what
/// `device_flow_poll` needs, kept internal to the flow rather than
/// re-displayed.
#[derive(Debug, Clone)]
pub struct DeviceFlowSession {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize, Default)]
struct TokenPollResponse {
    access_token: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubIdentity {
    pub login: String,
    pub id: i64,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Step 1 - requests a device/user code pair. Show `user_code` and
/// `verification_uri` to the user immediately, then call
/// [`device_flow_poll`] with the rest.
pub async fn device_flow_start(client_id: &str) -> Result<DeviceFlowSession> {
    let http = reqwest::Client::new();
    let resp = http
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", SCOPE)])
        .send()
        .await
        .context("failed to reach GitHub")?
        .error_for_status()
        .context("GitHub rejected the device code request - check the client id")?
        .json::<DeviceCodeResponse>()
        .await
        .context("unexpected response from GitHub")?;

    Ok(DeviceFlowSession {
        user_code: resp.user_code,
        verification_uri: resp.verification_uri,
        device_code: resp.device_code,
        interval: resp.interval,
        expires_in: resp.expires_in,
    })
}

/// Step 2 - polls until the user finishes entering the code (or it expires
/// / they cancel). Deliberately doesn't call `.error_for_status()`: GitHub
/// returns normal 200s with an `error` field in the body for
/// `authorization_pending`/`slow_down`, not HTTP error statuses.
pub async fn device_flow_poll(client_id: &str, session: &DeviceFlowSession) -> Result<String> {
    let http = reqwest::Client::new();
    let mut interval = Duration::from_secs(session.interval.max(5));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(session.expires_in + 5);

    loop {
        if tokio::time::Instant::now() >= deadline {
            bail!("the sign-in code expired - try again");
        }
        tokio::time::sleep(interval).await;

        let resp = http
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", session.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .context("failed to reach GitHub")?
            .json::<TokenPollResponse>()
            .await
            .context("unexpected response from GitHub")?;

        if let Some(token) = resp.access_token {
            return Ok(token);
        }
        match resp.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += Duration::from_secs(5);
                continue;
            }
            Some("expired_token") => bail!("the sign-in code expired - try again"),
            Some("access_denied") => bail!("sign-in was cancelled"),
            Some(other) => bail!("GitHub sign-in failed: {other}"),
            None => bail!("unexpected response from GitHub during sign-in"),
        }
    }
}

/// Works with any valid GitHub token regardless of scope - identity isn't
/// scope-gated, only writes/private data are. Used both right after a
/// device-flow sign-in and to validate+identify a pasted PAT in one step.
pub async fn fetch_identity(token: &str) -> Result<GithubIdentity> {
    let http = reqwest::Client::new();
    http.get("https://api.github.com/user")
        .header("Accept", "application/vnd.github+json")
        // GitHub's API rejects requests with no User-Agent outright.
        .header("User-Agent", "otto-assistant")
        .bearer_auth(token)
        .send()
        .await
        .context("failed to reach GitHub")?
        .error_for_status()
        .context("GitHub rejected the token")?
        .json::<GithubIdentity>()
        .await
        .context("unexpected response from GitHub")
}
