use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
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
    credential_key, AgentLoop, CredentialStore, KeyringStore, Profile, ProfileStore, SkillRegistry,
    SqliteStore, DEFAULT_PERSONA, LOCAL_PROFILE_ID,
};
use otto_skill_calendar::{connect_google_calendar, CalendarSkill};
use otto_skill_deploy::DeploySkill;
use otto_skill_github::{device_flow_poll, device_flow_start, fetch_identity, GithubSkill};
use otto_skill_notify::NotifySkill;

const DEFAULT_PROVIDER: &str = "anthropic";
const DEFAULT_MODEL: &str = "claude-opus-5";
const GITHUB_KEY: &str = "github-token";
const MODEL_PROVIDER_SETTING_KEY: &str = "model_provider";
const MODEL_ID_SETTING_KEY: &str = "model_id";
const PERSONA_SETTING_KEY: &str = "persona_override";
const NOTIFICATIONS_ENABLED_KEY: &str = "notifications_enabled";

/// Otto - a personal automation assistant for your professional life.
#[derive(Parser)]
#[command(name = "otto", version, about)]
struct Cli {
    /// Natural-language command, e.g. "show me my open PRs". Required unless
    /// --connect-google-calendar, --login, --login-with-token, --whoami, or
    /// --switch-profile is given.
    #[arg(trailing_var_arg = true)]
    input: Vec<String>,

    /// Run the Google Calendar OAuth2 connect flow (opens the consent
    /// screen in your default browser, waits for it) and exit - the same
    /// flow otto-ui's Settings "Connect" button runs, for when you'd rather
    /// not open the desktop app just for this one-time step. Requires
    /// google-calendar-client-id to already be set (see the README).
    #[arg(long)]
    connect_google_calendar: bool,

    /// Sign in with GitHub via the Device Flow (prints a code and a URL to
    /// visit, then waits for you to enter it) and exit. Requires
    /// --github-client-id / OTTO_GITHUB_CLIENT_ID to be set - see the
    /// README's GitHub OAuth App setup section.
    #[arg(long)]
    login: bool,

    /// Sign in with an existing GitHub personal access token instead of the
    /// device flow, and exit.
    #[arg(long, value_name = "TOKEN")]
    login_with_token: Option<String>,

    /// Print the currently signed-in profile (or that you're not signed in)
    /// and exit.
    #[arg(long)]
    whoami: bool,

    /// Switch the active profile to an already-signed-in one (see --whoami
    /// and the otto-ui Sidebar for known profile ids) and exit.
    #[arg(long, value_name = "PROFILE_ID")]
    switch_profile: Option<String>,

    /// The GitHub OAuth App client id used by --login. The app must have
    /// Device Flow enabled - see the README.
    #[arg(long, env = "OTTO_GITHUB_CLIENT_ID")]
    github_client_id: Option<String>,

    /// Default "owner/name" repo for GitHub commands that don't name one.
    #[arg(long, env = "OTTO_DEFAULT_REPO")]
    repo: Option<String>,

    /// Which LLM provider to use - anthropic, openai, gemini, groq,
    /// deepseek, mistral, xai, openrouter, perplexity, zai, or ollama.
    /// Defaults to whatever's selected in otto-ui (Settings/Models), or
    /// "anthropic" if neither that nor OTTO_PROVIDER is set.
    #[arg(long, env = "OTTO_PROVIDER")]
    provider: Option<String>,

    /// Model to use. Defaults to whatever's selected in otto-ui, or
    /// "claude-opus-5" if neither that nor OTTO_MODEL is set.
    #[arg(long, env = "OTTO_MODEL")]
    model: Option<String>,
}

/// Registers the llmprism provider matching `id`, if Otto knows one -
/// mirrors `otto-ui`'s `register_provider`. Mechanical, one arm per entry
/// in `otto_core::PROVIDER_CATALOG`.
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

/// Where a profile's `SqliteStore` lives - mirrors `otto-ui`'s own helper of
/// the same name. `LOCAL_PROFILE_ID` (the auto-migrated pre-profiles
/// install) resolves to the unchanged, unscoped `default_path()`.
fn store_path_for(profile_id: &str) -> otto_core::CoreResult<PathBuf> {
    if profile_id == LOCAL_PROFILE_ID {
        SqliteStore::default_path()
    } else {
        SqliteStore::path_for_profile(profile_id)
    }
}

