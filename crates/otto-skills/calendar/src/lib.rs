use async_trait::async_trait;
use otto_core::{CoreError, CoreResult, Skill, SkillOutcome, ToolSpec};
use serde_json::{json, Value};

/// Meeting/calendar integration. Scaffolded against Google Calendar (picked
/// over Outlook as the starting provider - OAuth + Calendar API v3); not
/// yet wired to a live calendar.
pub struct CalendarSkill;

impl CalendarSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CalendarSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for CalendarSkill {
    fn id(&self) -> &'static str {
        "calendar"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec::new(
                "calendar_upcoming",
                "List upcoming meetings on the user's calendar.",
                json!({
                    "type": "object",
                    "properties": {
                        "hours_ahead": {
                            "type": "integer",
                            "description": "How many hours ahead to look. Defaults to 24."
                        }
                    },
                    "required": [],
                    "additionalProperties": false
                }),
            ),
            ToolSpec::new(
                "calendar_conflicts",
                "Flag any overlapping/conflicting meetings in the given window.",
                json!({
                    "type": "object",
                    "properties": {
                        "hours_ahead": {
                            "type": "integer",
                            "description": "How many hours ahead to look. Defaults to 24."
                        }
                    },
                    "required": [],
                    "additionalProperties": false
                }),
            ),
        ]
    }

    async fn call(&self, tool_name: &str, _input: Value) -> CoreResult<SkillOutcome> {
        match tool_name {
            "calendar_upcoming" | "calendar_conflicts" => Err(CoreError::SkillFailed {
                skill: self.id().to_string(),
                message: "the calendar skill is scaffolded but not yet implemented".into(),
            }),
            other => Err(CoreError::SkillNotFound(other.to_string())),
        }
    }
}
