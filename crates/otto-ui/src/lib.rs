use std::sync::Arc;

use llmprism::providers::anthropic::AnthropicProvider;
use llmprism::Registry as LlmRegistry;
use otto_core::{
    ActiveWindow, ActivityEntry, ActivityKind, ActivityLog, AgentLoop, AgentTurn, CredentialStore, KeyringStore,
    ModelInfo, SkillInfo, SkillRegistry, SqliteStore, DEFAULT_PERSONA, MODEL_CATALOG,
};
use otto_skill_calendar::CalendarSkill;
use otto_skill_deploy::DeploySkill;
use otto_skill_github::GithubSkill;
use otto_skill_notify::NotifySkill;
use otto_voice::VoiceEngine;
use serde::Serialize;
use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::Emitter;
use tokio::sync::RwLock;

const DEFAULT_MODEL: &str = "claude-opus-5";
const ANTHROPIC_KEY: &str = "anthropic-api-key";
const GITHUB_KEY: &str = "github-token";
const MODEL_SETTING_KEY: &str = "model";

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

struct AppState {
    credentials: Arc<dyn CredentialStore>,
    runtime: RwLock<SharedRuntime>,
    provider: String,
    model: RwLock<String>,
    persona: String,
    store: Arc<SqliteStore>,
    voice: Option<VoiceEngine>,
}

impl AppState {
    /// Builds a fresh `AgentLoop` from the current runtime snapshot.
    /// Cheap - `AgentLoop` just holds clones of `Arc`s and small strings, so
    /// there's no need to keep a long-lived instance around and worry about
    /// it going stale after a credential or model change.
    async fn agent(&self) -> AgentLoop {
        let rt = self.runtime.read().await;
        let model = self.model.read().await.clone();
        AgentLoop::new(rt.llm.clone(), self.provider.clone(), model, self.persona.clone(), rt.registry.clone(), self.store.clone())
    }

    async fn rebuild_runtime(&self) -> anyhow::Result<()> {
        let fresh = build_runtime(self.credentials.clone()).await?;
        *self.runtime.write().await = fresh;
        Ok(())
    }
}

#[derive(Serialize)]
struct CredentialStatus {
    anthropic: bool,
    github: bool,
}

#[tauri::command]
async fn run_command(state: tauri::State<'_, Arc<AppState>>, input: String) -> Result<AgentTurn, String> {
    if state.credentials.get(ANTHROPIC_KEY).await.is_err() {
        return Err("no Claude API key configured yet - open Settings and add one".to_string());
    }
    state.agent().await.run(&input).await.map_err(|e| e.to_string())
}

/// Most recent activity first (commands, growth-digest notes, schedule
/// events - one feed), for the Activities tab.
#[tauri::command]
async fn list_activities(state: tauri::State<'_, Arc<AppState>>, limit: Option<u32>) -> Result<Vec<ActivityEntry>, String> {
    state.store.list_recent(limit.unwrap_or(100) as usize).await.map_err(|e| e.to_string())
}

/// The curated model picker's contents - a fixed catalog, not
/// user-editable, so this needs no state.
#[tauri::command]
fn list_models() -> Vec<ModelInfo> {
    MODEL_CATALOG.to_vec()
}

#[tauri::command]
async fn get_model(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    Ok(state.model.read().await.clone())
}

/// Switches the active model immediately and persists the choice so it
/// survives a restart.
#[tauri::command]
async fn set_model(state: tauri::State<'_, Arc<AppState>>, model: String) -> Result<(), String> {
    if !MODEL_CATALOG.iter().any(|m| m.id == model) {
        return Err(format!("unknown model '{model}'"));
    }
    state.store.set_setting(MODEL_SETTING_KEY, &model).await.map_err(|e| e.to_string())?;
    *state.model.write().await = model;
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
    std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok()
}

/// Whether each known credential is currently set - never the value itself.
#[tauri::command]
async fn get_credential_status(state: tauri::State<'_, Arc<AppState>>) -> Result<CredentialStatus, String> {
    Ok(CredentialStatus {
        anthropic: state.credentials.get(ANTHROPIC_KEY).await.is_ok(),
        github: state.credentials.get(GITHUB_KEY).await.is_ok(),
    })
}

/// Saves a credential to the OS keychain and rebuilds the runtime so it
/// takes effect immediately, without restarting the app.
#[tauri::command]
async fn set_credential(state: tauri::State<'_, Arc<AppState>>, key: String, value: String) -> Result<(), String> {
    if key != ANTHROPIC_KEY && key != GITHUB_KEY {
        return Err(format!("unknown credential '{key}'"));
    }
    if value.trim().is_empty() {
        return Err("value can't be empty".to_string());
    }
    state.credentials.set(&key, value.trim()).await.map_err(|e| e.to_string())?;
    state.rebuild_runtime().await.map_err(|e| e.to_string())
}

