use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;

use crate::error::CoreResult;

/// A tool exposed to the LLM, in the shape Anthropic's `tools` array expects
/// (`name` / `description` / `input_schema`).
#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl ToolSpec {
    pub fn new(name: impl Into<String>, description: impl Into<String>, input_schema: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// Structured result of a skill handling one tool call. `summary` is fed back
/// to the LLM as the tool_result content; `data` is what the UI renders.
#[derive(Debug, Clone, Serialize)]
pub struct SkillOutcome {
    pub summary: String,
    pub data: Value,
}

impl SkillOutcome {
    pub fn new(summary: impl Into<String>, data: Value) -> Self {
        Self {
            summary: summary.into(),
            data,
        }
    }
}

/// One integration (GitHub, deploy, calendar, notify, ...). A skill can own
/// several tools; the registry dispatches by tool name.
#[async_trait]
pub trait Skill: Send + Sync {
    /// Human-readable id for logging, e.g. "github".
    fn id(&self) -> &'static str;

    fn tools(&self) -> Vec<ToolSpec>;

    async fn call(&self, tool_name: &str, input: Value) -> CoreResult<SkillOutcome>;
}
