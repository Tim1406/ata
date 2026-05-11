//! In-session scheduling primitives for ata: Cron, Monitor, and Loop.
//!
//! Phase 1 scope: data types only. Higher-level concerns — agent tools,
//! TUI surface, structured logs, resume semantics — land in later phases.
//! All call sites are gated by `codex_features::Feature::Scheduling`.

pub mod cron_job;
pub mod loop_task;
pub mod monitor;
pub mod task;

pub use cron_job::{CronError, CronJob};
pub use loop_task::LoopTask;
pub use monitor::MonitorTask;
pub use task::{TaskId, TaskKind, TaskStatus};
