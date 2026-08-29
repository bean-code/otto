use std::sync::Arc;
use std::time::Duration as StdDuration;

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use otto_core::{CoreError, CoreResult, CredentialStore, Skill, SkillOutcome, ToolSpec};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const CLIENT_ID_KEY: &str = "google-calendar-client-id";
const OAUTH_KEY: &str = "google-calendar-oauth";

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
// Read-only, events-only - the narrowest scope that covers both tools below.
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events.readonly";

/// What's actually persisted in the OS keychain under `OAUTH_KEY`, as JSON.
/// Google's refresh response doesn't return a new `refresh_token`, so it's
/// carried forward unchanged across refreshes.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTokens {
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct EventsResponse {
    #[serde(default)]
    items: Vec<GEvent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GEvent {
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    html_link: Option<String>,
    start: GEventTime,
    end: GEventTime,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GEventTime {
    #[serde(default)]
    date_time: Option<String>,
    #[serde(default)]
    date: Option<String>,
}

impl GEventTime {
    /// The display string for this side of the event - a full timestamp for
    /// timed events, just the date for all-day ones.
    fn display(&self) -> &str {
        self.date_time
            .as_deref()
            .or(self.date.as_deref())
            .unwrap_or("?")
    }

    /// Only timed events (not all-day) participate in conflict detection -
    /// "Company Holiday" isn't a double-booking in the usual sense, and
    /// giving all-day events a timezone-aware span is out of scope here.
    fn as_datetime(&self) -> Option<DateTime<Utc>> {
        self.date_time
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }
}

/// Meeting/calendar integration, against Google Calendar via OAuth2 (PKCE,
/// no client secret - see `connect_google_calendar`) + Calendar API v3.
/// Read-only: lists upcoming events and flags overlapping ones. No
/// persistent HTTP client is held (unlike `GithubSkill`'s `octocrab`
/// client) since there's no SDK object here, just per-request `reqwest`
/// calls.
pub struct CalendarSkill {
    credentials: Arc<dyn CredentialStore>,
}

impl CalendarSkill {
    pub fn new(credentials: Arc<dyn CredentialStore>) -> Self {
        Self { credentials }
    }

    fn failed(&self, message: impl std::fmt::Display) -> CoreError {
        CoreError::SkillFailed {
            skill: self.id().to_string(),
            message: message.to_string(),
        }
    }

    /// A valid (refreshed if necessary) access token, or a clear "not
    /// connected" error rather than a raw HTTP failure.
    async fn access_token(&self) -> CoreResult<String> {
        let raw = self.credentials.get(OAUTH_KEY).await.map_err(|_| {
            self.failed("Google Calendar isn't connected - open Settings and connect it")
        })?;
        let stored: StoredTokens = serde_json::from_str(&raw)
            .map_err(|e| self.failed(format!("stored Google Calendar tokens are corrupt: {e}")))?;

        if stored.expires_at > Utc::now() + Duration::seconds(60) {
            return Ok(stored.access_token);
        }

        let client_id = self.credentials.get(CLIENT_ID_KEY).await.map_err(|_| {
            self.failed("Google Calendar client id is missing - reconnect it in Settings")
        })?;

        let http = reqwest::Client::new();
        let response = http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", stored.refresh_token.as_str()),
            ])
            .send()
            .await
            .map_err(|e| self.failed(format!("failed to refresh Google Calendar token: {e}")))?
            .error_for_status()
            .map_err(|e| {
                self.failed(format!(
                    "Google rejected the refresh - reconnect in Settings: {e}"
                ))
            })?
            .json::<TokenResponse>()
            .await
            .map_err(|e| self.failed(format!("unexpected refresh response from Google: {e}")))?;

        let refreshed = StoredTokens {
            access_token: response.access_token.clone(),
            refresh_token: stored.refresh_token,
            expires_at: Utc::now() + Duration::seconds(response.expires_in),
        };
        let blob = serde_json::to_string(&refreshed).map_err(|e| self.failed(e))?;
        self.credentials.set(OAUTH_KEY, &blob).await?;

        Ok(refreshed.access_token)
    }

    fn hours_ahead(input: &Value) -> i64 {
        input
            .get("hours_ahead")
            .and_then(Value::as_i64)
            .unwrap_or(24)
    }

    async fn fetch_events(&self, hours_ahead: i64) -> CoreResult<Vec<GEvent>> {
        let access_token = self.access_token().await?;
        let time_min = Utc::now().to_rfc3339();
        let time_max = (Utc::now() + Duration::hours(hours_ahead)).to_rfc3339();

        let url = Url::parse_with_params(
            "https://www.googleapis.com/calendar/v3/calendars/primary/events",
            &[
                ("timeMin", time_min.as_str()),
                ("timeMax", time_max.as_str()),
                ("singleEvents", "true"),
                ("orderBy", "startTime"),
                ("maxResults", "50"),
            ],
        )
        .map_err(|e| self.failed(e))?;

        let http = reqwest::Client::new();
        let response = http
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| self.failed(format!("failed to reach Google Calendar: {e}")))?
            .error_for_status()
            .map_err(|e| self.failed(e))?
            .json::<EventsResponse>()
            .await
            .map_err(|e| self.failed(format!("unexpected response from Google Calendar: {e}")))?;

        Ok(response.items)
    }

    async fn upcoming(&self, input: Value) -> CoreResult<SkillOutcome> {
        let hours_ahead = Self::hours_ahead(&input);
        let events = self.fetch_events(hours_ahead).await?;

        let items: Vec<Value> = events
            .iter()
            .map(|e| {
                json!({
                    "summary": e.summary,
                    "start": e.start.display(),
                    "end": e.end.display(),
                    "url": e.html_link,
                })
            })
            .collect();

        let summary = if events.is_empty() {
            format!("No meetings in the next {hours_ahead}h.")
        } else {
            let lines: Vec<String> = events
                .iter()
                .map(|e| {
                    format!(
                        "{} - {} ({})",
                        e.start.display(),
                        e.summary.as_deref().unwrap_or("(untitled)"),
                        e.end.display()
                    )
                })
                .collect();
            format!(
                "{} meeting(s) in the next {hours_ahead}h:\n{}",
                events.len(),
                lines.join("\n")
            )
        };

        Ok(SkillOutcome::new(
            summary,
            json!({ "hours_ahead": hours_ahead, "events": items }),
        ))
    }

    async fn conflicts(&self, input: Value) -> CoreResult<SkillOutcome> {
        let hours_ahead = Self::hours_ahead(&input);
        let events = self.fetch_events(hours_ahead).await?;
        let conflicts = find_conflicts(&events);

        let summary = if conflicts.is_empty() {
            format!("No overlapping meetings in the next {hours_ahead}h.")
        } else {
            let lines: Vec<String> = conflicts
                .iter()
                .map(|c| {
                    format!(
                        "\"{}\" overlaps \"{}\"",
                        c["a"].as_str().unwrap_or("?"),
                        c["b"].as_str().unwrap_or("?")
                    )
                })
                .collect();
            format!(
                "{} overlapping pair(s) in the next {hours_ahead}h:\n{}",
                conflicts.len(),
                lines.join("\n")
            )
        };

        Ok(SkillOutcome::new(
            summary,
            json!({ "hours_ahead": hours_ahead, "conflicts": conflicts }),
        ))
    }
}

