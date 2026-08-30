use std::path::PathBuf;
use std::sync::Arc;

use llmprism::providers::anthropic::AnthropicProvider;
use llmprism::providers::gemini::GeminiProvider;
use llmprism::providers::ollama::OllamaProvider;
use llmprism::providers::openai::OpenAiProvider;
use llmprism::providers::openai_compatible::{
    DeepSeekProvider, GroqProvider, MistralProvider, OpenRouterProvider, PerplexityProvider,
    XaiProvider, ZaiProvider,
};
use llmprism::Registry as LlmRegistry;
use otto_core::{
    credential_key, ActiveWindow, ActivityEntry, ActivityKind, ActivityLog, AgentLoop, AgentTurn,
    CoreError, CredentialStore, KeyringStore, ModelInfo, Profile, ProfileStore, ProviderInfo,
    SkillInfo, SkillRegistry, SqliteStore, DEFAULT_PERSONA, LOCAL_PROFILE_ID, MODEL_CATALOG,
    PROVIDER_CATALOG,
};
use otto_skill_calendar::{connect_google_calendar, CalendarSkill};
use otto_skill_deploy::DeploySkill;
use otto_skill_github::{device_flow_poll, device_flow_start, fetch_identity, GithubSkill};
use otto_skill_notify::NotifySkill;
use otto_voice::VoiceEngine;
use serde::{Deserialize, Serialize};
use tauri::menu::{AboutMetadataBuilder, MenuBuilder, SubmenuBuilder};
use tauri::Emitter;
use tokio::sync::RwLock;

const DEFAULT_PROVIDER: &str = "anthropic";
const DEFAULT_MODEL: &str = "claude-opus-5";
const GITHUB_KEY: &str = "github-token";
const GOOGLE_CALENDAR_CLIENT_ID_KEY: &str = "google-calendar-client-id";
const GOOGLE_CALENDAR_OAUTH_KEY: &str = "google-calendar-oauth";
const MODEL_PROVIDER_SETTING_KEY: &str = "model_provider";
const MODEL_ID_SETTING_KEY: &str = "model_id";
const PERSONA_SETTING_KEY: &str = "persona_override";
const TALK_BACK_DEFAULT_KEY: &str = "talk_back_default";
const NOTIFICATIONS_ENABLED_KEY: &str = "notifications_enabled";

/// The parts of Otto's runtime that depend on credentials, held behind a
/// lock so a Settings save can swap in a freshly-built one without
/// restarting the app. Rebuilt as a whole rather than patched in place:
/// `GithubSkill` resolves its token once (via a `OnceCell`) on first use,
/// so picking up a changed token means a new `GithubSkill`, which means a
/// new `SkillRegistry` - simplest to just rebuild both together.
struct SharedRuntime {
    registry: Arc<SkillRegistry>,
    llm: LlmRegistry,
}

/// Which provider + model is currently active. A model belongs to exactly
/// one provider, so these travel together rather than as two independent
/// settings that could drift out of sync.
#[derive(Debug, Clone, Serialize)]
struct SelectedModel {
    provider: String,
    model: String,
}

/// Otto's credentials/activity-log/persona/notifications are per profile -
/// this is the bundle that changes as a whole on sign-in or profile switch.
/// `runtime`/`selected` were already behind their own `RwLock`s (a Settings
/// save rebuilds `runtime` without restarting); the rest join them here
/// rather than living as one shared lock, so an unrelated read (e.g.
/// `list_skills` reading `runtime`) never blocks on a profile switch still
/// in progress or vice versa.
struct AppState {
    /// The top-level profile registry - always unscoped, has to be
    /// readable before any profile is "entered".
    profile_store: Arc<ProfileStore>,
    /// The GitHub OAuth App's client id, for the Device Flow - `None` means
    /// sign-in falls back to "paste a token" only (see `start_github_device_flow`).
    github_client_id: Option<String>,
    credentials: RwLock<Arc<dyn CredentialStore>>,
    runtime: RwLock<SharedRuntime>,
    selected: RwLock<SelectedModel>,
    persona: RwLock<String>,
    notifications_enabled: RwLock<bool>,
    store: RwLock<Arc<SqliteStore>>,
    voice: Option<VoiceEngine>,
}

