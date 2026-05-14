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
mod watch_for;

pub use list::MonitorListHandler;
pub use start::MonitorStartHandler;
pub use stop::MonitorStopHandler;
pub use wait::MonitorWaitHandler;
pub use watch_for::MonitorWatchForHandler;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct MonitorStartArgs {
    command: String,
    /// Optional Slice 5 toggle. When omitted (default) or `true`, per-line
    /// stdout chat cells are suppressed — the user sees the running
    /// monitor's existence and line count in `/scheduling` but not every
    /// streamed line. Set `false` for live tail-style monitors where you
    /// want the per-line stream.
    #[serde(default = "default_background")]
    background: bool,
}

fn default_background() -> bool {
    true
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct MonitorWatchForArgs {
    task_id: String,
    /// Literal substring to look for in each line. Case-sensitive.
    pattern: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct MonitorWatchForResponse {
    /// True if a matching line was seen (either in the tail buffer at
    /// subscribe time or on the live broadcast).
    matched: bool,
    /// The full matching line, when `matched` is true.
    matching_line: Option<String>,
    /// `"stdout"` or `"stderr"`, when `matched` is true.
    stream: Option<String>,
    /// True when the subprocess terminated before any line matched.
    terminated_without_match: bool,
    /// True when an explicit `timeout_seconds` fired before a match.
    timed_out: bool,
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
