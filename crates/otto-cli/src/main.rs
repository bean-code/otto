use std::sync::Arc;

use otto_core::{AgentLoop, CredentialStore, JsonFileHistory, KeyringStore, SkillRegistry, DEFAULT_PERSONA};
use otto_skill_calendar::CalendarSkill;
use otto_skill_deploy::DeploySkill;
use otto_skill_github::GithubSkill;
use otto_skill_notify::NotifySkill;
use clap::Parser;
use llmprism::providers::anthropic::AnthropicProvider;
use llmprism::Registry as LlmRegistry;

const DEFAULT_MODEL: &str = "claude-opus-5";

/// Otto - a personal automation assistant for your professional life.
#[derive(Parser)]
#[command(name = "otto", version, about)]
struct Cli {
    /// Natural-language command, e.g. "show me my open PRs".
    #[arg(trailing_var_arg = true, required = true)]
    input: Vec<String>,

    /// Default "owner/name" repo for GitHub commands that don't name one.
    #[arg(long, env = "OTTO_DEFAULT_REPO")]
    repo: Option<String>,

    /// Claude model to use.
    #[arg(long, env = "OTTO_MODEL", default_value = DEFAULT_MODEL)]
    model: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Loads `.env` (searched from the current directory upward) into the
    // process environment, if present. Must run before `Cli::parse()`,
    // which reads `OTTO_*` env vars for its `env = "..."` fields.
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    let cli = Cli::parse();
    let input = cli.input.join(" ");

    let credentials: Arc<dyn CredentialStore> = Arc::new(KeyringStore::new());
    let anthropic_key = credentials.get("anthropic-api-key").await?;

    let mut llm = LlmRegistry::new();
    llm.register("anthropic", AnthropicProvider::new(anthropic_key));

    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(GithubSkill::new(credentials.clone(), cli.repo)));
    registry.register(Arc::new(DeploySkill::new()));
    registry.register(Arc::new(CalendarSkill::new()));
    registry.register(Arc::new(NotifySkill::new()));

    let persona = std::env::var("OTTO_PERSONA").unwrap_or_else(|_| DEFAULT_PERSONA.to_string());
    let history = Arc::new(JsonFileHistory::new(JsonFileHistory::default_path()?));
    let agent = AgentLoop::new(llm, "anthropic", cli.model, persona, Arc::new(registry), history);

    let turn = agent.run(&input).await?;

    if !turn.reply.is_empty() {
        println!("{}", turn.reply);
    }
    for step in &turn.steps {
        println!("\n[{}] {}", step.tool_name, step.summary);
    }

    Ok(())
}