impl AppState {
    /// Builds a fresh `AgentLoop` from the current runtime snapshot.
    /// Cheap - `AgentLoop` just holds clones of `Arc`s and small strings, so
    /// there's no need to keep a long-lived instance around and worry about
    /// it going stale after a credential or model change.
    async fn agent(&self) -> AgentLoop {
        let rt = self.runtime.read().await;
        let selected = self.selected.read().await.clone();
        let persona = self.persona.read().await.clone();
        let store = self.store.read().await.clone();
        AgentLoop::new(
            rt.llm.clone(),
            selected.provider,
            selected.model,
            persona,
            rt.registry.clone(),
            store,
        )
    }

    async fn rebuild_runtime(&self) -> anyhow::Result<()> {
        let credentials = self.credentials.read().await.clone();
        let notifications_enabled = *self.notifications_enabled.read().await;
        let fresh = build_runtime(credentials, notifications_enabled).await?;
        *self.runtime.write().await = fresh;
        Ok(())
    }

    /// Swaps every profile-scoped piece of state to `profile_id` and makes
    /// it current in the registry - used both when switching between
    /// already-known profiles and right after a fresh sign-in.
    async fn switch_to_profile(&self, profile_id: &str) -> anyhow::Result<()> {
        let scoped = load_profile_scoped(profile_id).await?;
        *self.credentials.write().await = scoped.credentials;
        *self.store.write().await = scoped.store;
        *self.selected.write().await = scoped.selected;
        *self.persona.write().await = scoped.persona;
        *self.notifications_enabled.write().await = scoped.notifications_enabled;
        *self.runtime.write().await = scoped.runtime;
        self.profile_store.set_current(profile_id)?;
        Ok(())
    }
}

/// Where a profile's `SqliteStore` lives - `LOCAL_PROFILE_ID` (the
/// auto-migrated pre-profiles install) resolves to the unchanged, unscoped
/// `default_path()` so upgrading needs no data migration; every other
/// profile gets its own `profiles/{id}/otto.sqlite3`.
fn store_path_for(profile_id: &str) -> otto_core::CoreResult<PathBuf> {
    if profile_id == LOCAL_PROFILE_ID {
        SqliteStore::default_path()
    } else {
        SqliteStore::path_for_profile(profile_id)
    }
}

/// Every piece of state that's scoped to one signed-in profile, loaded
/// together as a unit - used both to build `AppState` at startup and to
/// swap profiles afterward (`AppState::switch_to_profile`).
struct ProfileScoped {
    credentials: Arc<dyn CredentialStore>,
    store: Arc<SqliteStore>,
    runtime: SharedRuntime,
    selected: SelectedModel,
    persona: String,
    notifications_enabled: bool,
}

