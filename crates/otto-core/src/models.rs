use serde::Serialize;

/// One selectable model, as shown in the model picker.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModelInfo {
    /// Which provider this model belongs to - matches
    /// `otto_core::providers::ProviderInfo::id`.
    pub provider: &'static str,
    /// The exact id passed to the provider (e.g. `llmprism::Registry::text`).
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    /// A rough cost/capability grouping the UI can use for a badge -
    /// `"flagship"`, `"balanced"`, `"fast"`, or `"frontier"`.
    pub tier: &'static str,
}

/// A curated starting point per provider, not an exhaustive or
/// permanently-accurate list - every provider in `PROVIDER_CATALOG`
/// (`otto_core::providers`) ships new models every few months, so instead
/// of chasing that, the model picker also accepts a free-typed model id for
/// any provider (see `otto-ui`'s Models tab). Treat entries here as good
/// defaults, not a promise they'll still be current; verified against each
/// provider's own docs as of 2026-08-29, not guessed - re-verify before
/// trusting an old-looking entry rather than assuming it's still right.
///
/// OpenRouter and Ollama are deliberately absent: OpenRouter proxies
/// hundreds of other providers' models under its own namespacing (a fixed
/// list would be perpetually incomplete), and Ollama's catalog is whatever
/// the user has pulled locally - neither has a meaningful "curated" list,
/// only the free-typed model id.
pub const MODEL_CATALOG: &[ModelInfo] = &[
    // Anthropic
    ModelInfo {
        provider: "anthropic",
        id: "claude-opus-5",
        name: "Opus 5",
        tagline: "Most capable for day-to-day use - the default. Reach for this on anything you'd want a senior engineer's judgment on.",
        tier: "flagship",
    },
    ModelInfo {
        provider: "anthropic",
        id: "claude-sonnet-5",
        name: "Sonnet 5",
        tagline: "Faster and cheaper than Opus, still sharp - a good pick if you're running Otto constantly through the day.",
        tier: "balanced",
    },
    ModelInfo {
        provider: "anthropic",
        id: "claude-haiku-4-5",
        name: "Haiku 4.5",
        tagline: "Fastest and cheapest - best for quick, low-stakes lookups where you just want an answer immediately.",
        tier: "fast",
    },
    ModelInfo {
        provider: "anthropic",
        id: "claude-fable-5",
        name: "Fable 5",
        tagline: "Anthropic's most capable model, full stop - frontier reasoning at a premium cost and with longer turns.",
        tier: "frontier",
    },
    // OpenAI
    ModelInfo {
        provider: "openai",
        id: "gpt-5.6-sol",
        name: "GPT-5.6 Sol",
        tagline: "OpenAI's flagship for complex professional work - highest cost, highest ceiling.",
        tier: "flagship",
    },
    ModelInfo {
        provider: "openai",
        id: "gpt-5.6-terra",
        name: "GPT-5.6 Terra",
        tagline: "Balances intelligence with cost - the everyday-driver tier.",
        tier: "balanced",
    },
    ModelInfo {
        provider: "openai",
        id: "gpt-5.6-luna",
        name: "GPT-5.6 Luna",
        tagline: "Optimized for budget-conscious, high-volume use.",
        tier: "fast",
    },
    // Google Gemini
    ModelInfo {
        provider: "gemini",
        id: "gemini-3.1-pro-preview",
        name: "Gemini 3.1 Pro",
        tagline: "Advanced reasoning and agentic coding - Google's current most capable model.",
        tier: "flagship",
    },
    ModelInfo {
        provider: "gemini",
        id: "gemini-3.7-flash",
        name: "Gemini 3.7 Flash",
        tagline: "Complex coding and agentic workflows at Flash speed and pricing.",
        tier: "balanced",
    },
    ModelInfo {
        provider: "gemini",
        id: "gemini-3.5-flash-lite",
        name: "Gemini 3.5 Flash-Lite",
        tagline: "The cheapest, fastest Gemini tier for high-throughput lookups.",
        tier: "fast",
    },
    // Groq (fast open-weight inference)
    ModelInfo {
        provider: "groq",
        id: "llama-4-maverick-17b-128e-instruct",
        name: "Llama 4 Maverick",
        tagline: "Groq's largest hosted open-weight model, served at Groq's characteristic inference speed.",
        tier: "flagship",
    },
    ModelInfo {
        provider: "groq",
        id: "llama-4-scout-17b-16e-instruct",
        name: "Llama 4 Scout",
        tagline: "Smaller Llama 4 tier - a balance of capability and throughput.",
        tier: "balanced",
    },
    ModelInfo {
        provider: "groq",
        id: "openai/gpt-oss-20b",
        name: "GPT-OSS 20B",
        tagline: "OpenAI's open-weight model, hosted on Groq for very low latency.",
        tier: "fast",
    },
    // DeepSeek
    ModelInfo {
        provider: "deepseek",
        id: "deepseek-v4-pro",
        name: "DeepSeek V4 Pro",
        tagline: "DeepSeek's reasoning-mode flagship - strong for multi-step, analytical work.",
        tier: "flagship",
    },
    ModelInfo {
        provider: "deepseek",
        id: "deepseek-v4-flash",
        name: "DeepSeek V4 Flash",
        tagline: "Fast, non-thinking mode - general assistant tasks at lower cost.",
        tier: "fast",
    },
    // Mistral - "-latest" aliases, so these stay current on their own.
    ModelInfo {
        provider: "mistral",
        id: "mistral-large-latest",
        name: "Mistral Large",
        tagline: "Mistral's flagship - always resolves to their current largest model.",
        tier: "flagship",
    },
    ModelInfo {
        provider: "mistral",
        id: "mistral-small-latest",
        name: "Mistral Small",
        tagline: "Fast and inexpensive - always resolves to their current small model.",
        tier: "fast",
    },
    // xAI - "grok-4" is xAI's own rolling alias to their latest stable model.
    ModelInfo {
        provider: "xai",
        id: "grok-4",
        name: "Grok 4",
        tagline: "xAI's flagship, via their own always-current alias - strong at coding and agentic tool use.",
        tier: "flagship",
    },
    // Perplexity (search-grounded)
    ModelInfo {
        provider: "perplexity",
        id: "sonar-reasoning-pro",
        name: "Sonar Reasoning Pro",
        tagline: "Search-grounded reasoning, powered by DeepSeek-R1 - best for research-heavy questions.",
        tier: "flagship",
    },
    ModelInfo {
        provider: "perplexity",
        id: "sonar-pro",
        name: "Sonar Pro",
        tagline: "Deeper retrieval with follow-ups - a strong default for grounded, cited answers.",
        tier: "balanced",
    },
    ModelInfo {
        provider: "perplexity",
        id: "sonar",
        name: "Sonar",
        tagline: "Lightweight grounded search - fast, cheap, good for quick lookups.",
        tier: "fast",
    },
    // Z.ai (GLM)
    ModelInfo {
        provider: "zai",
        id: "glm-5.3",
        name: "GLM-5.3",
        tagline: "Z.ai's current flagship - agentic, reasoning, and coding focused.",
        tier: "flagship",
    },
    ModelInfo {
        provider: "zai",
        id: "glm-5.3-flash",
        name: "GLM-5.3 Flash",
        tagline: "Native multimodal, tuned for efficient coding and long-horizon agent tasks.",
        tier: "fast",
    },
];
