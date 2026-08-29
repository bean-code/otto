pub mod agent;
pub mod credentials;
pub mod error;
pub mod growth;
pub mod history;
pub mod registry;
pub mod schedule;
pub mod skill;

pub use agent::{AgentLoop, AgentStep, AgentTurn, DEFAULT_PERSONA};
pub use credentials::{CredentialStore, KeyringStore};
pub use error::{CoreError, CoreResult};
pub use growth::run_growth_digest;
pub use history::{HistoryStore, JsonFileHistory};
pub use registry::SkillRegistry;
pub use schedule::ActiveWindow;
pub use skill::{Skill, SkillOutcome, ToolSpec};

/// The LLM reasoning layer is `llmprism::Registry` directly: register
/// whichever provider(s) you want (Anthropic by default - see
/// `otto-cli`/`otto-ui` for how the API key is pulled from the credential
/// store), and `AgentLoop` drives it. Swapping providers, or wrapping one in
/// middleware (logging, caching, a default system prompt), is a `Registry`
/// concern - `AgentLoop` and every `Skill` are unaware of which provider is
/// behind it.
pub use llmprism;
