use async_trait::async_trait;
use otto_core::{CoreError, CoreResult, Skill, SkillOutcome, ToolSpec};
use serde_json::{json, Value};

/// CI/CD trigger + monitoring. Scaffolded with the intended tool surface
/// for GitHub Actions (the first target - deploy pipelines on other
/// providers would register more tools on this same skill); not yet wired
/// to the GitHub Actions API.
pub struct DeploySkill;

impl DeploySkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeploySkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for DeploySkill {
    fn id(&self) -> &'static str {
        "deploy"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec::new(
                "deploy_trigger_workflow",
                "Trigger a GitHub Actions workflow run (workflow_dispatch).",
                json!({
                    "type": "object",
                    "properties": {
                        "repo": { "type": "string", "description": "Repository as \"owner/name\"." },
                        "workflow": { "type": "string", "description": "Workflow file name or id, e.g. \"deploy.yml\"." },
                        "ref_name": { "type": "string", "description": "Git ref to run the workflow on, e.g. \"main\"." }
                    },
                    "required": ["repo", "workflow", "ref_name"],
                    "additionalProperties": false
                }),
            ),
            ToolSpec::new(
                "deploy_status",
                "Get the status of the latest run of a GitHub Actions workflow.",
                json!({
                    "type": "object",
                    "properties": {
                        "repo": { "type": "string", "description": "Repository as \"owner/name\"." },
                        "workflow": { "type": "string", "description": "Workflow file name or id, e.g. \"deploy.yml\"." }
                    },
                    "required": ["repo", "workflow"],
                    "additionalProperties": false
                }),
            ),
        ]
    }

    async fn call(&self, tool_name: &str, _input: Value) -> CoreResult<SkillOutcome> {
        match tool_name {
            "deploy_trigger_workflow" | "deploy_status" => Err(CoreError::SkillFailed {
                skill: self.id().to_string(),
                message: "the deploy skill is scaffolded but not yet implemented".into(),
            }),
            other => Err(CoreError::SkillNotFound(other.to_string())),
        }
    }
}
