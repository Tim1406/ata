//! Responses API tool definitions for in-session Cron scheduling.
//!
//! These specs are registered only when [`Feature::Scheduling`] is enabled.
//! Each description includes "use when…" / "don't use when…" guidance so
//! the model picks the right tool from this family (vs. Monitor or Loop,
//! which arrive in later phases).

use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub const CRON_CREATE_TOOL_NAME: &str = "cron_create";
pub const CRON_LIST_TOOL_NAME: &str = "cron_list";
pub const CRON_DELETE_TOOL_NAME: &str = "cron_delete";

// @agent-facing
pub fn create_cron_create_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "cron_expr".to_string(),
            JsonSchema::string(Some(
                "Required. A 6-field cron expression `sec min hour day-of-month month day-of-week`. Examples: `0 0 * * * *` (every hour on the hour), `0 0 9 * * 1-5` (weekdays at 09:00:00), `0 */5 * * * *` (every 5 minutes)."
                    .to_string(),
            )),
        ),
        (
            "prompt".to_string(),
            JsonSchema::string(Some(
                "Required. The user-message text to inject into this session each time the schedule fires."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: CRON_CREATE_TOOL_NAME.to_string(),
        description: r#"Schedule a prompt to be injected as a new user message on a recurring cron schedule, in the current session.

Use when:
- The user asks for something to happen on a fixed schedule (e.g. "every hour", "at 9am daily", "every 5 minutes").
- There is no specific termination condition — the schedule is open-ended.

Don't use when:
- The user wants to react to streaming output (logs, build progress) — that's the Monitor tool.
- The user wants the agent to keep checking until a condition is met, then stop — that's the Loop tool.

The job fires inside the current session; closing the session stops it. Returns a task_id that can be used with cron_delete."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["cron_expr".to_string(), "prompt".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

// @agent-facing
pub fn create_cron_list_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: CRON_LIST_TOOL_NAME.to_string(),
        description: r#"List all cron jobs currently scheduled in this session.

Use when:
- The user asks "what's scheduled?", "what cron jobs are running?", or similar.
- You need to look up a task_id before calling cron_delete.

Returns each job's task_id, cron expression, prompt, status, next_fire_at, and fire_count."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
        output_schema: None,
    })
}

// @agent-facing
pub fn create_cron_delete_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "task_id".to_string(),
        JsonSchema::string(Some(
            "Required. The task_id returned from cron_create or shown in cron_list."
                .to_string(),
        )),
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: CRON_DELETE_TOOL_NAME.to_string(),
        description: r#"Cancel a scheduled cron job by its task_id. After deletion, the job will not fire again.

Use when:
- The user asks to "cancel", "stop", or "remove" a scheduled task.
- The user no longer needs a recurring schedule.

If the task_id is not found, returns a not-found result rather than an error.

Use cron_list first if you don't already have the task_id."#
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
    fn create_tool_requires_cron_expr_and_prompt() {
        let ToolSpec::Function(tool) = create_cron_create_tool() else {
            panic!("cron_create should be a function tool");
        };
        let required = tool
            .parameters
            .required
            .as_ref()
            .expect("required fields list");
        assert!(required.contains(&"cron_expr".to_string()));
        assert!(required.contains(&"prompt".to_string()));
    }

    #[test]
    fn list_tool_has_no_required_args() {
        let ToolSpec::Function(tool) = create_cron_list_tool() else {
            panic!("cron_list should be a function tool");
        };
        assert!(matches!(tool.parameters.required.as_ref().map(|r| r.is_empty()), Some(true)));
    }

    #[test]
    fn delete_tool_requires_task_id() {
        let ToolSpec::Function(tool) = create_cron_delete_tool() else {
            panic!("cron_delete should be a function tool");
        };
        assert_eq!(
            tool.parameters.required.as_ref().expect("required").len(),
            1
        );
    }
}
