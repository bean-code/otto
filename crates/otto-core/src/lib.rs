pub mod agent;
pub mod credentials;
pub mod error;
pub mod growth;
pub mod models;
pub mod profiles;
pub mod providers;
pub mod registry;
pub mod schedule;
pub mod skill;
pub mod store;

pub use agent::{AgentLoop, AgentStep, AgentTurn, DEFAULT_PERSONA};
pub use credentials::{CredentialStore, KeyringStore};
pub use error::{CoreError, CoreResult};
pub use growth::run_growth_digest;
pub use models::{ModelInfo, MODEL_CATALOG};
pub use profiles::{Profile, ProfileStore, LOCAL_PROFILE_ID};
pub use providers::{credential_key, provider_info, ProviderInfo, PROVIDER_CATALOG};
pub use registry::{SkillInfo, SkillRegistry};
pub use schedule::ActiveWindow;
pub use skill::{Skill, SkillOutcome, ToolSpec};
pub use store::{ActivityEntry, ActivityKind, ActivityLog, SqliteStore};

/// The LLM reasoning layer is `llmprism::Registry` directly: register
/// whichever provider(s) you want (Anthropic by default - see
/// `otto-cli`/`otto-ui` for how the API key is pulled from the credential
/// store), and `AgentLoop` drives it. Swapping providers, or wrapping one in
/// middleware (logging, caching, a default system prompt), is a `Registry`
/// concern - `AgentLoop` and every `Skill` are unaware of which provider is
/// behind it.
pub use llmprism;
