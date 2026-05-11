//! `CronJob` — a recurring prompt fired on a cron schedule, scoped to the
//! enclosing agent session.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::str::FromStr;
use thiserror::Error;

use crate::task::TaskId;
use crate::task::TaskStatus;

#[derive(Debug, Error)]
pub enum CronError {
    #[error("invalid cron expression: {0}")]
    InvalidExpression(String),
}

/// One scheduled, recurring prompt. The cron expression is validated at
/// construction; the rest of the fields are bookkeeping that later phases
/// (timer engine, persistence) will read and update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: TaskId,
    /// Validated 5- or 6-field cron expression.
    pub cron_expr: String,
    /// The prompt re-injected as a user message when the job fires.
    pub prompt: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub next_fire_at: Option<DateTime<Utc>>,
    pub fire_count: u64,
}

impl CronJob {
    /// Construct a new pending `CronJob`. Returns `CronError::InvalidExpression`
    /// if `cron_expr` is not a parseable cron schedule.
    pub fn new(cron_expr: String, prompt: String) -> Result<Self, CronError> {
        cron::Schedule::from_str(&cron_expr)
            .map_err(|e| CronError::InvalidExpression(e.to_string()))?;
        Ok(Self {
            id: TaskId::new(),
            cron_expr,
            prompt,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            last_fired_at: None,
            next_fire_at: None,
            fire_count: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_cron_expression() {
        let err =
            CronJob::new("not a cron expression".into(), "do the thing".into()).unwrap_err();
        assert!(matches!(err, CronError::InvalidExpression(_)));
    }

    #[test]
    fn accepts_standard_cron_expression() {
        // "0 9 * * * *" (sec min hour dom mon dow) — daily at 09:00:00 in the cron crate's 6-field form.
        let job = CronJob::new("0 0 9 * * *".into(), "check CI".into()).expect("valid cron");
        assert_eq!(job.status, TaskStatus::Pending);
        assert_eq!(job.fire_count, 0);
        assert!(job.last_fired_at.is_none());
    }

    #[test]
    fn each_job_gets_unique_id() {
        let a = CronJob::new("0 0 * * * *".into(), "a".into()).unwrap();
        let b = CronJob::new("0 0 * * * *".into(), "b".into()).unwrap();
        assert_ne!(a.id, b.id);
    }
}
