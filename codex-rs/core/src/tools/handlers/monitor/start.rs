use chrono::Utc;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SchedulingMonitorOutputDeltaEvent;
use codex_protocol::protocol::Submission;
use codex_protocol::user_input::UserInput;
use codex_scheduling::MonitorTask;
use codex_scheduling::TaskId;
use codex_scheduling::TaskStatus;
use codex_tools::ToolName;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use uuid::Uuid;

use crate::session::session::Session;

use crate::function_tool::FunctionCallError;
use crate::scheduling_runtime::MonitorRuntime;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::monitor_spec::MONITOR_START_TOOL_NAME;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::MonitorStartArgs;
use super::MonitorStartResponse;

pub struct MonitorStartHandler;

impl ToolHandler for MonitorStartHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(MONITOR_START_TOOL_NAME)
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
                    "monitor_start handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: MonitorStartArgs = parse_arguments(&arguments)?;
        if args.command.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "monitor_start requires a non-empty command".to_string(),
            ));
        }

        let runtime = session.monitor_runtime().cloned().ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "scheduling feature is not enabled in this session".to_string(),
            )
        })?;

        let task = MonitorTask::new(args.command.clone());
        let task_id = runtime.registry.insert(task);

        let tx_sub = session.submission_tx();
        let session_for_task = session.clone();
        let registry = runtime.registry.clone();
        let runtime_for_task = runtime.clone();
        let task_id_for_task = task_id.clone();
        let command = args.command;

        let join_handle = tokio::spawn(async move {
            run_monitor(
                task_id_for_task,
                command,
                registry,
                runtime_for_task,
                tx_sub,
                session_for_task,
            )
            .await;
        });
        runtime.store_handle(task_id.clone(), join_handle.abort_handle());

        let response = MonitorStartResponse {
            task_id: task_id.to_string(),
        };
        let body = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "monitor_start response serialization failed: {err}"
            ))
        })?;
        Ok(FunctionToolOutput::from_text(body, Some(true)))
    }
}

async fn run_monitor(
    task_id: TaskId,
    command: String,
    registry: Arc<codex_scheduling::MonitorRegistry>,
    _runtime: Arc<MonitorRuntime>,
    tx_sub: async_channel::Sender<Submission>,
    session: Arc<Session>,
) {
    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(&command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => {
            registry.mark_terminal(&task_id, TaskStatus::Failed, Utc::now());
            tracing::warn!("monitor_start failed to spawn `{command}`: {err}");
            emit_terminate_summary(&task_id, &command, TaskStatus::Failed, Vec::new(), &tx_sub)
                .await;
            return;
        }
    };

    registry.mark_running(&task_id, Utc::now());

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            registry.mark_terminal(&task_id, TaskStatus::Failed, Utc::now());
            emit_terminate_summary(&task_id, &command, TaskStatus::Failed, Vec::new(), &tx_sub)
                .await;
            return;
        }
    };
    let stderr = child.stderr.take();

    let mut stdout_reader = BufReader::new(stdout).lines();

    let stderr_handle = if let Some(stderr) = stderr {
        let registry_for_stderr = registry.clone();
        let task_id_for_stderr = task_id.clone();
        let session_for_stderr = session.clone();
        Some(tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                emit_line(
                    &task_id_for_stderr,
                    "stderr",
                    &line,
                    &registry_for_stderr,
                    &session_for_stderr,
                )
                .await;
            }
        }))
    } else {
        None
    };

    loop {
        match stdout_reader.next_line().await {
            Ok(Some(line)) => {
                emit_line(&task_id, "stdout", &line, &registry, &session).await;
            }
            Ok(None) => break,
            Err(err) => {
                tracing::warn!("monitor stdout read error: {err}");
                break;
            }
        }
    }

    if let Some(h) = stderr_handle {
        let _ = h.await;
    }

    let status = match child.wait().await {
        Ok(status) if status.success() => TaskStatus::Completed,
        Ok(_) => TaskStatus::Failed,
        Err(_) => TaskStatus::Failed,
    };
    registry.mark_terminal(&task_id, status, Utc::now());

    let tail = registry.tail_snapshot(&task_id);
    emit_terminate_summary(&task_id, &command, status, tail, &tx_sub).await;
}

async fn emit_line(
    task_id: &TaskId,
    stream: &str,
    line: &str,
    registry: &Arc<codex_scheduling::MonitorRegistry>,
    session: &Arc<Session>,
) {
    registry.record_line(task_id);
    registry.record_tail_line(task_id, format!("[{stream}] {line}"));

    let event = Event {
        id: format!("monitor-{}", Uuid::now_v7()),
        msg: EventMsg::SchedulingMonitorOutputDelta(SchedulingMonitorOutputDeltaEvent {
            task_id: task_id.to_string(),
            stream: stream.to_string(),
            line: line.to_string(),
        }),
    };
    session.send_event_raw(event).await;
}

async fn emit_terminate_summary(
    task_id: &TaskId,
    command: &str,
    status: TaskStatus,
    tail: Vec<String>,
    tx_sub: &async_channel::Sender<Submission>,
) {
    let status_text = match status {
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        _ => "ended",
    };
    let mut body = format!("[monitor {task_id}] `{command}` {status_text}.");
    if !tail.is_empty() {
        body.push_str("\nLast output:\n");
        for line in tail {
            body.push_str(&line);
            body.push('\n');
        }
    }
    let op = Op::UserInput {
        items: vec![UserInput::Text {
            text: body,
            text_elements: Vec::new(),
        }],
        environments: None,
        final_output_json_schema: None,
        responsesapi_client_metadata: None,
    };
    let sub = Submission {
        id: format!("monitor-{}", Uuid::now_v7()),
        op,
        trace: None,
    };
    let _ = tx_sub.send(sub).await;
}
