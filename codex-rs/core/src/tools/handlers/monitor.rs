//! Built-in tool handlers for in-session Monitor watchers.
//!
//! Three tools (`monitor_start`, `monitor_list`, `monitor_stop`) are
//! registered when [`Feature::Scheduling`] is enabled. They operate on the
//! session-scoped `MonitorRuntime` (registry + per-monitor abort handles).

use serde::Deserialize;
use serde::Serialize;

mod list;
mod start;
mod stop;
mod wait;

pub use list::MonitorListHandler;
pub use start::MonitorStartHandler;
pub use stop::MonitorStopHandler;
pub use wait::MonitorWaitHandler;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct MonitorStartArgs {
    command: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct MonitorStopArgs {
    task_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct MonitorWaitArgs {
    task_id: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct MonitorStartResponse {
    task_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct MonitorWaitResponse {
    status: String,
    timed_out: bool,
    tail: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct MonitorSummary {
    task_id: String,
    command: String,
    status: String,
    started_at: Option<String>,
    stopped_at: Option<String>,
    lines_emitted: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct MonitorListResponse {
    monitors: Vec<MonitorSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct MonitorStopResponse {
    stopped: bool,
}
