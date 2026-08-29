use serde::Serialize;

/// One selectable model, as shown in the model picker.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModelInfo {
    /// The exact id passed to the provider (e.g. `llmprism::Registry::text`).
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    /// A rough cost/capability grouping the UI can use for a badge -
    /// `"flagship"`, `"balanced"`, `"fast"`, or `"frontier"`.
    pub tier: &'static str,
}

/// The curated set of Claude models Otto's picker offers. Deliberately a
/// short, hand-picked list rather than everything the Anthropic API
/// exposes - each one earns its place with a real tradeoff (capability vs.
/// speed vs. cost), not just "another id that exists". Extending this to
/// other providers later means adding a `provider` field and grouping by
/// it in the UI; the picker is already a card grid, not a flat dropdown,
/// so that grouping has somewhere to go.
pub const MODEL_CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "claude-opus-5",
        name: "Opus 5",
        tagline: "Most capable for day-to-day use - the default. Reach for this on anything you'd want a senior engineer's judgment on.",
        tier: "flagship",
    },
    ModelInfo {
        id: "claude-sonnet-5",
        name: "Sonnet 5",
        tagline: "Faster and cheaper than Opus, still sharp - a good pick if you're running Otto constantly through the day.",
        tier: "balanced",
    },
    ModelInfo {
        id: "claude-haiku-4-5",
        name: "Haiku 4.5",
        tagline: "Fastest and cheapest - best for quick, low-stakes lookups where you just want an answer immediately.",
        tier: "fast",
    },
    ModelInfo {
        id: "claude-fable-5",
        name: "Fable 5",
        tagline: "Anthropic's most capable model, full stop - frontier reasoning at a premium cost and with longer turns.",
        tier: "frontier",
    },
];
