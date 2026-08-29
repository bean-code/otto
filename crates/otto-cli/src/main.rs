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
    credential_key, AgentLoop, CredentialStore, KeyringStore, SkillRegistry, SqliteStore,
    DEFAULT_PERSONA,
};
use otto_skill_calendar::{connect_google_calendar, CalendarSkill};
use otto_skill_deploy::DeploySkill;
use otto_skill_github::GithubSkill;
use otto_skill_notify::NotifySkill;

const DEFAULT_PROVIDER: &str = "anthropic";
const DEFAULT_MODEL: &str = "claude-opus-5";
const MODEL_PROVIDER_SETTING_KEY: &str = "model_provider";
const MODEL_ID_SETTING_KEY: &str = "model_id";

/// Otto - a personal automation assistant for your professional life.
#[derive(Parser)]
#[command(name = "otto", version, about)]
struct Cli {
    /// Natural-language command, e.g. "show me my open PRs". Required unless
    /// --connect-google-calendar is given.
    #[arg(trailing_var_arg = true)]
    input: Vec<String>,

    /// Run the Google Calendar OAuth2 connect flow (opens the consent
    /// screen in your default browser, waits for it) and exit - the same
    /// flow otto-ui's Settings "Connect" button runs, for when you'd rather
    /// not open the desktop app just for this one-time step. Requires
    /// google-calendar-client-id to already be set (see the README).
    #[arg(long)]
    connect_google_calendar: bool,

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

    let credentials: Arc<dyn CredentialStore> = Arc::new(KeyringStore::new());

    if cli.connect_google_calendar {
        connect_google_calendar(&*credentials).await?;
        println!("Google Calendar connected.");
        return Ok(());
    }

    if cli.input.is_empty() {
        anyhow::bail!("no command given - pass one, e.g. `otto \"show me my open PRs\"`, or see `otto --help`");
    }
    let input = cli.input.join(" ");

    let store = Arc::new(SqliteStore::open(SqliteStore::default_path()?)?);

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
    registry.register(Arc::new(NotifySkill::new()));

    let persona = std::env::var("OTTO_PERSONA").unwrap_or_else(|_| DEFAULT_PERSONA.to_string());
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
