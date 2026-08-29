use serde::Serialize;

/// One LLM provider Otto can talk to via `llmprism`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ProviderInfo {
    /// The id `llmprism::Registry` registers/looks this provider up under -
    /// also used to derive its credential key (`"{id}-api-key"`) and to
    /// match `ModelInfo::provider`.
    pub id: &'static str,
    pub name: &'static str,
    /// `false` only for Ollama - a local server, not a hosted API key.
    pub needs_api_key: bool,
}

/// Every provider `otto-ui`/`otto-cli` know how to register, in the order
/// they should be listed in Settings. Adding a new one is: add a row here,
/// add its `llmprism` Cargo feature, add its `AnthropicProvider`-shaped
/// `XProvider::new(api_key)` registration in `build_runtime` - three small,
/// mechanical edits, not an architecture change.
pub const PROVIDER_CATALOG: &[ProviderInfo] = &[
    ProviderInfo { id: "anthropic", name: "Anthropic", needs_api_key: true },
    ProviderInfo { id: "openai", name: "OpenAI", needs_api_key: true },
    ProviderInfo { id: "gemini", name: "Google Gemini", needs_api_key: true },
    ProviderInfo { id: "groq", name: "Groq", needs_api_key: true },
    ProviderInfo { id: "deepseek", name: "DeepSeek", needs_api_key: true },
    ProviderInfo { id: "mistral", name: "Mistral", needs_api_key: true },
    ProviderInfo { id: "xai", name: "xAI", needs_api_key: true },
    ProviderInfo { id: "openrouter", name: "OpenRouter", needs_api_key: true },
    ProviderInfo { id: "perplexity", name: "Perplexity", needs_api_key: true },
    ProviderInfo { id: "zai", name: "Z.ai", needs_api_key: true },
    ProviderInfo { id: "ollama", name: "Ollama (local)", needs_api_key: false },
];

/// The OS-keychain credential key a provider's API key is stored under.
/// `"anthropic-api-key"`, `"openai-api-key"`, and so on - one convention,
/// so nothing provider-specific needs its own constant.
pub fn credential_key(provider_id: &str) -> String {
    format!("{provider_id}-api-key")
}

pub fn provider_info(provider_id: &str) -> Option<&'static ProviderInfo> {
    PROVIDER_CATALOG.iter().find(|p| p.id == provider_id)
}
