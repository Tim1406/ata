//! Responses API tool definitions for in-session Loop tasks.
//!
//! Registered when [`Feature::Scheduling`] is enabled. Descriptions are
//! tuned so the model picks Loop (model-paced retry until done) over Cron
//! (fixed clock-time schedule) or Monitor (react to streaming output).
//!
//! Phase 2c ships fixed-interval looping only. Model-paced dynamic delays
//! (Claude Code's `ScheduleWakeup`) are a follow-up.

use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub const LOOP_START_TOOL_NAME: &str = "loop_start";
pub const LOOP_LIST_TOOL_NAME: &str = "loop_list";
pub const LOOP_STOP_TOOL_NAME: &str = "loop_stop";

// @agent-facing
pub fn create_loop_start_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "prompt".to_string(),
            JsonSchema::string(Some(
                "Required. The prompt to inject as a new user-message turn each iteration."
                    .to_string(),
            )),
        ),
        (
            "interval_seconds".to_string(),
            JsonSchema::integer(Some(
                "Required. Seconds between iterations. Minimum 5. Convert any duration the user mentions to seconds before passing in:\n\
                - \"every 30 seconds\" → 30\n\
                - \"every 5 minutes\" → 300\n\
                - \"every 30 minutes\" → 1800\n\
                - \"every hour\" → 3600\n\
                - \"every 6 hours\" → 21600\n\
                - \"every day\" / \"daily\" (interval-style, starting from now) → 86400\n\
                - \"every week\" → 604800\n\
                - \"every month\" (approximate) → 2592000 (30 days)\n\
                Always honor the user's exact unit; don't round to a different unit unless they say so."
                    .to_string(),
            )),
        ),
        (
            "background".to_string(),
            JsonSchema::boolean(Some(
                "Optional. Default `true`. Controls whether each iteration is visible in chat.\n\n\
                Pass `false` when the user clearly wants to SEE each iteration's result — phrases like \"say X every N seconds\", \"tell me X each iteration\", \"print Y every N seconds\", \"show me Z every N\". With `background=false`, the agent's reply is rendered as a normal chat turn.\n\n\
                Pass `true` (or omit) when the loop is a quiet poll that should only alert on a condition — phrases like \"alert me if\", \"check until X is true\", \"poll for changes\", \"keep watching for Y\". With `background=true`, the agent's reply text is hidden; only tool-call output (e.g. shell `echo`) renders in chat.\n\n\
                Rule of thumb: if the user's request has no conditional (\"only if…\", \"when X happens…\"), prefer `false` so they actually see output. Default to `true` only when the prompt is clearly a polling check."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: LOOP_START_TOOL_NAME.to_string(),
        description: r#"Repeat a prompt on a fixed **interval from now**, in this session. The first iteration fires `interval_seconds` after creation; subsequent iterations fire `interval_seconds` apart. Each iteration injects the prompt as a new user-message turn so you can respond to it (think, run tools, summarize). Keeps running until you call loop_stop.

USE THIS TOOL for any interval-based request — the user almost always means "starting now":
- "every N seconds, do X"
- "every N minutes, do X"
- "every hour starting now"
- "run X every 30 seconds"
- "keep checking ... until ..."
- "repeat X N times"

Research-workflow examples (compose with skills like `$paper-discovery`, `$hn-synthesis`, `$kb`):
- "every 4 hours, run $hn-synthesis on 'agent reasoning'" — running research digest
- "every 30 minutes, search arxiv for new papers on AI safety" — frequent feed check
- "every 2 hours, expand citations for papers in the KB" — incremental citation graph build
- "every 15 minutes, check if my paper-summarization batch script has finished new entries"

DO NOT USE THIS TOOL for clock-aligned schedules ("at 9am daily", "every Monday", "on the hour") — those belong to cron_create.

Don't use when:
- You should react to streaming subprocess output as it appears — use monitor_start.
- The work is a tight pure-shell sequence with no agent reasoning per iteration (e.g. "print date 3 times back-to-back as fast as possible") — a `for` loop is fine there.

Returns a task_id usable with loop_stop. Iteration count and last-fired time are exposed via loop_list. Minimum interval is 5 seconds."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["prompt".to_string(), "interval_seconds".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

// @agent-facing
pub fn create_loop_list_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: LOOP_LIST_TOOL_NAME.to_string(),
        description: r#"List all active loops in this session, with their interval, prompt, status, and iteration count.

Use when:
- The user asks "what loops are running?" or you need a task_id before calling loop_stop."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
        output_schema: None,
    })
}

// @agent-facing
pub fn create_loop_stop_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "task_id".to_string(),
        JsonSchema::string(Some(
            "Required. The task_id returned from loop_start or shown in loop_list."
                .to_string(),
        )),
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: LOOP_STOP_TOOL_NAME.to_string(),
        description: r#"Stop a running loop by its task_id. No further iterations fire.

Use when:
- The user asks to "stop", "cancel", or "end" a loop.
- The condition the loop was waiting for has been met."#
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
    fn start_tool_requires_prompt_and_interval() {
        let ToolSpec::Function(tool) = create_loop_start_tool() else {
            panic!("loop_start should be a function tool");
        };
        let required = tool.parameters.required.as_ref().expect("required");
        assert!(required.contains(&"prompt".to_string()));
        assert!(required.contains(&"interval_seconds".to_string()));
    }

    #[test]
    fn list_tool_has_no_required_args() {
        let ToolSpec::Function(tool) = create_loop_list_tool() else {
            panic!("loop_list should be a function tool");
        };
        assert!(matches!(
            tool.parameters.required.as_ref().map(|r| r.is_empty()),
            Some(true)
        ));
    }

    #[test]
    fn stop_tool_requires_task_id() {
        let ToolSpec::Function(tool) = create_loop_stop_tool() else {
            panic!("loop_stop should be a function tool");
        };
        assert_eq!(
            tool.parameters.required.as_ref().expect("required").len(),
            1
        );
    }
}
