use std::sync::Arc;

use llmprism::providers::anthropic::AnthropicProvider;
use llmprism::Registry as LlmRegistry;
use otto_core::{
    AgentLoop, AgentTurn, ActiveWindow, CredentialStore, JsonFileHistory, KeyringStore, SkillRegistry, DEFAULT_PERSONA,
};
use otto_skill_calendar::CalendarSkill;
use otto_skill_deploy::DeploySkill;
use otto_skill_github::GithubSkill;
use otto_skill_notify::NotifySkill;
use otto_voice::VoiceEngine;

const DEFAULT_MODEL: &str = "claude-opus-5";

struct AppState {
    agent: AgentLoop,
    registry: Arc<SkillRegistry>,
    llm: LlmRegistry,
    provider: String,
    model: String,
    voice: Option<VoiceEngine>,
}

#[tauri::command]
async fn run_command(state: tauri::State<'_, AppState>, input: String) -> Result<AgentTurn, String> {
    state.agent.run(&input).await.map_err(|e| e.to_string())
}

/// Starts recording from the default microphone. Errors if no voice engine
/// is available (no whisper model configured - see the README).
#[tauri::command]
async fn start_recording(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let voice = state.voice.as_ref().ok_or("voice input isn't available (no whisper model configured)")?;
    voice.start_recording().await.map_err(|e| e.to_string())
}

/// Stops recording and transcribes what was captured.
#[tauri::command]
async fn stop_recording(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let voice = state.voice.as_ref().ok_or("voice input isn't available (no whisper model configured)")?;
    voice.stop_recording().await.map_err(|e| e.to_string())
}

/// Speaks `text` aloud via the OS's native TTS engine.
#[tauri::command]
async fn speak(state: tauri::State<'_, AppState>, text: String) -> Result<(), String> {
    let voice = state.voice.as_ref().ok_or("talk-back isn't available (no text-to-speech backend on this system)")?;
    voice.speak(text).await.map_err(|e| e.to_string())
}

async fn build_state() -> anyhow::Result<AppState> {
    let credentials: Arc<dyn CredentialStore> = Arc::new(KeyringStore::new());
    let anthropic_key = credentials.get("anthropic-api-key").await?;

    let mut llm = LlmRegistry::new();
    llm.register("anthropic", AnthropicProvider::new(anthropic_key));

    let default_repo = std::env::var("OTTO_DEFAULT_REPO").ok();
    let model = std::env::var("OTTO_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let persona = std::env::var("OTTO_PERSONA").unwrap_or_else(|_| DEFAULT_PERSONA.to_string());

    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(GithubSkill::new(credentials.clone(), default_repo)));
    registry.register(Arc::new(DeploySkill::new()));
    registry.register(Arc::new(CalendarSkill::new()));
    registry.register(Arc::new(NotifySkill::new()));
    let registry = Arc::new(registry);

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

    let history = Arc::new(JsonFileHistory::new(JsonFileHistory::default_path()?));
    let agent = AgentLoop::new(llm.clone(), "anthropic", model.clone(), persona, registry.clone(), history);

    Ok(AppState {
        agent,
        registry,
        llm,
        provider: "anthropic".to_string(),
        model,
        voice,
    })
}

/// Spawns the background task that runs `run_growth_digest` on a repeating
/// interval (`OTTO_GROWTH_INTERVAL_HOURS`, default 24h) for as long as the
/// app is running.
fn spawn_growth_digest(registry: Arc<SkillRegistry>, llm: LlmRegistry, provider: String, model: String) {
    let interval_hours: u64 = std::env::var("OTTO_GROWTH_INTERVAL_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);

    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_hours * 3600));
        loop {
            ticker.tick().await;
            if let Err(e) = otto_core::run_growth_digest(&registry, &llm, &provider, &model).await {
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

    let state = tauri::async_runtime::block_on(build_state())
        .expect("failed to initialize Otto (check the anthropic-api-key / github-token credentials - see the README)");

    let growth_registry = state.registry.clone();
    let growth_llm = state.llm.clone();
    let growth_provider = state.provider.clone();
    let growth_model = state.model.clone();

    tauri::Builder::default()
        .manage(state)
        .setup(move |app| {
            if let Some(window) = active_window {
                // We only got here because `window.is_active_now()` was
                // true, so this is the time remaining until it next turns
                // off - a single sleep-then-exit, not a poll loop.
                let handle = app.handle().clone();
                let sleep_for = window.duration_until_next_boundary();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(sleep_for).await;
                    tracing::info!("Otto's scheduled active window has ended; exiting");
                    handle.exit(0);
                });
            }

            spawn_growth_digest(growth_registry, growth_llm, growth_provider, growth_model);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![run_command, start_recording, stop_recording, speak])
        .run(tauri::generate_context!())
        .expect("error while running the Otto desktop app");
}
