//! Responses API tool definitions for in-session Monitor watchers.
//!
//! Registered only when [`Feature::Scheduling`] is enabled. Descriptions
//! include "use when…" / "don't use when…" guidance so the model picks
//! Monitor (stream-driven) over Cron (fixed schedule) or Loop (model-paced
//! retry-until) when appropriate.

use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub const MONITOR_START_TOOL_NAME: &str = "monitor_start";
pub const MONITOR_LIST_TOOL_NAME: &str = "monitor_list";
pub const MONITOR_STOP_TOOL_NAME: &str = "monitor_stop";
pub const MONITOR_WAIT_TOOL_NAME: &str = "monitor_wait";
pub const MONITOR_WATCH_FOR_TOOL_NAME: &str = "monitor_watch_for";

// @agent-facing
pub fn create_monitor_start_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "command".to_string(),
            JsonSchema::string(Some(
                "Required. Shell command to run in the background. Examples: `tail -F build.log`, `ping -c 30 example.com`, `cargo test 2>&1`."
                    .to_string(),
            )),
        ),
        (
            "background".to_string(),
            JsonSchema::boolean(Some(
                "Optional. Default `true`. Controls per-line visibility in chat.\n\n\
                Pass `false` when the user wants to watch streaming output live — phrases like \"show me each ping\", \"tail and display\", \"stream the log\", \"I want to see every line as it comes\". With `background=false`, every stdout/stderr line renders as a chat cell as it happens.\n\n\
                Pass `true` (or omit) when the user just wants to know when it finishes (or alert on the result), not see every line — phrases like \"wait for X to finish then…\", \"download and summarize\", \"run build and tell me if it fails\". With `background=true`, per-line output is suppressed in chat; the `/scheduling` panel's `lines N` counter still climbs and the terminate summary still fires."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: MONITOR_START_TOOL_NAME.to_string(),
        description: r#"Spawn a background shell command. Each output line is streamed to the user's view in real time; on termination you receive a single summary message with status and the tail of output.

Returns immediately with a task_id. To wait for the command to finish and get the final output in the same turn, call `monitor_wait` with that task_id.

Use when:
- The user wants to run a long-running command (build, test, ping, log tail) and react to the result.
- There is no fixed schedule — work happens whenever output appears.

Don't use when:
- The user wants something to happen on a fixed schedule (every minute, daily at 9am) — use cron_create.
- The user wants model-paced retries until a condition is met — use the loop tool.

The command runs until it exits naturally or you call `monitor_stop`."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["command".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

// @agent-facing
pub fn create_monitor_list_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: MONITOR_LIST_TOOL_NAME.to_string(),
        description: r#"List all background monitors active in this session.

Use when:
- The user asks "what's being watched?", "what monitors are running?", or similar.
- You need to look up a task_id before calling monitor_stop.

Returns each monitor's task_id, command, status (Pending / Running / Completed / Failed / Killed), started_at, stopped_at, and lines_emitted count."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
        output_schema: None,
    })
}

