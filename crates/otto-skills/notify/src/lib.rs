use async_trait::async_trait;
use otto_core::{CoreError, CoreResult, Skill, SkillOutcome, ToolSpec};
use serde_json::{json, Value};

/// Desktop notifications, so the agent loop (or another skill) can alert
/// the user outside of the command bar - e.g. "CI just went red." `enabled`
/// is the user's Preferences toggle: when off, `notify_desktop` succeeds
/// without actually showing anything, so the agent doesn't see an error
/// just because the user opted out.
pub struct NotifySkill {
    enabled: bool,
}

impl NotifySkill {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl Default for NotifySkill {
    fn default() -> Self {
        Self::new(true)
    }
}

#[async_trait]
impl Skill for NotifySkill {
    fn id(&self) -> &'static str {
        "notify"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![ToolSpec::new(
            "notify_desktop",
            "Show a desktop notification to the user.",
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Notification title." },
                    "message": { "type": "string", "description": "Notification body." }
                },
                "required": ["title", "message"],
                "additionalProperties": false
            }),
        )]
    }

    async fn call(&self, tool_name: &str, input: Value) -> CoreResult<SkillOutcome> {
        if tool_name != "notify_desktop" {
            return Err(CoreError::SkillNotFound(tool_name.to_string()));
        }

        let title = input
            .get("title")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::InvalidInput {
                tool: tool_name.into(),
                message: "missing required field 'title'".into(),
            })?
            .to_string();
        let message = input
            .get("message")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::InvalidInput {
                tool: tool_name.into(),
                message: "missing required field 'message'".into(),
            })?
            .to_string();

        if !self.enabled {
            return Ok(SkillOutcome::new(
                format!(
                    "Notifications are disabled in Preferences - skipped: \"{title}\" - {message}"
                ),
                json!({ "title": title, "message": message, "shown": false }),
            ));
        }

        let (title_owned, message_owned) = (title.clone(), message.clone());
        tokio::task::spawn_blocking(move || {
            notify_rust::Notification::new()
                .summary(&title_owned)
                .body(&message_owned)
                .show()
        })
        .await
        .map_err(|e| CoreError::SkillFailed {
            skill: "notify".into(),
            message: e.to_string(),
        })?
        .map_err(|e| CoreError::SkillFailed {
            skill: "notify".into(),
            message: e.to_string(),
        })?;

        Ok(SkillOutcome::new(
            format!("Notified user: \"{title}\" - {message}"),
            json!({ "title": title, "message": message }),
        ))
    }
}