async fn load_profile_scoped(profile_id: &str) -> anyhow::Result<ProfileScoped> {
    let credentials: Arc<dyn CredentialStore> = Arc::new(KeyringStore::for_profile(profile_id));
    let store = Arc::new(SqliteStore::open(store_path_for(profile_id)?)?);

    // A model/provider picked via Settings outlives the process it was
    // picked in; OTTO_MODEL/the built-in default are only the first-run
    // fallback (and always assumed Anthropic, for back-compat with the
    // single-provider OTTO_MODEL env var from before multi-provider
    // support).
    let selected = match (
        store.get_setting(MODEL_PROVIDER_SETTING_KEY).await?,
        store.get_setting(MODEL_ID_SETTING_KEY).await?,
    ) {
        (Some(provider), Some(model)) => SelectedModel { provider, model },
        _ => SelectedModel {
            provider: DEFAULT_PROVIDER.to_string(),
            model: std::env::var("OTTO_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
        },
    };
    let persona = match store.get_setting(PERSONA_SETTING_KEY).await? {
        Some(persona) => persona,
        None => std::env::var("OTTO_PERSONA").unwrap_or_else(|_| DEFAULT_PERSONA.to_string()),
    };
    let notifications_enabled = store
        .get_setting(NOTIFICATIONS_ENABLED_KEY)
        .await?
        .map(|v| v == "true")
        .unwrap_or(true);

    let runtime = build_runtime(credentials.clone(), notifications_enabled).await?;

    Ok(ProfileScoped {
        credentials,
        store,
        runtime,
        selected,
        persona,
        notifications_enabled,
    })
}

#[derive(Serialize)]
struct CredentialStatus {
    key: String,
    label: String,
    configured: bool,
}

#[tauri::command]
async fn run_command(
    state: tauri::State<'_, Arc<AppState>>,
    input: String,
) -> Result<AgentTurn, String> {
    let selected = state.selected.read().await.clone();
    let credentials = state.credentials.read().await.clone();
    if let Some(info) = otto_core::provider_info(&selected.provider) {
        if info.needs_api_key && credentials.get(&credential_key(info.id)).await.is_err() {
            return Err(format!(
                "no {} API key configured yet - open Settings and add one",
                info.name
            ));
        }
    }
    state
        .agent()
        .await
        .run(&input)
        .await
        .map_err(|e| friendly_run_error(&selected, &e))
}

/// `run`'s LLM errors reach here already flattened to a string (see
/// `CoreError::Llm`), so a dead Ollama server surfaces as reqwest's raw
/// "Could not connect to localhost: Connection refused" - accurate but
/// meaningless to someone who never touched a socket. Ollama's the only
/// provider that can fail this way (every hosted provider already got
/// caught above by the missing-API-key check), so it's the only one worth
/// pattern-matching the message for.
fn friendly_run_error(selected: &SelectedModel, err: &CoreError) -> String {
    let message = err.to_string();
    if selected.provider == "ollama" && message.to_lowercase().contains("connect") {
        format!(
            "couldn't reach Ollama on localhost - is it running? Start it with `ollama serve`, \
             make sure '{}' is pulled (`ollama pull {}`), or pick a different model in Settings.",
            selected.model, selected.model
        )
    } else {
        message
    }
}

/// Most recent activity first (commands, growth-digest notes, schedule
/// events - one feed), for the Activities tab.
#[tauri::command]
async fn list_activities(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<u32>,
) -> Result<Vec<ActivityEntry>, String> {
    state
        .store
        .read()
        .await
        .list_recent(limit.unwrap_or(100) as usize)
        .await
        .map_err(|e| e.to_string())
}

/// Every provider Otto knows how to talk to, for Settings and the Models
/// tab's provider grouping.
#[tauri::command]
fn list_providers() -> Vec<ProviderInfo> {
    PROVIDER_CATALOG.to_vec()
}

/// The curated model picker's contents - a starting point per provider, not
/// exhaustive (see `otto_core::MODEL_CATALOG`'s own docs). The Models tab
/// also accepts a free-typed model id for any provider, for exactly that
/// reason.
#[tauri::command]
fn list_models() -> Vec<ModelInfo> {
    MODEL_CATALOG.to_vec()
}

#[tauri::command]
async fn get_selected_model(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SelectedModel, String> {
    Ok(state.selected.read().await.clone())
}

/// Switches the active provider+model immediately and persists the choice
/// so it survives a restart. `model` isn't restricted to `MODEL_CATALOG` -
/// any non-empty string is accepted, so a newly-released or self-hosted
/// (Ollama) model works without an otto-core update.
#[tauri::command]
async fn set_selected_model(
    state: tauri::State<'_, Arc<AppState>>,
    provider: String,
    model: String,
) -> Result<(), String> {
    if otto_core::provider_info(&provider).is_none() {
        return Err(format!("unknown provider '{provider}'"));
    }
    if model.trim().is_empty() {
        return Err("model id can't be empty".to_string());
    }
    let model = model.trim().to_string();

    let store = state.store.read().await.clone();
    store
        .set_setting(MODEL_PROVIDER_SETTING_KEY, &provider)
        .await
        .map_err(|e| e.to_string())?;
    store
        .set_setting(MODEL_ID_SETTING_KEY, &model)
        .await
        .map_err(|e| e.to_string())?;
    *state.selected.write().await = SelectedModel { provider, model };
    Ok(())
}

/// Every registered skill and the tools it exposes, for a "what can Otto
/// do" browser - see `docs/SKILLS.md` for how to add a new one.
#[tauri::command]
async fn list_skills(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<SkillInfo>, String> {
    Ok(state.runtime.read().await.registry.skills())
}

/// A first-name-ish greeting for the dashboard header, from the OS
/// account - never asked for, never stored; just whatever the OS already
/// knows. `None` if neither env var is set, which the frontend falls back
/// on gracefully.
#[tauri::command]
fn get_greeting_name() -> Option<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
}

/// Whether each known credential (every API-key provider, plus GitHub) is
/// currently set - never the value itself.
#[tauri::command]
async fn get_credential_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<CredentialStatus>, String> {
    let credentials = state.credentials.read().await.clone();
    let mut out = Vec::with_capacity(PROVIDER_CATALOG.len() + 1);
    for provider in PROVIDER_CATALOG.iter().filter(|p| p.needs_api_key) {
        let key = credential_key(provider.id);
        let configured = credentials.get(&key).await.is_ok();
        out.push(CredentialStatus {
            key,
            label: provider.name.to_string(),
            configured,
        });
    }
    out.push(CredentialStatus {
        key: GITHUB_KEY.to_string(),
        label: "GitHub".to_string(),
        configured: credentials.get(GITHUB_KEY).await.is_ok(),
    });
    out.push(CredentialStatus {
        key: GOOGLE_CALENDAR_CLIENT_ID_KEY.to_string(),
        label: "Google Calendar Client ID".to_string(),
        configured: credentials.get(GOOGLE_CALENDAR_CLIENT_ID_KEY).await.is_ok(),
    });
    out.push(CredentialStatus {
        key: GOOGLE_CALENDAR_OAUTH_KEY.to_string(),
        label: "Google Calendar".to_string(),
        configured: credentials.get(GOOGLE_CALENDAR_OAUTH_KEY).await.is_ok(),
    });
    Ok(out)
}

/// Saves a credential to the OS keychain and rebuilds the runtime so it
/// takes effect immediately, without restarting the app. Deliberately
/// excludes `GOOGLE_CALENDAR_OAUTH_KEY` - that one is only ever written by
/// `connect_google_calendar`'s own OAuth flow, never pasted.
#[tauri::command]
async fn set_credential(
    state: tauri::State<'_, Arc<AppState>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let is_known_provider_key = PROVIDER_CATALOG
        .iter()
        .any(|p| p.needs_api_key && credential_key(p.id) == key);
    let is_known_key =
        key == GITHUB_KEY || key == GOOGLE_CALENDAR_CLIENT_ID_KEY || is_known_provider_key;
    if !is_known_key {
        return Err(format!("unknown credential '{key}'"));
    }
    if value.trim().is_empty() {
        return Err("value can't be empty".to_string());
    }
    state
        .credentials
        .read()
        .await
        .set(&key, value.trim())
        .await
        .map_err(|e| e.to_string())?;
    state.rebuild_runtime().await.map_err(|e| e.to_string())
}

/// Runs the Google Calendar OAuth2/PKCE connect flow (opens the consent
/// screen in the user's default browser, waits for the redirect, exchanges
/// the code) and rebuilds the runtime so the connection is live immediately.
/// Requires `GOOGLE_CALENDAR_CLIENT_ID_KEY` to already be set.
#[tauri::command]
async fn connect_google_calendar_cmd(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let credentials = state.credentials.read().await.clone();
    connect_google_calendar(&*credentials)
        .await
        .map_err(|e| e.to_string())?;
    state.rebuild_runtime().await.map_err(|e| e.to_string())
}

/// Every known profile (the auto-migrated `"local"` one, plus every account
/// that's ever signed in), for the Sidebar's switch-profile menu.
#[tauri::command]
async fn list_profiles(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<Profile>, String> {
    state.profile_store.list().map_err(|e| e.to_string())
}

/// The signed-in profile, or `None` before any sign-in - the frontend shows
/// the sign-in gate exactly when this is `None`.
#[tauri::command]
async fn get_current_profile(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<Profile>, String> {
    state.profile_store.current().map_err(|e| e.to_string())
}

/// What the `"device-code"` event carries - just enough for the frontend to
/// show the user where to go and what to type.
#[derive(Clone, Serialize)]
struct DeviceCodePayload {
    user_code: String,
    verification_uri: String,
}

/// Runs the GitHub Device Flow end to end: requests a code, emits it to the
/// frontend as soon as it's ready (a `"device-code"` event, same pattern as
/// the existing `"open-settings"` menu event) so the UI can show it while
/// this call is still blocked polling, then waits for the user to enter it
/// at github.com/login/device, fetches their identity, and signs in.
#[tauri::command]
async fn start_github_device_flow(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Profile, String> {
    let client_id = state.github_client_id.clone().ok_or(
        "GitHub sign-in isn't configured (no OAuth client id) - see the README, or paste a token instead",
    )?;

    let session = device_flow_start(&client_id)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        "device-code",
        DeviceCodePayload {
            user_code: session.user_code.clone(),
            verification_uri: session.verification_uri.clone(),
        },
    );
    // Best-effort - the frontend also shows the code/URI so the user can
    // navigate there manually if this fails (e.g. no default browser set).
    let _ = webbrowser::open(&session.verification_uri);

    let token = device_flow_poll(&client_id, &session)
        .await
        .map_err(|e| e.to_string())?;
    complete_github_sign_in(&state, &token).await
}

/// Signs in with a pasted personal access token instead of the device flow
/// - validates it and derives the profile in the same `fetch_identity` call.
#[tauri::command]
async fn sign_in_with_pat(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
) -> Result<Profile, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("token can't be empty".to_string());
    }
    complete_github_sign_in(&state, token).await
}

/// Shared tail of both sign-in paths: fetch identity, upsert the profile,
/// switch to it, then store the token under *that* profile's now-active
/// keychain namespace and rebuild the runtime so `GithubSkill`/`DeploySkill`
/// pick it up immediately.
async fn complete_github_sign_in(state: &Arc<AppState>, token: &str) -> Result<Profile, String> {
    let identity = fetch_identity(token).await.map_err(|e| e.to_string())?;
    let profile = Profile {
        id: identity.login.clone(),
        display_name: identity
            .name
            .clone()
            .unwrap_or_else(|| identity.login.clone()),
        avatar_url: identity.avatar_url.clone(),
        github_login: Some(identity.login.clone()),
        created_at: chrono::Utc::now(),
    };
    state
        .profile_store
        .upsert(&profile)
        .map_err(|e| e.to_string())?;
    state
        .switch_to_profile(&profile.id)
        .await
        .map_err(|e| e.to_string())?;

    state
        .credentials
        .read()
        .await
        .set(GITHUB_KEY, token)
        .await
        .map_err(|e| e.to_string())?;
    state.rebuild_runtime().await.map_err(|e| e.to_string())?;

    Ok(profile)
}

/// Switches to an already-known profile (picked from the Sidebar menu).
#[tauri::command]
async fn switch_profile(
    state: tauri::State<'_, Arc<AppState>>,
    profile_id: String,
) -> Result<(), String> {
    if state
        .profile_store
        .get(&profile_id)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err(format!("unknown profile '{profile_id}'"));
    }
    state
        .switch_to_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())
}