#[async_trait]
impl Skill for CalendarSkill {
    fn id(&self) -> &'static str {
        "calendar"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec::new(
                "calendar_upcoming",
                "List upcoming meetings on the user's calendar.",
                json!({
                    "type": "object",
                    "properties": {
                        "hours_ahead": {
                            "type": "integer",
                            "description": "How many hours ahead to look. Defaults to 24."
                        }
                    },
                    "required": [],
                    "additionalProperties": false
                }),
            ),
            ToolSpec::new(
                "calendar_conflicts",
                "Flag any overlapping/conflicting meetings in the given window.",
                json!({
                    "type": "object",
                    "properties": {
                        "hours_ahead": {
                            "type": "integer",
                            "description": "How many hours ahead to look. Defaults to 24."
                        }
                    },
                    "required": [],
                    "additionalProperties": false
                }),
            ),
        ]
    }

    async fn call(&self, tool_name: &str, input: Value) -> CoreResult<SkillOutcome> {
        match tool_name {
            "calendar_upcoming" => self.upcoming(input).await,
            "calendar_conflicts" => self.conflicts(input).await,
            other => Err(CoreError::SkillNotFound(other.to_string())),
        }
    }
}

/// Pairwise overlap check over the timed (non-all-day) events in `events` -
/// pulled out of `CalendarSkill::conflicts` so it's testable without a
/// network round trip.
fn find_conflicts(events: &[GEvent]) -> Vec<Value> {
    let timed: Vec<(&GEvent, DateTime<Utc>, DateTime<Utc>)> = events
        .iter()
        .filter_map(|e| Some((e, e.start.as_datetime()?, e.end.as_datetime()?)))
        .collect();

    let mut conflicts = Vec::new();
    for i in 0..timed.len() {
        for j in (i + 1)..timed.len() {
            let (a, a_start, a_end) = &timed[i];
            let (b, b_start, b_end) = &timed[j];
            if a_start < b_end && b_start < a_end {
                conflicts.push(json!({
                    "a": a.summary.as_deref().unwrap_or("(untitled)"),
                    "a_start": a.start.display(),
                    "b": b.summary.as_deref().unwrap_or("(untitled)"),
                    "b_start": b.start.display(),
                }));
            }
        }
    }
    conflicts
}

