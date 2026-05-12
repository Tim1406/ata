use codex_scheduling::TaskId;
use codex_scheduling::TaskStatus;
use codex_tools::ToolName;
use std::time::Duration;
use tokio::time::Instant;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::monitor_spec::MONITOR_WAIT_TOOL_NAME;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::MonitorWaitArgs;
use super::MonitorWaitResponse;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const DEFAULT_TIMEOUT_SECONDS: u64 = 600;

pub struct MonitorWaitHandler;

impl ToolHandler for MonitorWaitHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(MONITOR_WAIT_TOOL_NAME)
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
                    "monitor_wait handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: MonitorWaitArgs = parse_arguments(&arguments)?;
        let task_id: TaskId = args.task_id.clone().into();
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS));

        let runtime = session.monitor_runtime().ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "scheduling feature is not enabled in this session".to_string(),
            )
        })?;

        let registry = runtime.registry.clone();
        let deadline = Instant::now() + timeout;

        // Confirm the monitor exists at all before we start polling.
        if !registry.list().into_iter().any(|m| m.id == task_id) {
            return Err(FunctionCallError::RespondToModel(format!(
                "monitor_wait: task_id `{}` not found",
                args.task_id
            )));
        }

        let (final_status, timed_out) = loop {
            let snapshot = registry.list();
            let Some(task) = snapshot.into_iter().find(|m| m.id == task_id) else {
                // Monitor was removed mid-wait (e.g. by monitor_stop).
                break (TaskStatus::Killed, false);
            };
            match task.status {
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Killed => {
                    break (task.status, false);
                }
                _ => {}
            }
            if Instant::now() >= deadline {
                break (task.status, true);
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        };

        let tail = registry.tail_snapshot(&task_id);
        let response = MonitorWaitResponse {
            status: if timed_out {
                "Running".to_string()
            } else {
                format!("{final_status:?}")
            },
            timed_out,
            tail,
        };
        let body = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "monitor_wait response serialization failed: {err}"
            ))
        })?;
        Ok(FunctionToolOutput::from_text(body, Some(true)))
    }
}
