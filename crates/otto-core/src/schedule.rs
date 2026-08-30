use std::time::Duration;

use chrono::{Local, NaiveTime, Timelike};

use crate::error::{CoreError, CoreResult};

/// A daily active window ("only run between these times"), read from
/// `OTTO_ACTIVE_FROM` / `OTTO_ACTIVE_UNTIL` (both `HH:MM`, 24h, local time).
/// Supports an overnight window (e.g. `22:00` -> `06:00`).
#[derive(Debug, Clone, Copy)]
pub struct ActiveWindow {
    start: NaiveTime,
    end: NaiveTime,
}

impl ActiveWindow {
    /// Reads `OTTO_ACTIVE_FROM`/`OTTO_ACTIVE_UNTIL` from the environment.
    /// `None` means neither is set - always active, no schedule configured.
    /// `Some(Err(_))` means one is set but malformed, or only one of the two
    /// was given.
    pub fn from_env() -> Option<CoreResult<Self>> {
        let start = std::env::var("OTTO_ACTIVE_FROM").ok();
        let end = std::env::var("OTTO_ACTIVE_UNTIL").ok();
        match (start, end) {
            (None, None) => None,
            (Some(start), Some(end)) => Some(Self::parse(&start, &end)),
            _ => Some(Err(CoreError::Other(anyhow::anyhow!(
                "set both OTTO_ACTIVE_FROM and OTTO_ACTIVE_UNTIL (HH:MM, 24h), or neither"
            )))),
        }
    }

    fn parse(start: &str, end: &str) -> CoreResult<Self> {
        let parse_one = |s: &str| {
            NaiveTime::parse_from_str(s, "%H:%M").map_err(|e| {
                CoreError::Other(anyhow::anyhow!("invalid time '{s}' (expected HH:MM): {e}"))
            })
        };
        Ok(Self {
            start: parse_one(start)?,
            end: parse_one(end)?,
        })
    }

    /// Whether the current local time falls inside the window.
    pub fn is_active_now(&self) -> bool {
        self.contains(Local::now().time())
    }

    fn contains(&self, t: NaiveTime) -> bool {
        if self.start <= self.end {
            t >= self.start && t < self.end
        } else {
            // Overnight window, e.g. 22:00 -> 06:00: active if *outside* the
            // (end, start) gap.
            t >= self.start || t < self.end
        }
    }

    /// How long from now until the window next flips (active -> inactive or
    /// the reverse). Meant for scheduling a single wakeup rather than
    /// polling in a tight loop.
    pub fn duration_until_next_boundary(&self) -> Duration {
        let now = Local::now().time();
        let boundary = if self.contains(now) {
            self.end
        } else {
            self.start
        };

        let now_secs = now.num_seconds_from_midnight() as i64;
        let boundary_secs = boundary.num_seconds_from_midnight() as i64;
        let mut delta = boundary_secs - now_secs;
        if delta <= 0 {
            delta += 24 * 3600;
        }
        Duration::from_secs(delta as u64)
    }
}