fn random_url_safe(num_bytes: usize) -> String {
    let mut bytes = vec![0u8; num_bytes];
    rand::fill(&mut bytes[..]);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Runs the whole PKCE authorization-code flow against Google and stores the
/// resulting tokens: generates a verifier/challenge/state, binds a loopback
/// listener on an OS-assigned port, opens the consent screen in the user's
/// default browser, waits for the one redirect it produces, exchanges the
/// code, and persists the tokens. Not a `CalendarSkill` method - it's
/// invoked once from a "Connect" action (a Tauri command today), not as
/// part of the normal `Skill::call` dispatch path.
pub async fn connect_google_calendar(credentials: &dyn CredentialStore) -> CoreResult<()> {
    let client_id = credentials
        .get(CLIENT_ID_KEY)
        .await
        .map_err(|_| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: "add a Google Calendar Client ID in Settings first".into(),
        })?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: format!("couldn't open a local port for the OAuth redirect: {e}"),
        })?;
    let port = listener
        .local_addr()
        .map_err(|e| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: e.to_string(),
        })?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let verifier = random_url_safe(32);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_url_safe(16);

    let auth_url = Url::parse_with_params(
        AUTH_URL,
        &[
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", state.as_str()),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ],
    )
    .map_err(|e| CoreError::SkillFailed {
        skill: "calendar".into(),
        message: e.to_string(),
    })?;

    webbrowser::open(auth_url.as_str()).map_err(|e| CoreError::SkillFailed {
        skill: "calendar".into(),
        message: format!("couldn't open a browser: {e}"),
    })?;

    let code = tokio::time::timeout(
        StdDuration::from_secs(300),
        wait_for_redirect(&listener, &state),
    )
    .await
    .map_err(|_| CoreError::SkillFailed {
        skill: "calendar".into(),
        message: "timed out waiting for Google sign-in".into(),
    })??;

    let http = reqwest::Client::new();
    let response = http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: format!("token exchange failed: {e}"),
        })?
        .error_for_status()
        .map_err(|e| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: format!("Google rejected the token exchange: {e}"),
        })?
        .json::<TokenResponse>()
        .await
        .map_err(|e| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: format!("unexpected token response: {e}"),
        })?;

    let refresh_token = response.refresh_token.ok_or_else(|| CoreError::SkillFailed {
        skill: "calendar".into(),
        message: "Google didn't return a refresh token - revoke Otto's access at myaccount.google.com/permissions and reconnect".into(),
    })?;

    let stored = StoredTokens {
        access_token: response.access_token,
        refresh_token,
        expires_at: Utc::now() + Duration::seconds(response.expires_in),
    };
    let blob = serde_json::to_string(&stored).map_err(|e| CoreError::SkillFailed {
        skill: "calendar".into(),
        message: e.to_string(),
    })?;
    credentials.set(OAUTH_KEY, &blob).await?;

    Ok(())
}

