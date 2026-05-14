use codex_scheduling::TaskId;
use codex_tools::ToolName;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::monitor_spec::MONITOR_WATCH_FOR_TOOL_NAME;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::MonitorWatchForArgs;
use super::MonitorWatchForResponse;

pub struct MonitorWatchForHandler;

impl ToolHandler for MonitorWatchForHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(MONITOR_WATCH_FOR_TOOL_NAME)
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "monitor_watch_for handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: MonitorWatchForArgs = parse_arguments(&arguments)?;
        if args.pattern.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "monitor_watch_for requires a non-empty pattern".to_string(),
            ));
        }

        let task_id: TaskId = args.task_id.clone().into();
        let pattern = args.pattern;
        let deadline = args
            .timeout_seconds
            .map(|s| tokio::time::Instant::now() + Duration::from_secs(s));

        let runtime = session.monitor_runtime().ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "scheduling feature is not enabled in this session".to_string(),
            )
        })?;

        // Confirm the monitor exists at all.
        if !runtime.registry.list().into_iter().any(|m| m.id == task_id) {
            return Err(FunctionCallError::RespondToModel(format!(
                "monitor_watch_for: task_id `{}` not found",
                args.task_id
            )));
        }

        // First, scan the tail buffer for an already-emitted match. This
        // covers the race where lines arrive between `monitor_start` and
        // the watcher attaching. Tail entries are prefixed with `[stdout] `
        // or `[stderr] ` by `emit_line`; strip the prefix when matching so
        // the user's `pattern` matches the raw line, not the tag.
        for tail_line in runtime.registry.tail_snapshot(&task_id) {
            let (stream, payload) = strip_stream_prefix(&tail_line);
            if payload.contains(&pattern) {
                tracing::info!(
                    target: "codex_scheduling::monitor",
                    task_id = %task_id,
                    pattern = %pattern,
                    source = "tail",
                    "monitor.watch_for_match"
                );
                return ok_response(MonitorWatchForResponse {
                    matched: true,
                    matching_line: Some(payload.to_string()),
                    stream: Some(stream.to_string()),
                    terminated_without_match: false,
                    timed_out: false,
                });
            }
        }

        // Now subscribe to future lines. `None` means the monitor has
        // already terminated and dropped its broadcast — report it.
        let Some(mut rx) = runtime.subscribe(&task_id) else {
            tracing::info!(
                target: "codex_scheduling::monitor",
                task_id = %task_id,
                pattern = %pattern,
                "monitor.watch_for_terminated"
            );
            return ok_response(MonitorWatchForResponse {
                matched: false,
                matching_line: None,
                stream: None,
                terminated_without_match: true,
                timed_out: false,
            });
        };

        loop {
            // Compute the per-iteration sleep so we don't keep recomputing
            // the deadline each pass.
            let sleep_until = deadline.map(tokio::time::sleep_until);
            tokio::select! {
                biased;
                recv = rx.recv() => {
                    match recv {
                        Ok((stream, line)) => {
                            if line.contains(&pattern) {
                                tracing::info!(
                                    target: "codex_scheduling::monitor",
                                    task_id = %task_id,
                                    pattern = %pattern,
                                    source = "broadcast",
                                    "monitor.watch_for_match"
                                );
                                return ok_response(MonitorWatchForResponse {
                                    matched: true,
                                    matching_line: Some(line),
                                    stream: Some(stream),
                                    terminated_without_match: false,
                                    timed_out: false,
                                });
                            }
                            // No match, keep listening.
                        }
                        Err(RecvError::Closed) => {
                            tracing::info!(
                                target: "codex_scheduling::monitor",
                                task_id = %task_id,
                                pattern = %pattern,
                                "monitor.watch_for_terminated"
                            );
                            return ok_response(MonitorWatchForResponse {
                                matched: false,
                                matching_line: None,
                                stream: None,
                                terminated_without_match: true,
                                timed_out: false,
                            });
                        }
                        Err(RecvError::Lagged(skipped)) => {
                            // The broadcast buffer (256) overran us. The
                            // lagged lines may have contained a match; fall
                            // back to inspecting the tail buffer once before
                            // continuing to listen.
                            tracing::warn!(
                                target: "codex_scheduling::monitor",
                                task_id = %task_id,
                                skipped,
                                "monitor.watch_for_lagged"
                            );
                            for tail_line in runtime.registry.tail_snapshot(&task_id) {
                                let (stream, payload) = strip_stream_prefix(&tail_line);
                                if payload.contains(&pattern) {
                                    return ok_response(MonitorWatchForResponse {
                                        matched: true,
                                        matching_line: Some(payload.to_string()),
                                        stream: Some(stream.to_string()),
                                        terminated_without_match: false,
                                        timed_out: false,
                                    });
                                }
                            }
                        }
                    }
                }
                _ = async {
                    match sleep_until {
                        Some(s) => s.await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    tracing::info!(
                        target: "codex_scheduling::monitor",
                        task_id = %task_id,
                        pattern = %pattern,
                        "monitor.watch_for_timeout"
                    );
                    return ok_response(MonitorWatchForResponse {
                        matched: false,
                        matching_line: None,
                        stream: None,
                        terminated_without_match: false,
                        timed_out: true,
                    });
                }
            }
        }
    }
}

/// Tail entries look like `[stdout] actual line` or `[stderr] actual line`.
/// Split the prefix off so pattern matching runs against the payload only.
fn strip_stream_prefix(tail_line: &str) -> (&str, &str) {
    if let Some(rest) = tail_line.strip_prefix("[stdout] ") {
        ("stdout", rest)
    } else if let Some(rest) = tail_line.strip_prefix("[stderr] ") {
        ("stderr", rest)
    } else {
        ("stdout", tail_line)
    }
}

fn ok_response(
    response: MonitorWatchForResponse,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let body = serde_json::to_string(&response).map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "monitor_watch_for response serialization failed: {err}"
        ))
    })?;
    Ok(FunctionToolOutput::from_text(body, Some(true)))
}
