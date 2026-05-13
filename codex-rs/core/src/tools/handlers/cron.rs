//! Built-in tool handlers for in-session Cron scheduling.
//!
//! These three tools (`cron_create`, `cron_list`, `cron_delete`) are
//! registered when [`Feature::Scheduling`] is enabled and operate on the
//! session-scoped `CronRegistry` owned by `Session`.

use serde::Deserialize;
use serde::Serialize;

mod create;
mod delete;
mod list;

pub use create::CronCreateHandler;
pub use delete::CronDeleteHandler;
pub use list::CronListHandler;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CronCreateArgs {
    cron_expr: String,
    prompt: String,
    /// Optional Slice 5 toggle. When omitted (default) or `true`, the cron
    /// fires silently — the agent's natural-language reply is hidden from
    /// chat so periodic crons don't flood it. Tool outputs still render.
    /// Set `false` for verbose diagnostic crons where you want every reply.
    #[serde(default = "default_background")]
    background: bool,
}

fn default_background() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CronDeleteArgs {
    task_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct CronCreateResponse {
    task_id: String,
    next_fire_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct CronJobSummary {
    task_id: String,
    cron_expr: String,
    prompt: String,
    status: String,
    next_fire_at: Option<String>,
    last_fired_at: Option<String>,
    fire_count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct CronListResponse {
    jobs: Vec<CronJobSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct CronDeleteResponse {
    deleted: bool,
}
