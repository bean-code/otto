use std::sync::Arc;

use async_trait::async_trait;
use octocrab::params::repos::Commitish;
use octocrab::Octocrab;
use otto_core::{CoreError, CoreResult, CredentialStore, Skill, SkillOutcome, ToolSpec};
use serde_json::{json, Value};
use tokio::sync::OnceCell;

pub mod auth;
pub use auth::{
    device_flow_poll, device_flow_start, fetch_identity, DeviceFlowSession, GithubIdentity,
};

const CREDENTIAL_KEY: &str = "github-token";

/// GitHub integration: list open PRs, check their CI status, and open new
/// PRs from a branch. The client is built lazily on first use and cached,
/// since fetching the token from the credential store is async.
pub struct GithubSkill {
    credentials: Arc<dyn CredentialStore>,
    default_repo: Option<String>,
    client: OnceCell<Octocrab>,
}

impl GithubSkill {
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
                Octocrab::builder()
                    .personal_token(token)
                    .build()
                    .map_err(|e| CoreError::SkillFailed {
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
                tool: "github".into(),
                message: "no 'repo' given and no default repo configured; expected \"owner/name\""
                    .into(),
            })?;

        let (owner, name) = repo_str
            .split_once('/')
            .ok_or_else(|| CoreError::InvalidInput {
                tool: "github".into(),
                message: format!("'{repo_str}' is not a valid \"owner/name\" repo"),
            })?;