/// Accepts the single redirect Google's consent screen sends back, reads
/// just its request line (headers/body are irrelevant here), and returns
/// the authorization code - after checking `state` matches, as a minimal
/// CSRF guard.
async fn wait_for_redirect(listener: &TcpListener, expected_state: &str) -> CoreResult<String> {
    let (stream, _) = listener
        .accept()
        .await
        .map_err(|e| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: e.to_string(),
        })?;
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .map_err(|e| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: e.to_string(),
        })?;

    // "GET /callback?code=...&state=... HTTP/1.1"
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let full_url =
        Url::parse(&format!("http://127.0.0.1{path}")).map_err(|e| CoreError::SkillFailed {
            skill: "calendar".into(),
            message: e.to_string(),
        })?;
    let params: std::collections::HashMap<_, _> = full_url.query_pairs().into_owned().collect();

    let body = if params.contains_key("code")
        && params.get("state").map(String::as_str) == Some(expected_state)
    {
        "<html><body>Connected - you can close this tab and go back to Otto.</body></html>"
    } else {
        "<html><body>Something went wrong - go back to Otto and try connecting again.</body></html>"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = reader.into_inner();
    let _ = stream.write_all(response.as_bytes()).await;

    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Err(CoreError::SkillFailed {
            skill: "calendar".into(),
            message: "OAuth state mismatch - try connecting again".into(),
        });
    }

    params.get("code").cloned().ok_or_else(|| CoreError::SkillFailed {
        skill: "calendar".into(),
        message: "Google's redirect didn't include an authorization code (sign-in may have been cancelled)".into(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::Mutex;

    use super::*;

    struct MapCredentialStore(Mutex<HashMap<String, String>>);

    impl MapCredentialStore {
        fn empty() -> Self {
            Self(Mutex::new(HashMap::new()))
        }
    }

    #[async_trait]
    impl CredentialStore for MapCredentialStore {
        async fn get(&self, key: &str) -> CoreResult<String> {
            self.0
                .lock()
                .await
                .get(key)
                .cloned()
                .ok_or_else(|| CoreError::CredentialNotFound(key.to_string()))
        }

        async fn set(&self, key: &str, value: &str) -> CoreResult<()> {
            self.0
                .lock()
                .await
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
    }

    fn timed_event(summary: &str, start: &str, end: &str) -> GEvent {
        GEvent {
            summary: Some(summary.to_string()),
            html_link: None,
            start: GEventTime {
                date_time: Some(start.to_string()),
                date: None,
            },
            end: GEventTime {
                date_time: Some(end.to_string()),
                date: None,
            },
        }
    }

    fn all_day_event(summary: &str, date: &str) -> GEvent {
        GEvent {
            summary: Some(summary.to_string()),
            html_link: None,
            start: GEventTime {
                date_time: None,
                date: Some(date.to_string()),
            },
            end: GEventTime {
                date_time: None,
                date: Some(date.to_string()),
            },
        }
    }

    #[tokio::test]
    async fn access_token_reports_not_connected_clearly() {
        let skill = CalendarSkill::new(Arc::new(MapCredentialStore::empty()));
        let err = skill.access_token().await.unwrap_err().to_string();
        assert!(err.contains("isn't connected"), "unexpected error: {err}");
    }

    #[test]
    fn find_conflicts_flags_overlapping_timed_events() {
        let events = vec![
            timed_event("Standup", "2026-08-29T09:00:00Z", "2026-08-29T09:15:00Z"),
            timed_event("1:1", "2026-08-29T09:10:00Z", "2026-08-29T09:30:00Z"),
            timed_event("Lunch", "2026-08-29T12:00:00Z", "2026-08-29T13:00:00Z"),
        ];

        let conflicts = find_conflicts(&events);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0]["a"], "Standup");
        assert_eq!(conflicts[0]["b"], "1:1");
    }

    #[test]
    fn find_conflicts_ignores_back_to_back_and_all_day_events() {
        let events = vec![
            timed_event("Standup", "2026-08-29T09:00:00Z", "2026-08-29T09:15:00Z"),
            // Starts exactly when Standup ends - adjacent, not overlapping.
            timed_event("Planning", "2026-08-29T09:15:00Z", "2026-08-29T09:45:00Z"),
            all_day_event("Company Holiday", "2026-08-29"),
        ];

        assert!(find_conflicts(&events).is_empty());
    }
}