/// Signs out back to the sign-in gate/picker - the profile and its data
/// stay exactly as they are for the next sign-in, nothing is deleted.
#[tauri::command]
async fn sign_out(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .profile_store
        .clear_current()
        .map_err(|e| e.to_string())
}

/// The current profile's editable preferences - persona override,
/// talk-back-by-default, and whether `notify_desktop` actually shows
/// anything.
#[derive(Serialize, Deserialize)]
struct Preferences {
    persona: String,
    talk_back_default: bool,
    notifications_enabled: bool,
}

#[tauri::command]
async fn get_preferences(state: tauri::State<'_, Arc<AppState>>) -> Result<Preferences, String> {
    let store = state.store.read().await.clone();
    let persona = state.persona.read().await.clone();
    let talk_back_default = store
        .get_setting(TALK_BACK_DEFAULT_KEY)
        .await
        .map_err(|e| e.to_string())?
        .map(|v| v == "true")
        .unwrap_or(false);
    let notifications_enabled = *state.notifications_enabled.read().await;
    Ok(Preferences {
        persona,
        talk_back_default,
        notifications_enabled,
    })
}

/// Saves the current profile's preferences and rebuilds the runtime so a
/// changed notifications toggle takes effect immediately (persona and
/// talk-back are read fresh per use already, so they don't strictly need
/// the rebuild - but one rebuild for the whole save keeps this simple).
#[tauri::command]
async fn set_preferences(
    state: tauri::State<'_, Arc<AppState>>,
    preferences: Preferences,
) -> Result<(), String> {
    let persona = if preferences.persona.trim().is_empty() {
        DEFAULT_PERSONA.to_string()
    } else {
        preferences.persona.trim().to_string()
    };

    let store = state.store.read().await.clone();
    store
        .set_setting(PERSONA_SETTING_KEY, &persona)
        .await
        .map_err(|e| e.to_string())?;
    store
        .set_setting(
            TALK_BACK_DEFAULT_KEY,
            if preferences.talk_back_default {
                "true"
            } else {
                "false"
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    store
        .set_setting(
            NOTIFICATIONS_ENABLED_KEY,
            if preferences.notifications_enabled {
                "true"
            } else {
                "false"
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    *state.persona.write().await = persona;
    *state.notifications_enabled.write().await = preferences.notifications_enabled;
    state.rebuild_runtime().await.map_err(|e| e.to_string())
}

/// Starts recording from the default microphone. Errors if no voice engine
/// is available (no whisper model configured - see the README).
#[tauri::command]
async fn start_recording(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let voice = state
        .voice
        .as_ref()
        .ok_or("voice input isn't available (no whisper model configured)")?;
    voice.start_recording().await.map_err(|e| e.to_string())
}

/// Stops recording and transcribes what was captured.
#[tauri::command]
async fn stop_recording(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let voice = state
        .voice
        .as_ref()
        .ok_or("voice input isn't available (no whisper model configured)")?;
    voice.stop_recording().await.map_err(|e| e.to_string())
}

/// Speaks `text` aloud via the OS's native TTS engine.
#[tauri::command]
async fn speak(state: tauri::State<'_, Arc<AppState>>, text: String) -> Result<(), String> {
    let voice = state
        .voice
        .as_ref()
        .ok_or("talk-back isn't available (no text-to-speech backend on this system)")?;
    voice.speak(text).await.map_err(|e| e.to_string())
}

/// Registers the llmprism provider matching `id`, if Otto knows one -
/// mechanical, one arm per entry in `otto_core::PROVIDER_CATALOG`. Adding a
/// new provider here (plus its Cargo feature and a `PROVIDER_CATALOG` row)
/// is the entire integration surface.
fn register_provider(llm: &mut LlmRegistry, id: &str, api_key: String) {
    match id {
        "anthropic" => llm.register(id, AnthropicProvider::new(api_key)),
        "openai" => llm.register(id, OpenAiProvider::new(api_key)),
        "gemini" => llm.register(id, GeminiProvider::new(api_key)),
        "groq" => llm.register(id, GroqProvider::new(api_key)),
        "deepseek" => llm.register(id, DeepSeekProvider::new(api_key)),
        "mistral" => llm.register(id, MistralProvider::new(api_key)),
        "xai" => llm.register(id, XaiProvider::new(api_key)),
        "openrouter" => llm.register(id, OpenRouterProvider::new(api_key)),
        "perplexity" => llm.register(id, PerplexityProvider::new(api_key)),
        "zai" => llm.register(id, ZaiProvider::new(api_key)),
        "ollama" => llm.register(id, OllamaProvider::new()),
        other => {
            tracing::warn!("no llmprism provider wired up for '{other}' - update register_provider")
        }
    }
}

async fn build_runtime(
    credentials: Arc<dyn CredentialStore>,
    notifications_enabled: bool,
) -> anyhow::Result<SharedRuntime> {
    let mut llm = LlmRegistry::new();
    for provider in PROVIDER_CATALOG {
        if provider.needs_api_key {
            // Missing key isn't fatal: the provider simply isn't
            // registered, so selecting a model that needs it fails with a
            // clear "not configured" message (see `run_command`) instead
            // of the app refusing to start.
            if let Ok(key) = credentials.get(&credential_key(provider.id)).await {
                register_provider(&mut llm, provider.id, key);
            }
        } else {
            // Ollama: a local server, not a hosted API key - always
            // registered once compiled in, same as llmprism's own
            // `Registry::from_env()` treats it. If nothing's listening on
            // localhost, a request against it just fails at call time.
            register_provider(&mut llm, provider.id, String::new());
        }
    }

    let default_repo = std::env::var("OTTO_DEFAULT_REPO").ok();

    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(GithubSkill::new(
        credentials.clone(),
        default_repo.clone(),
    )));
    registry.register(Arc::new(DeploySkill::new(
        credentials.clone(),
        default_repo,
    )));
    registry.register(Arc::new(CalendarSkill::new(credentials.clone())));
    registry.register(Arc::new(NotifySkill::new(notifications_enabled)));

    Ok(SharedRuntime {
        registry: Arc::new(registry),
        llm,
    })
}

async fn build_state() -> anyhow::Result<AppState> {
    let profile_store = Arc::new(ProfileStore::open()?);
    // Registered by the user against a GitHub OAuth App with Device Flow
    // enabled - see the README. `None` just means "Sign in with GitHub"
    // isn't offered; pasting a token still works either way.
    let github_client_id = std::env::var("OTTO_GITHUB_CLIENT_ID").ok();

    // Nothing to load yet on a genuine fresh install (no profile has ever
    // been current) - `LOCAL_PROFILE_ID` is a safe placeholder to build
    // *some* scoped state against so every Tauri command has a `store`/
    // `credentials` to read, without implying anyone is signed in:
    // `get_current_profile` reads `profile_store.current()` directly, not
    // this state, so the frontend still shows the sign-in gate until a real
    // sign-in calls `switch_to_profile`.
    let current_profile_id = profile_store
        .current()?
        .map(|p| p.id)
        .unwrap_or_else(|| LOCAL_PROFILE_ID.to_string());
    let scoped = load_profile_scoped(&current_profile_id).await?;

    let voice = match otto_voice::default_model_path() {
        Ok(model_path) => match VoiceEngine::spawn(model_path) {
            Ok(engine) => Some(engine),
            Err(e) => {
                tracing::warn!("voice input/output disabled: {e}");
                None
            }
        },
        Err(e) => {
            tracing::warn!("voice input/output disabled: {e}");
            None
        }
    };

    Ok(AppState {
        profile_store,
        github_client_id,
        credentials: RwLock::new(scoped.credentials),
        runtime: RwLock::new(scoped.runtime),
        selected: RwLock::new(scoped.selected),
        persona: RwLock::new(scoped.persona),
        notifications_enabled: RwLock::new(scoped.notifications_enabled),
        store: RwLock::new(scoped.store),
        voice,
    })
}

/// The repos the growth digest looks at - `OTTO_GROWTH_REPOS` (comma-
/// separated "owner/name" entries) if set, else `OTTO_DEFAULT_REPO` alone,
/// else empty (meaning "whatever `GithubSkill`'s own default resolves to" -
/// unchanged single-repo behavior for anyone not opting into the new var).
fn growth_repos() -> Vec<String> {
    parse_growth_repos(
        std::env::var("OTTO_GROWTH_REPOS").ok(),
        std::env::var("OTTO_DEFAULT_REPO").ok(),
    )
}

/// The parsing logic behind [`growth_repos`], pulled out so it's testable
/// without mutating process-global env state (`std::env::set_var` in a
/// `#[test]` races every other test running in parallel).
fn parse_growth_repos(
    growth_repos_env: Option<String>,
    default_repo_env: Option<String>,
) -> Vec<String> {
    if let Some(repos) = growth_repos_env {
        return repos
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    }
    default_repo_env.into_iter().collect()
}

/// Spawns the background task that runs `run_growth_digest` on a repeating
/// interval (`OTTO_GROWTH_INTERVAL_HOURS`, default 24h) for as long as the
/// app is running. Reads `state.runtime`/`state.selected` fresh every tick
/// (rather than capturing a snapshot once at startup) so a change made via
/// Settings takes effect on the very next tick, not just after a restart.
fn spawn_growth_digest(state: Arc<AppState>) {
    let interval_hours: u64 = std::env::var("OTTO_GROWTH_INTERVAL_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);
    let repos = growth_repos();

    tauri::async_runtime::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(interval_hours * 3600));
        loop {
            ticker.tick().await;
            let rt = state.runtime.read().await;
            let selected = state.selected.read().await.clone();
            let store = state.store.read().await.clone();
            if let Err(e) = otto_core::run_growth_digest(
                &rt.registry,
                &rt.llm,
                &selected.provider,
                &selected.model,
                &*store,
                &repos,
            )
            .await
            {
                tracing::debug!("growth digest skipped this cycle: {e}");
            }
        }
    });
}

pub fn run() {
    // Loads `.env` (searched from the current directory upward) into the
    // process environment, if present. Must run before anything below
    // reads `OTTO_*` env vars.
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let active_window = match ActiveWindow::from_env() {
        None => None,
        Some(Ok(window)) => Some(window),
        Some(Err(e)) => {
            tracing::warn!("ignoring malformed active-hours schedule: {e}");
            None
        }
    };

    if let Some(window) = &active_window {
        if !window.is_active_now() {
            tracing::info!("outside Otto's configured active hours; exiting without starting");
            let _ = notify_rust::Notification::new()
                .summary("Otto")
                .body("Otto is off right now - it'll be back during its scheduled hours.")
                .show();
            return;
        }
    }

    let state = Arc::new(
        tauri::async_runtime::block_on(build_state())
            .expect("failed to initialize Otto (see the README's setup section)"),
    );

    tauri::Builder::default()
        .manage(state.clone())
        .setup(move |app| {
            if let Some(window) = active_window {
                // We only got here because `window.is_active_now()` was
                // true, so this is the time remaining until it next turns
                // off - a single sleep-then-exit, not a poll loop.
                let handle = app.handle().clone();
                let sleep_for = window.duration_until_next_boundary();
                let state_for_schedule = state.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(sleep_for).await;
                    tracing::info!("Otto's scheduled active window has ended; exiting");
                    let _ = state_for_schedule
                        .store
                        .read()
                        .await
                        .record_event(
                            ActivityKind::Schedule,
                            "Scheduled shutdown",
                            "Otto's active-hours window ended - shutting down until it reopens.",
                        )
                        .await;
                    handle.exit(0);
                });
            }

            spawn_growth_digest(state.clone());

            // Native app menu, standard on every desktop platform: an "Otto"
            // menu with About, Settings, and Quit, plus the usual Edit menu
            // (so cut/copy/paste and their OS keyboard shortcuts work in
            // text fields). "Settings..." emits an event the frontend
            // listens for to open its Settings panel, rather than a second
            // native window. About's copyright/license/website come from
            // tauri.conf.json's `bundle` config and its name/version/authors
            // from Cargo.toml (via `package_info()`) - one source of truth,
            // not duplicated into Rust source.
            let handle = app.handle();
            // Cargo's CARGO_PKG_AUTHORS (what package_info().authors comes
            // from) joins multiple authors with ':' - split it back out so
            // each shows as its own entry rather than one run-on string.
            let authors: Vec<String> = app
                .package_info()
                .authors
                .split(':')
                .map(|a| a.trim().to_string())
                .collect();
            // `credits` is the macOS-only equivalent of `authors` (which
            // AboutMetadata's own docs mark unsupported on macOS) - set
            // both so contributors show up on every platform.
            let credits = authors.join(", ");
            // The full LICENSE text, not just "MIT" - the license terms
            // themselves, not a one-word pointer to them.
            let license_text = include_str!("../../../LICENSE");
            // A properly about-dialog-sized icon (128x128) - the full
            // window icon (512x512 source / multi-res .ico) rendered life
            // size in a native about box, which is far too large.
            let about_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/about-icon.png")).ok();

            let about = AboutMetadataBuilder::new()
                .name(Some(app.package_info().name.clone()))
                .version(Some(app.package_info().version.to_string()))
                .authors(Some(authors))
                .credits(Some(credits))
                .copyright(app.config().bundle.copyright.clone())
                .license(Some(license_text.to_string()))
                .website(app.config().bundle.homepage.clone())
                .website_label(Some("github.com/bean-code/otto"))
                .icon(about_icon)
                .build();
            let file_menu = SubmenuBuilder::new(handle, "Otto")
                .about(Some(about))
                .separator()
                .text("settings", "Settings...")
                .separator()
                .quit()
                .build()?;
            let edit_menu = SubmenuBuilder::new(handle, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;
            let menu = MenuBuilder::new(handle)
                .item(&file_menu)
                .item(&edit_menu)
                .build()?;
            app.set_menu(menu)?;

            app.on_menu_event(|app, event| {
                if event.id() == "settings" {
                    let _ = app.emit("open-settings", ());
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_command,
            start_recording,
            stop_recording,
            speak,
            list_activities,
            list_providers,
            list_models,
            get_selected_model,
            set_selected_model,
            list_skills,
            get_greeting_name,
            get_credential_status,
            set_credential,
            connect_google_calendar_cmd,
            list_profiles,
            get_current_profile,
            start_github_device_flow,
            sign_in_with_pat,
            switch_profile,
            sign_out,
            get_preferences,
            set_preferences
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Otto desktop app");
}

#[cfg(test)]
mod growth_repos_tests {
    use super::parse_growth_repos;

    #[test]
    fn splits_and_trims_comma_separated_repos() {
        let repos = parse_growth_repos(
            Some(" acme/widgets, acme/gadgets ,acme/thing".to_string()),
            None,
        );
        assert_eq!(repos, vec!["acme/widgets", "acme/gadgets", "acme/thing"]);
    }

    #[test]
    fn falls_back_to_default_repo_when_unset() {
        let repos = parse_growth_repos(None, Some("acme/widgets".to_string()));
        assert_eq!(repos, vec!["acme/widgets"]);
    }

    #[test]
    fn empty_when_neither_is_set() {
        assert!(parse_growth_repos(None, None).is_empty());
    }

    #[test]
    fn growth_repos_takes_precedence_over_default_repo() {
        let repos = parse_growth_repos(
            Some("acme/widgets".to_string()),
            Some("other/repo".to_string()),
        );
        assert_eq!(repos, vec!["acme/widgets"]);
    }

    #[test]
    fn ignores_blank_entries() {
        let repos = parse_growth_repos(Some("acme/widgets,,  ,acme/gadgets".to_string()), None);
        assert_eq!(repos, vec!["acme/widgets", "acme/gadgets"]);
    }
}
