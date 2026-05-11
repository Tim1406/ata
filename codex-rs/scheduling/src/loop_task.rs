//! `LoopTask` — a prompt repeated on a fixed interval or with model-paced
//! wakeups, scoped to the enclosing agent session.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;

use crate::task::TaskId;
use crate::task::TaskStatus;

/// One model-driven loop iteration target. `interval == None` means the
/// model picks the next wakeup delay after each iteration (dynamic pacing,
/// equivalent to Claude Code's `ScheduleWakeup`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopTask {
    pub id: TaskId,
    pub prompt: String,
    /// Fixed delay between iterations. `None` = model-paced.
    pub interval: Option<Duration>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub last_iter_at: Option<DateTime<Utc>>,
    pub next_wakeup_at: Option<DateTime<Utc>>,
    pub iteration_count: u64,
}

impl LoopTask {
    pub fn new_fixed(prompt: String, interval: Duration) -> Self {
        Self::new_inner(prompt, Some(interval))
    }

    pub fn new_dynamic(prompt: String) -> Self {
        Self::new_inner(prompt, None)
    }

    fn new_inner(prompt: String, interval: Option<Duration>) -> Self {
        Self {
            id: TaskId::new(),
            prompt,
            interval,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            last_iter_at: None,
            next_wakeup_at: None,
            iteration_count: 0,
        }
    }

    pub fn is_dynamic(&self) -> bool {
        self.interval.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_loop_records_interval() {
        let l = LoopTask::new_fixed("poll".into(), Duration::from_secs(300));
        assert_eq!(l.interval, Some(Duration::from_secs(300)));
        assert!(!l.is_dynamic());
    }

    #[test]
    fn dynamic_loop_has_no_interval() {
        let l = LoopTask::new_dynamic("watch".into());
        assert_eq!(l.interval, None);
        assert!(l.is_dynamic());
    }

    #[test]
    fn new_loop_starts_pending_with_zero_iterations() {
        let l = LoopTask::new_dynamic("noop".into());
        assert_eq!(l.status, TaskStatus::Pending);
        assert_eq!(l.iteration_count, 0);
        assert!(l.last_iter_at.is_none());
        assert!(l.next_wakeup_at.is_none());
    }
}
