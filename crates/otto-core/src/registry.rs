use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::Value;

use crate::error::{CoreError, CoreResult};
use crate::skill::{Skill, SkillOutcome, ToolSpec};

/// Maps tool names to the skill that implements them, and dispatches calls.
#[derive(Default)]
pub struct SkillRegistry {
    owners: HashMap<String, Arc<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a skill's tools. Panics on a duplicate tool name across
    /// skills, since that's a programming error caught at startup.
    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        for tool in skill.tools() {
            if let Some(existing) = self.owners.insert(tool.name.clone(), skill.clone()) {
                panic!(
                    "tool '{}' registered by both '{}' and '{}'",
                    tool.name,
                    existing.id(),
                    skill.id()
                );
            }
        }
    }

    /// The full tool list to send to the LLM, deduplicated by name.
    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        let mut seen = HashSet::new();
        let mut specs = Vec::new();
        for skill in self.owners.values() {
            for tool in skill.tools() {
                if seen.insert(tool.name.clone()) {
                    specs.push(tool);
                }
            }
        }
        specs
    }

    pub async fn dispatch(&self, tool_name: &str, input: Value) -> CoreResult<SkillOutcome> {
        let skill = self
            .owners
            .get(tool_name)
            .ok_or_else(|| CoreError::SkillNotFound(tool_name.to_string()))?;
        skill.call(tool_name, input).await
    }
}