        Ok((owner.to_string(), name.to_string()))
    }

    fn failed(&self, message: impl std::fmt::Display) -> CoreError {
        CoreError::SkillFailed {
            skill: self.id().to_string(),
            message: message.to_string(),
        }
    }

    async fn list_open_prs(&self, input: Value) -> CoreResult<SkillOutcome> {
        let (owner, repo) = self.resolve_repo(&input)?;
        let client = self.client().await?;

        let page = client
            .pulls(&owner, &repo)
            .list()
            .state(octocrab::params::State::Open)
            .sort(octocrab::params::pulls::Sort::Updated)
            .direction(octocrab::params::Direction::Descending)
            .per_page(25)
            .send()
            .await
            .map_err(|e| self.failed(e))?;

        let prs: Vec<Value> = page
            .items
            .iter()
            .map(|pr| {
                json!({
                    "number": pr.number,
                    "title": pr.title,
                    "author": pr.user.as_ref().map(|u| u.login.clone()),
                    "url": pr.html_url.as_ref().map(|u| u.to_string()),
                    "draft": pr.draft.unwrap_or(false),
                    "head": pr.head.ref_field,
                    "base": pr.base.ref_field,
                    "updated_at": pr.updated_at,
                })
            })
            .collect();

        let summary = if page.items.is_empty() {
            format!("No open pull requests in {owner}/{repo}.")
        } else {
            let lines: Vec<String> = page
                .items
                .iter()
                .map(|pr| {
                    format!(
                        "#{} {}{} - by {}",
                        pr.number,
                        if pr.draft.unwrap_or(false) {
                            "[draft] "
                        } else {
                            ""
                        },
                        pr.title.clone().unwrap_or_default(),
                        pr.user
                            .as_ref()
                            .map(|u| u.login.as_str())
                            .unwrap_or("unknown"),
                    )
                })
                .collect();
            format!(
                "{} open pull request(s) in {owner}/{repo}:\n{}",
                page.items.len(),
                lines.join("\n")
            )
        };

        Ok(SkillOutcome::new(
            summary,
            json!({ "repo": format!("{owner}/{repo}"), "pull_requests": prs }),
        ))
    }

    async fn pr_ci_status(&self, input: Value) -> CoreResult<SkillOutcome> {
        let (owner, repo) = self.resolve_repo(&input)?;
        let pr_number = input
            .get("pr_number")
            .and_then(Value::as_u64)
            .ok_or_else(|| CoreError::InvalidInput {
                tool: "github_pr_ci_status".into(),
                message: "missing required integer field 'pr_number'".into(),
            })?;

        let client = self.client().await?;

        let pr = client
            .pulls(&owner, &repo)
            .get(pr_number)
            .await
            .map_err(|e| self.failed(e))?;
        let sha = pr.head.sha.clone();

        let runs = client
            .checks(&owner, &repo)
            .list_check_runs_for_git_ref(Commitish(sha.clone()))
            .send()
            .await
            .map_err(|e| self.failed(e))?;

        let checks: Vec<Value> = runs
            .check_runs
            .iter()
            .map(|run| {
                let state = run
                    .conclusion
                    .clone()
                    .unwrap_or_else(|| "in_progress".to_string());
                json!({ "name": run.name, "state": state })
            })
            .collect();

        let overall = if runs.check_runs.is_empty() {
            "no_checks"
        } else if runs.check_runs.iter().any(|r| {
            matches!(
                r.conclusion.as_deref(),
                Some("failure") | Some("cancelled") | Some("timed_out")
            )
        }) {
            "failing"
        } else if runs.check_runs.iter().any(|r| r.conclusion.is_none()) {
            "pending"
        } else {
            "passing"
        };

        let check_summary = checks
            .iter()
            .map(|c| {
                format!(
                    "{} = {}",
                    c["name"].as_str().unwrap_or("?"),
                    c["state"].as_str().unwrap_or("?")
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        let summary = if checks.is_empty() {
            format!("PR #{pr_number} in {owner}/{repo} has no CI checks reported.")
        } else {
            format!("PR #{pr_number} in {owner}/{repo}: {overall} ({check_summary})")
        };

        Ok(SkillOutcome::new(
            summary,
            json!({
                "repo": format!("{owner}/{repo}"),
                "pr_number": pr_number,
                "sha": sha,
                "overall": overall,
                "checks": checks,
            }),
        ))
    }

    async fn open_pr(&self, input: Value) -> CoreResult<SkillOutcome> {
        let (owner, repo) = self.resolve_repo(&input)?;
        let head = input
            .get("head")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::InvalidInput {
                tool: "github_open_pr".into(),
                message: "missing required field 'head' (the branch to open the PR from)".into(),
            })?
            .to_string();
        let title = input
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or(&head)
            .to_string();
        let body = input
            .get("body")
            .and_then(Value::as_str)
            .map(str::to_string);
        let draft = input.get("draft").and_then(Value::as_bool);

        let client = self.client().await?;

        let base = match input.get("base").and_then(Value::as_str) {
            Some(b) => b.to_string(),
            None => client
                .repos(&owner, &repo)
                .get()
                .await
                .map_err(|e| self.failed(e))?
                .default_branch
                .ok_or_else(|| self.failed("repo has no default branch and no 'base' was given"))?,
        };

        let pulls = client.pulls(&owner, &repo);
        let mut builder = pulls.create(&title, &head, &base);
        if let Some(body) = body {
            builder = builder.body(body);
        }
        if let Some(draft) = draft {
            builder = builder.draft(draft);
        }

        let pr = builder.send().await.map_err(|e| self.failed(e))?;
        let url = pr
            .html_url
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_default();

        Ok(SkillOutcome::new(
            format!(
                "Opened PR #{} \"{}\" ({head} -> {base}) in {owner}/{repo}: {url}",
                pr.number,
                pr.title.clone().unwrap_or_default(),
            ),
            json!({
                "repo": format!("{owner}/{repo}"),
                "number": pr.number,
                "url": url,
                "head": head,
                "base": base,
            }),
        ))
    }
}

#[async_trait]
impl Skill for GithubSkill {
    fn id(&self) -> &'static str {
        "github"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec::new(
                "github_list_open_prs",
                "List open pull requests in a GitHub repository, most recently updated first.",
                json!({
                    "type": "object",
                    "properties": {
                        "repo": {
                            "type": "string",
                            "description": "Repository as \"owner/name\". Optional if a default repo is configured."
                        }
                    },
                    "required": [],
                    "additionalProperties": false
                }),
            ),
            ToolSpec::new(
                "github_pr_ci_status",
                "Get the CI / check-run status for a specific pull request.",
                json!({
                    "type": "object",
                    "properties": {
                        "repo": {
                            "type": "string",
                            "description": "Repository as \"owner/name\". Optional if a default repo is configured."
                        },
                        "pr_number": {
                            "type": "integer",
                            "description": "The pull request number."
                        }
                    },
                    "required": ["pr_number"],
                    "additionalProperties": false
                }),
            ),
            ToolSpec::new(
                "github_open_pr",
                "Open a new pull request from a branch.",
                json!({
                    "type": "object",
                    "properties": {
                        "repo": {
                            "type": "string",
                            "description": "Repository as \"owner/name\". Optional if a default repo is configured."
                        },
                        "head": {
                            "type": "string",
                            "description": "The branch containing the changes (the source branch)."
                        },
                        "base": {
                            "type": "string",
                            "description": "The branch to merge into. Defaults to the repo's default branch."
                        },
                        "title": {
                            "type": "string",
                            "description": "PR title. Defaults to the head branch name."
                        },
                        "body": {
                            "type": "string",
                            "description": "PR description."
                        },
                        "draft": {
                            "type": "boolean",
                            "description": "Open as a draft PR."
                        }
                    },
                    "required": ["head"],
                    "additionalProperties": false
                }),
            ),
        ]
    }

    async fn call(&self, tool_name: &str, input: Value) -> CoreResult<SkillOutcome> {
        match tool_name {
            "github_list_open_prs" => self.list_open_prs(input).await,
            "github_pr_ci_status" => self.pr_ci_status(input).await,
            "github_open_pr" => self.open_pr(input).await,
            other => Err(CoreError::SkillNotFound(other.to_string())),
        }
    }
}