/// Shared tail of `--login` and `--login-with-token`: fetch identity,
/// upsert + switch to the profile, and store the token under that
/// profile's own keychain namespace.
async fn complete_github_sign_in(
    profile_store: &ProfileStore,
    token: &str,
) -> anyhow::Result<Profile> {
    let identity = fetch_identity(token).await?;
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
    profile_store.upsert(&profile)?;
    profile_store.set_current(&profile.id)?;
    KeyringStore::for_profile(&profile.id)
        .set(GITHUB_KEY, token)
        .await?;
    Ok(profile)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Loads `.env` (searched from the current directory upward) into the
    // process environment, if present. Must run before `Cli::parse()`,
    // which reads `OTTO_*` env vars for its `env = "..."` fields.
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let profile_store = ProfileStore::open()?;

    if cli.login {
        let client_id = cli.github_client_id.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "--github-client-id (or OTTO_GITHUB_CLIENT_ID) is required for --login - see the README"
            )
        })?;
        let session = device_flow_start(&client_id).await?;
        println!(
            "Go to {} and enter this code: {}",
            session.verification_uri, session.user_code
        );
        let _ = webbrowser::open(&session.verification_uri);
        let token = device_flow_poll(&client_id, &session).await?;
        let profile = complete_github_sign_in(&profile_store, &token).await?;
        println!("Signed in as {} ({}).", profile.display_name, profile.id);
        return Ok(());
    }

    if let Some(token) = &cli.login_with_token {
        let profile = complete_github_sign_in(&profile_store, token.trim()).await?;
        println!("Signed in as {} ({}).", profile.display_name, profile.id);
        return Ok(());
    }

    if cli.whoami {
        match profile_store.current()? {
            Some(profile) => println!("Signed in as {} ({}).", profile.display_name, profile.id),
            None => {
                println!("Not signed in - run `otto --login` or `otto --login-with-token <token>`.")
            }
        }
        return Ok(());
    }

    if let Some(profile_id) = &cli.switch_profile {
        profile_store.get(profile_id)?.ok_or_else(|| {
            anyhow::anyhow!("unknown profile '{profile_id}' - see `otto --whoami`")
        })?;
        profile_store.set_current(profile_id)?;
        println!("Switched to {profile_id}.");
        return Ok(());
    }

    // Every command below reads/writes the *current* profile's own store
    // and keychain namespace - `LOCAL_PROFILE_ID` (auto-migrated from a
    // pre-profiles install, or a genuine fresh install that's never signed
    // in) if nothing is current, so the CLI keeps working unauthenticated
    // exactly as before for anyone not using GitHub sign-in.
    let profile_id = profile_store
        .current()?
        .map(|p| p.id)
        .unwrap_or_else(|| LOCAL_PROFILE_ID.to_string());
    let credentials: Arc<dyn CredentialStore> = Arc::new(KeyringStore::for_profile(&profile_id));

    if cli.connect_google_calendar {
        connect_google_calendar(&*credentials).await?;
        println!("Google Calendar connected.");
        return Ok(());
    }

    if cli.input.is_empty() {
        anyhow::bail!("no command given - pass one, e.g. `otto \"show me my open PRs\"`, or see `otto --help`");
    }
    let input = cli.input.join(" ");

    let store = Arc::new(SqliteStore::open(store_path_for(&profile_id)?)?);

    // Precedence: --provider/--model flag > OTTO_PROVIDER/OTTO_MODEL env >
    // whatever's selected in otto-ui's Settings (same local database) >
    // hardcoded default. Sharing the settings table with otto-ui means
    // picking a model in the app is also what the CLI uses by default.
    let provider = match cli.provider {
        Some(p) => p,
        None => store
            .get_setting(MODEL_PROVIDER_SETTING_KEY)
            .await?
            .unwrap_or_else(|| DEFAULT_PROVIDER.to_string()),
    };
    let model = match cli.model {
        Some(m) => m,
        None => store
            .get_setting(MODEL_ID_SETTING_KEY)
            .await?
            .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
    };

    let provider_info = otto_core::provider_info(&provider)
        .ok_or_else(|| anyhow::anyhow!("unknown provider '{provider}' - see `otto --help`"))?;

    let mut llm = LlmRegistry::new();
    let api_key = if provider_info.needs_api_key {
        credentials.get(&credential_key(provider_info.id)).await?
    } else {
        String::new()
    };
    register_provider(&mut llm, provider_info.id, api_key);

    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(GithubSkill::new(
        credentials.clone(),
        cli.repo.clone(),
    )));
    registry.register(Arc::new(DeploySkill::new(credentials.clone(), cli.repo)));
    registry.register(Arc::new(CalendarSkill::new(credentials.clone())));
    let notifications_enabled = store
        .get_setting(NOTIFICATIONS_ENABLED_KEY)
        .await?
        .map(|v| v == "true")
        .unwrap_or(true);
    registry.register(Arc::new(NotifySkill::new(notifications_enabled)));

    let persona = match store.get_setting(PERSONA_SETTING_KEY).await? {
        Some(persona) => persona,
        None => std::env::var("OTTO_PERSONA").unwrap_or_else(|_| DEFAULT_PERSONA.to_string()),
    };
    let agent = AgentLoop::new(
        llm,
        provider_info.id,
        model,
        persona,
        Arc::new(registry),
        store,
    );

    let turn = agent.run(&input).await?;

    if !turn.reply.is_empty() {
        println!("{}", turn.reply);
    }
    for step in &turn.steps {
        println!("\n[{}] {}", step.tool_name, step.summary);
    }
    eprintln!("\n(via {} / {})", turn.provider, turn.model);

    Ok(())
}
