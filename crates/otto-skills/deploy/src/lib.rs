use std::sync::Arc;

use async_trait::async_trait;
use octocrab::Octocrab;
use otto_core::{CoreError, CoreResult, CredentialStore, Skill, SkillOutcome, ToolSpec};
use serde_json::{json, Value};
use tokio::sync::OnceCell;

const CREDENTIAL_KEY: &str = "github-token";

/// CI/CD trigger + monitoring, via the GitHub Actions API - reuses the same
/// GitHub token as `GithubSkill` (a PAT with `repo`/`workflow` scope covers
/// both). Deploy pipelines on other providers would register more tools on
/// this same skill.
pub struct DeploySkill {
    credentials: Arc<dyn CredentialStore>,
    default_repo: Option<String>,
    client: OnceCell<Octocrab>,
}

impl DeploySkill {
    pub fn new(credentials: Arc<dyn CredentialStore>, default_repo: Option<String>) -> Self {
        Self {
            credentials,
            default_repo,
            client: OnceCell::new(),
        }
    }

    async fn client(&self) -> CoreResult<&Octocrab> {
        self.client
            .get_or_try_init(|| async {
                let token = self.credentials.get(CREDENTIAL_KEY).await?;
                Octocrab::builder().personal_token(token).build().map_err(|e| CoreError::SkillFailed {
                    skill: self.id().to_string(),
                    message: format!("failed to build github client: {e}"),
                })
            })
            .await
    }

    fn resolve_repo(&self, input: &Value) -> CoreResult<(String, String)> {
        let repo_str = input
            .get("repo")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| self.default_repo.clone())
            .ok_or_else(|| CoreError::InvalidInput {
                tool: "deploy".into(),
                message: "no 'repo' given and no default repo configured; expected \"owner/name\"".into(),
            })?;

        let (owner, name) = repo_str.split_once('/').ok_or_else(|| CoreError::InvalidInput {
            tool: "deploy".into(),
            message: format!("'{repo_str}' is not a valid \"owner/name\" repo"),
        })?;

        Ok((owner.to_string(), name.to_string()))
    }

    fn required_str(&self, input: &Value, tool: &str, field: &str) -> CoreResult<String> {
        input
            .get(field)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| CoreError::InvalidInput {
                tool: tool.into(),
                message: format!("missing required field '{field}'"),
            })
    }

    fn failed(&self, message: impl std::fmt::Display) -> CoreError {
        CoreError::SkillFailed {
            skill: self.id().to_string(),
            message: message.to_string(),
        }
    }

    async fn trigger_workflow(&self, input: Value) -> CoreResult<SkillOutcome> {
        let (owner, repo) = self.resolve_repo(&input)?;
        let workflow = self.required_str(&input, "deploy_trigger_workflow", "workflow")?;
        let ref_name = self.required_str(&input, "deploy_trigger_workflow", "ref_name")?;

        let client = self.client().await?;
        client
            .actions()
            .create_workflow_dispatch(&owner, &repo, &workflow, &ref_name)
            .send()
            .await
            .map_err(|e| self.failed(e))?;

        // GitHub's workflow_dispatch endpoint returns 204 No Content with no
        // run id - there's no way to link straight to the run it just
        // created. `deploy_status` a few seconds later is the way to check.
        Ok(SkillOutcome::new(
            format!("Triggered '{workflow}' on {owner}/{repo}@{ref_name}. Check deploy_status shortly for its result."),
            json!({ "repo": format!("{owner}/{repo}"), "workflow": workflow, "ref": ref_name }),
        ))
    }

    async fn status(&self, input: Value) -> CoreResult<SkillOutcome> {
        let (owner, repo) = self.resolve_repo(&input)?;
        let workflow = self.required_str(&input, "deploy_status", "workflow")?;

        let client = self.client().await?;
        let page = client
            .workflows(&owner, &repo)
            .list_runs(&workflow)
            .per_page(1)
            .send()
            .await
            .map_err(|e| self.failed(e))?;

        let Some(run) = page.items.into_iter().next() else {
            return Ok(SkillOutcome::new(
                format!("No runs found for '{workflow}' in {owner}/{repo}."),
                json!({ "repo": format!("{owner}/{repo}"), "workflow": workflow, "runs": [] }),
            ));
        };

        let conclusion = run.conclusion.clone().unwrap_or_else(|| "pending".to_string());
        let summary = format!(
            "'{workflow}' in {owner}/{repo}: run #{} on {} - {} ({})",
            run.run_number, run.head_branch, run.status, conclusion
        );

        Ok(SkillOutcome::new(
            summary,
            json!({
                "repo": format!("{owner}/{repo}"),
                "workflow": workflow,
                "run_number": run.run_number,
                "branch": run.head_branch,
                "status": run.status,
                "conclusion": run.conclusion,
                "url": run.html_url.to_string(),
            }),
        ))
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
                        "repo": {
                            "type": "string",
                            "description": "Repository as \"owner/name\". Optional if a default repo is configured."
                        },
                        "workflow": { "type": "string", "description": "Workflow file name or id, e.g. \"deploy.yml\"." },
                        "ref_name": { "type": "string", "description": "Git ref to run the workflow on, e.g. \"main\"." }
                    },
                    "required": ["workflow", "ref_name"],
                    "additionalProperties": false
                }),
            ),
            ToolSpec::new(
                "deploy_status",
                "Get the status of the latest run of a GitHub Actions workflow.",
                json!({
                    "type": "object",
                    "properties": {
                        "repo": {
                            "type": "string",
                            "description": "Repository as \"owner/name\". Optional if a default repo is configured."
                        },
                        "workflow": { "type": "string", "description": "Workflow file name or id, e.g. \"deploy.yml\"." }
                    },
                    "required": ["workflow"],
                    "additionalProperties": false
                }),
            ),
        ]
    }

    async fn call(&self, tool_name: &str, input: Value) -> CoreResult<SkillOutcome> {
        match tool_name {
            "deploy_trigger_workflow" => self.trigger_workflow(input).await,
            "deploy_status" => self.status(input).await,
            other => Err(CoreError::SkillNotFound(other.to_string())),
        }
    }
}