/// Starts recording from the default microphone. Errors if no voice engine
/// is available (no whisper model configured - see the README).
#[tauri::command]
async fn start_recording(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let voice = state.voice.as_ref().ok_or("voice input isn't available (no whisper model configured)")?;
    voice.start_recording().await.map_err(|e| e.to_string())
}

/// Stops recording and transcribes what was captured.
#[tauri::command]
async fn stop_recording(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let voice = state.voice.as_ref().ok_or("voice input isn't available (no whisper model configured)")?;
    voice.stop_recording().await.map_err(|e| e.to_string())
}

/// Speaks `text` aloud via the OS's native TTS engine.
#[tauri::command]
async fn speak(state: tauri::State<'_, Arc<AppState>>, text: String) -> Result<(), String> {
    let voice = state.voice.as_ref().ok_or("talk-back isn't available (no text-to-speech backend on this system)")?;
    voice.speak(text).await.map_err(|e| e.to_string())
}

async fn build_runtime(credentials: Arc<dyn CredentialStore>) -> anyhow::Result<SharedRuntime> {
    let mut llm = LlmRegistry::new();
    // Missing key isn't fatal here: the provider simply isn't registered,
    // so a command that needs it fails with a clear "not configured"
    // message (see `run_command`) instead of the app refusing to start.
    if let Ok(anthropic_key) = credentials.get(ANTHROPIC_KEY).await {
        llm.register("anthropic", AnthropicProvider::new(anthropic_key));
    }

    let default_repo = std::env::var("OTTO_DEFAULT_REPO").ok();

    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(GithubSkill::new(credentials.clone(), default_repo)));
    registry.register(Arc::new(DeploySkill::new()));
    registry.register(Arc::new(CalendarSkill::new()));
    registry.register(Arc::new(NotifySkill::new()));

    Ok(SharedRuntime { registry: Arc::new(registry), llm })
}

async fn build_state() -> anyhow::Result<AppState> {
    let credentials: Arc<dyn CredentialStore> = Arc::new(KeyringStore::new());
    let runtime = build_runtime(credentials.clone()).await?;

    let store = Arc::new(SqliteStore::open(SqliteStore::default_path()?)?);

    // A model picked via Settings outlives the process it was picked in;
    // OTTO_MODEL/the built-in default are only the first-run fallback.
    let model = match store.get_setting(MODEL_SETTING_KEY).await? {
        Some(saved) => saved,
        None => std::env::var("OTTO_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
    };
    let persona = std::env::var("OTTO_PERSONA").unwrap_or_else(|_| DEFAULT_PERSONA.to_string());

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
        credentials,
        runtime: RwLock::new(runtime),
        provider: "anthropic".to_string(),
        model: RwLock::new(model),
        persona,
        store,
        voice,
    })
}

/// Spawns the background task that runs `run_growth_digest` on a repeating
/// interval (`OTTO_GROWTH_INTERVAL_HOURS`, default 24h) for as long as the
/// app is running. Reads `state.runtime`/`state.model` fresh every tick
/// (rather than capturing a snapshot once at startup) so a change made via
/// Settings takes effect on the very next tick, not just after a restart.
fn spawn_growth_digest(state: Arc<AppState>) {
    let interval_hours: u64 = std::env::var("OTTO_GROWTH_INTERVAL_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);

    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_hours * 3600));
        loop {
            ticker.tick().await;
            let rt = state.runtime.read().await;
            let model = state.model.read().await.clone();
            if let Err(e) = otto_core::run_growth_digest(&rt.registry, &rt.llm, &state.provider, &model, &*state.store).await
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

    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

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
        tauri::async_runtime::block_on(build_state()).expect("failed to initialize Otto (see the README's setup section)"),
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

            // Native app menu, standard on every desktop platform: a File
            // menu with Settings + Quit, and the usual Edit menu (so
            // cut/copy/paste and their OS keyboard shortcuts work in text
            // fields) - "Settings..." emits an event the frontend listens
            // for to open its Settings panel, rather than a second native
            // window.
            let handle = app.handle();
            let file_menu = SubmenuBuilder::new(handle, "Otto")
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
            let menu = MenuBuilder::new(handle).item(&file_menu).item(&edit_menu).build()?;
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
            list_models,
            get_model,
            set_model,
            list_skills,
            get_greeting_name,
            get_credential_status,
            set_credential
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Otto desktop app");
}
