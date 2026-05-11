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

// @agent-facing
pub fn create_monitor_start_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "command".to_string(),
        JsonSchema::string(Some(
            "Required. Shell command to run in the background. Each stdout line will be injected back into this session as a new user message so you can react to it. Examples: `tail -F build.log`, `ping -c 30 example.com`, `cargo test 2>&1`."
                .to_string(),
        )),
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: MONITOR_START_TOOL_NAME.to_string(),
        description: r#"Spawn a background shell command and stream each output line back into this session as a new user-message turn — so you can react to the output as it appears.

Use when:
- The user wants you to react to streaming output (logs, build progress, test results, network probes) as it arrives.
- There is no fixed schedule — work happens whenever output appears.

Don't use when:
- The user wants something to happen on a fixed schedule (every minute, daily at 9am) — use cron_create.
- The user wants you to keep checking a condition with model-paced retries until it's met — use the loop tool.

Returns a task_id usable with monitor_stop. The command runs until it exits naturally or you stop it. Each stdout line becomes its own user-message turn in this conversation."#
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