// @agent-facing
pub fn create_monitor_watch_for_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "task_id".to_string(),
            JsonSchema::string(Some(
                "Required. The task_id returned from monitor_start.".to_string(),
            )),
        ),
        (
            "pattern".to_string(),
            JsonSchema::string(Some(
                "Required. Literal substring to look for in each output line (case-sensitive). The call returns the moment a matching line appears on stdout or stderr. The tail buffer is also scanned first so matches that happened before this call started are not missed."
                    .to_string(),
            )),
        ),
        (
            "timeout_seconds".to_string(),
            JsonSchema::number(Some(
                "Optional. Maximum seconds to wait before returning even if no line has matched yet. Omit to wait indefinitely — the call stays alive until either (a) a matching line appears, or (b) the subprocess terminates without ever matching."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: MONITOR_WATCH_FOR_TOOL_NAME.to_string(),
        description: r#"Block until a running monitor emits a line containing `pattern`, or until the subprocess terminates without ever matching, or until an explicit `timeout_seconds` fires. Lighter than `monitor_wait` when the caller only cares about a specific event (a phase boundary, an error keyword, a "ready" signal) rather than the final result.

By default this waits indefinitely. Matching is a literal substring on the raw line payload (not the `[stdout]` / `[stderr]` tag).

Returns: {
  matched: bool,
  matching_line: string | null,
  stream: "stdout" | "stderr" | null,
  terminated_without_match: bool,
  timed_out: bool
}.

Use when:
- The user asked you to react to a *specific* phrase in the output ("tell me when the build prints 'warning:'", "alert me when the server says 'listening on port'", "wait for 'PANIC' to appear").
- You want to act mid-stream without waiting for the full subprocess to finish.

Don't use when:
- The user wants the *final result* of the command, including exit status — use `monitor_wait` instead.
- The user wants to keep watching forever — just call `monitor_start` with `background=false` and let the per-line stream render in chat."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["task_id".to_string(), "pattern".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

// @agent-facing
pub fn create_monitor_wait_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "task_id".to_string(),
            JsonSchema::string(Some(
                "Required. The task_id returned from monitor_start.".to_string(),
            )),
        ),
        (
            "timeout_seconds".to_string(),
            JsonSchema::number(Some(
                "Optional. Maximum seconds to wait before returning even if the monitor is still running. Omit to wait indefinitely — the call stays alive for as long as the subprocess takes (minutes, hours, days). Set a value only when the caller explicitly wants to give up after some bound (e.g. \"check back in 30 seconds\")."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: MONITOR_WAIT_TOOL_NAME.to_string(),
        description: r#"Block until a running monitor terminates and return its final status plus the tail of its output. Use this right after monitor_start when you want the result in the same turn instead of polling.

By default this call waits indefinitely — there is no internal time cap. The function sits in a cheap polling loop against an in-memory registry, costs no LLM tokens while waiting, and returns the instant the subprocess actually exits. Set `timeout_seconds` only when the caller explicitly wants a bounded wait.

Returns: { status: "Completed" | "Failed" | "Killed" | "Running" (only if an explicit timeout fires), tail: [string] }.

Use when:
- You started a monitor and want to use its output in the same response (e.g. report the result, summarize errors).
- The user asked for a result that depends on a command finishing.

Don't use when:
- The user wants the monitor to keep running in the background — just call monitor_start and move on."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["task_id".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

// @agent-facing
pub fn create_monitor_stop_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "task_id".to_string(),
        JsonSchema::string(Some(
            "Required. The task_id returned from monitor_start or shown in monitor_list."
                .to_string(),
        )),
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: MONITOR_STOP_TOOL_NAME.to_string(),
        description: r#"Stop a running background monitor by its task_id. The underlying process is killed and no further output is injected.

Use when:
- The user asks to "stop watching", "kill the monitor", or "cancel" a watch task.

If the task_id is not found, returns a not-found result rather than an error. Use monitor_list first if you don't already have the task_id."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["task_id".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_tool_requires_command() {
        let ToolSpec::Function(tool) = create_monitor_start_tool() else {
            panic!("monitor_start should be a function tool");
        };
        assert_eq!(
            tool.parameters.required.as_ref().expect("required").len(),
            1
        );
    }

    #[test]
    fn stop_tool_requires_task_id() {
        let ToolSpec::Function(tool) = create_monitor_stop_tool() else {
            panic!("monitor_stop should be a function tool");
        };
        assert_eq!(
            tool.parameters.required.as_ref().expect("required").len(),
            1
        );
    }

    #[test]
    fn list_tool_has_no_required_args() {
        let ToolSpec::Function(tool) = create_monitor_list_tool() else {
            panic!("monitor_list should be a function tool");
        };
        assert!(matches!(
            tool.parameters.required.as_ref().map(|r| r.is_empty()),
            Some(true)
        ));
    }
}
