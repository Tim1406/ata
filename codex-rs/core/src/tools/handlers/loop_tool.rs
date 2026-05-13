//! Built-in tool handlers for in-session Loop tasks.
//!
//! Three tools (`loop_start`, `loop_list`, `loop_stop`) registered when
//! [`Feature::Scheduling`] is enabled. Module is `loop_tool` rather than
//! `loop` because `loop` is a Rust keyword.

use serde::Deserialize;
use serde::Serialize;

mod list;
mod start;
mod stop;

pub use list::LoopListHandler;
pub use start::LoopStartHandler;
pub use stop::LoopStopHandler;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct LoopStartArgs {
    prompt: String,
    interval_seconds: u64,
    /// Optional Slice 5 toggle. When omitted (default) or `true`, the loop
    /// fires silently — the agent's natural-language reply is hidden so
    /// polling loops don't flood the chat. Tool outputs (bash echoes, etc.)
    /// still render so the prompt's intentional alerts come through. Set
    /// `false` for chatty diagnostic loops where you want every reply.
    #[serde(default = "default_background")]
    background: bool,
}

fn default_background() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct LoopStopArgs {
    task_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct LoopStartResponse {
    task_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct LoopSummary {
    task_id: String,
    prompt: String,
    interval_seconds: Option<u64>,
    status: String,
    last_iter_at: Option<String>,
    iteration_count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct LoopListResponse {
    loops: Vec<LoopSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct LoopStopResponse {
    stopped: bool,
}
