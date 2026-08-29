use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use llmprism::error::ToolError;
use llmprism::schema::ObjectSchema;
use llmprism::tool::Tool as LlmTool;
use llmprism::value_objects::ToolOutput;
use llmprism::Registry as LlmRegistry;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CoreError, CoreResult};
use crate::history::HistoryStore;
use crate::registry::SkillRegistry;

const SYSTEM_PROMPT: &str = "You are Otto, a personal automation assistant that runs the user's \
professional life: coding workflows, GitHub pull requests, deployments, and meeting/calendar \
management. Use the available tools whenever the user's request maps to one of them. If a request \
doesn't need a tool, reply directly. Keep replies short and concrete - state what you found or did, \
not how you did it.";

/// The default persona, appended to `SYSTEM_PROMPT`. Overridable per-user via
/// `OTTO_PERSONA` (see [`AgentLoop::new`]) rather than hardcoded forever -
/// this is a starting point, not a fixed identity.
pub const DEFAULT_PERSONA: &str = "The person you're assisting is a senior software engineer with \
roughly a decade of experience - a co-founder and platform engineer who has built and scaled \
systems, technically deep enough that you never need to over-explain fundamentals: get straight to \
the substance, name tradeoffs plainly, and assume fluency with distributed systems, infrastructure, \
and modern tooling. They think like a builder and a leader at once, so when it's useful, connect a \
technical decision to its broader consequences (team, product, cost) rather than treating it in \
isolation. They're also a lifelong reader with a soft spot for novels and a hopeless-romantic streak \
- a well-placed literary turn of phrase or a touch of wit is welcome and often lands better than \
another dry status update, but never let flourish get in the way of the actual answer.";

/// Hard cap on tool-call round-trips per command, so a confused model can't
/// loop forever burning API calls. Passed straight through to llmprism's
/// own tool-calling loop.
const MAX_STEPS: u32 = 8;

/// One tool call the loop made while handling a command, for the UI/history
/// to render alongside the reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub tool_name: String,
    pub summary: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentTurn {
    pub reply: String,
    pub steps: Vec<AgentStep>,
}

/// Adapts one tool exposed by a `Skill` to llmprism's `Tool` trait so the
/// skill registry can be handed straight to a `PendingTextRequest`.
/// llmprism runs the model <-> tool round trips itself; this only needs to
/// dispatch the call and record what happened for the UI/history.
struct SkillToolAdapter {
    name: String,
    description: String,
    schema: ObjectSchema,
    registry: Arc<SkillRegistry>,
    steps: Arc<Mutex<Vec<AgentStep>>>,
}

#[async_trait]
impl LlmTool for SkillToolAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> &ObjectSchema {
        &self.schema
    }

    async fn call(&self, args: Value) -> Result<ToolOutput, ToolError> {
        match self.registry.dispatch(&self.name, args).await {
            Ok(outcome) => {
                self.steps.lock().expect("steps mutex poisoned").push(AgentStep {
                    tool_name: self.name.clone(),
                    summary: outcome.summary.clone(),
                    data: outcome.data,
                });
                Ok(ToolOutput::from(outcome.summary))
            }
            Err(err) => Err(ToolError::Runtime {
                name: self.name.clone(),
                message: err.to_string(),
            }),
        }
    }
}

/// Orchestrates one natural-language command: sends it to the LLM (via
/// llmprism) with the registered skills exposed as tools, and lets
/// llmprism's own tool-calling loop dispatch calls back into
/// `SkillRegistry` until the model produces a final answer.
pub struct AgentLoop {
    llm: LlmRegistry,
    provider: String,
    model: String,
    persona: String,
    registry: Arc<SkillRegistry>,
    history: Arc<dyn HistoryStore>,
}

impl AgentLoop {
    /// `persona` is appended to the operational system prompt as-is - pass
    /// [`DEFAULT_PERSONA`], an `OTTO_PERSONA` env override, or an empty
    /// string for no persona at all.
    pub fn new(
        llm: LlmRegistry,
        provider: impl Into<String>,
        model: impl Into<String>,
        persona: impl Into<String>,
        registry: Arc<SkillRegistry>,
        history: Arc<dyn HistoryStore>,
    ) -> Self {
        Self {
            llm,
            provider: provider.into(),
            model: model.into(),
            persona: persona.into(),
            registry,
            history,
        }
    }

    pub async fn run(&self, input: &str) -> CoreResult<AgentTurn> {
        let steps: Arc<Mutex<Vec<AgentStep>>> = Arc::new(Mutex::new(Vec::new()));

        let tools: Vec<Arc<dyn LlmTool>> = self
            .registry
            .tool_specs()
            .into_iter()
            .map(|spec| {
                let schema = ObjectSchema::from_raw_json_schema(spec.name.clone(), spec.input_schema);
                Arc::new(SkillToolAdapter {
                    name: spec.name,
                    description: spec.description,
                    schema,
                    registry: self.registry.clone(),
                    steps: steps.clone(),
                }) as Arc<dyn LlmTool>
            })
            .collect();

        let system_prompt = if self.persona.is_empty() {
            SYSTEM_PROMPT.to_string()
        } else {
            format!("{SYSTEM_PROMPT}\n\n{}", self.persona)
        };

        let response = self
            .llm
            .text(&self.provider, &self.model)
            .map_err(|e| CoreError::Llm(e.to_string()))?
            .with_system_prompt(system_prompt)
            .with_prompt(input)
            .with_tools(tools)
            .with_max_steps(MAX_STEPS)
            .generate()
            .await
            .map_err(|e| CoreError::Llm(e.to_string()))?;

        let recorded_steps = steps.lock().expect("steps mutex poisoned").clone();

        let turn = AgentTurn {
            reply: response.text.unwrap_or_default().trim().to_string(),
            steps: recorded_steps,
        };
        self.history.record(input, &turn).await?;
        Ok(turn)
    }
}
