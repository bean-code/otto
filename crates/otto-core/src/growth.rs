use llmprism::Registry as LlmRegistry;
use serde_json::json;

use crate::error::{CoreError, CoreResult};
use crate::registry::SkillRegistry;
use crate::store::{ActivityKind, ActivityLog};

/// Looks at the user's current open-PR activity (via whatever `Skill`
/// happens to be registered under the `github_list_open_prs` tool - this
/// module never depends on `otto-skill-github` directly, only on the
/// `Skill`/`SkillRegistry` abstraction) and, if there's something concrete
/// worth surfacing, sends it as a desktop notification via `notify_desktop`
/// and records it to the activity log so it shows up in the Activities
/// feed, not just as a notification that's gone once dismissed.
///
/// Deliberately scoped to *open* PRs for now rather than a fuller activity
/// history (commits, merged/closed PRs across repos) - that's a real
/// limitation, not an oversight: extending it means growing
/// `otto-skill-github`'s tool surface, which is future work.
pub async fn run_growth_digest(
    registry: &SkillRegistry,
    llm: &LlmRegistry,
    provider: &str,
    model: &str,
    activity_log: &dyn ActivityLog,
) -> CoreResult<()> {
    let activity = registry.dispatch("github_list_open_prs", json!({})).await?;

    let prompt = format!(
        "Here is the user's current open pull-request activity:\n\n{}\n\n\
         Give exactly ONE short, specific observation about their engineering habits or a pattern \
         worth improving, grounded only in this data - no generic advice, no filler, 2-3 sentences \
         max. If there's genuinely nothing interesting to say from this data alone, reply with \
         exactly: NOTHING",
        serde_json::to_string_pretty(&activity.data).unwrap_or_default(),
    );

    let response = llm
        .text(provider, model)
        .map_err(|e| CoreError::Llm(e.to_string()))?
        .with_system_prompt(
            "You are a terse, specific engineering mentor reviewing one engineer's real GitHub \
             activity. Never pad your answer, never restate the data back at them.",
        )
        .with_prompt(prompt)
        .generate()
        .await
        .map_err(|e| CoreError::Llm(e.to_string()))?;

    let insight = response.text.unwrap_or_default();
    let insight = insight.trim();
    if insight.is_empty() || insight.eq_ignore_ascii_case("nothing") {
        return Ok(());
    }

    registry
        .dispatch(
            "notify_desktop",
            json!({ "title": "Otto - growth note", "message": insight }),
        )
        .await?;

    activity_log.record_event(ActivityKind::GrowthDigest, "Growth digest", insight).await?;

    Ok(())
}
