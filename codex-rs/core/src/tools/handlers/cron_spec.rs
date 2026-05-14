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
        (
            "background".to_string(),
            JsonSchema::boolean(Some(
                "Optional. Default `true`. Controls whether each firing is visible in chat.\n\n\
                Pass `false` when the user clearly wants to SEE the result of every firing — phrases like \"say X every minute\", \"tell me X each hour\", \"print Y\", \"show me Z periodically\", \"report back every N minutes\". With `background=false`, the agent's reply is rendered as a normal chat turn.\n\n\
                Pass `true` (or omit) when the user wants the firing to run quietly and only alert on a condition — phrases like \"alert me if\", \"only tell me when\", \"check X and notify me if Y\", \"watch for changes\". With `background=true`, the agent's reply text is hidden; only tool-call output (e.g. shell `echo`) renders in chat.\n\n\
                Rule of thumb: if the user's request has no conditional (\"only if…\", \"when X happens…\"), prefer `false` so they actually see output. Default to `true` only when the prompt is clearly an alert-style poll."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: CRON_CREATE_TOOL_NAME.to_string(),
        description: r#"Schedule a prompt to be injected as a new user message on a recurring **clock-aligned** schedule, in the current session.

USE THIS TOOL ONLY when the user wants firings tied to wall-clock times:
- "every Monday at 9am"
- "at the top of every hour"
- "daily at 09:00"
- "on the 1st of every month"

Research-workflow examples that fit naturally here (compose with the research skills like `$paper-discovery`, `$hn-synthesis`, `$kb`):
- "every weekday at 9am, run $paper-discovery on new transformer-architecture papers" — daily literature review
- "every Sunday at 8pm, run $hn-synthesis on 'AI safety' for the past week" — weekly digest
- "daily at 6am, sync new Zotero items into the KB" — overnight library sync
- "every Tuesday at 10am, run citation tracking on the papers in my KB" — weekly citation refresh

DO NOT USE THIS TOOL for interval-based requests like:
- "every 5 minutes" — use `loop_start` with interval_seconds=300. The user almost always means "5 minutes from now, then every 5 minutes", NOT "at :00 :05 :10 of every hour".
- "every 30 seconds" — use `loop_start`.
- "every hour starting now" — use `loop_start` with interval_seconds=3600.

Rule of thumb: if the user picks an explicit wall-clock time ("at 9am", "at midnight", "on Sunday"), use cron. If they just give a duration ("every N minutes"), use `loop_start` so the schedule starts from now, not the next clock boundary.

Don't use when:
- The user wants to react to streaming output (logs, build progress) — that's the Monitor tool.
- The user wants the agent to keep checking until a condition is met, then stop — that's the Loop tool.

Visibility (background flag):
- If the user says "say X every hour" / "tell me X each morning" / "report back" — pass `background: false` so they see the agent's reply each time.
- If the user says "alert me if" / "only when" / "check for X and notify me if Y" — pass `background: true` (or omit) so quiet runs don't flood chat.

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
