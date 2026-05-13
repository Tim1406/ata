use chrono::Utc;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::Submission;
use codex_protocol::user_input::UserInput;
use codex_scheduling::LoopTask;
use codex_scheduling::TaskId;
use codex_tools::ToolName;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::loop_tool_spec::LOOP_START_TOOL_NAME;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::LoopStartArgs;
use super::LoopStartResponse;

const MIN_INTERVAL_SECONDS: u64 = 5;

pub struct LoopStartHandler;

impl ToolHandler for LoopStartHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(LOOP_START_TOOL_NAME)
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
                    "loop_start handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: LoopStartArgs = parse_arguments(&arguments)?;
        if args.prompt.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "loop_start requires a non-empty prompt".to_string(),
            ));
        }
        if args.interval_seconds < MIN_INTERVAL_SECONDS {
            return Err(FunctionCallError::RespondToModel(format!(
                "loop_start requires interval_seconds >= {MIN_INTERVAL_SECONDS}"
            )));
        }

        let runtime = session.loop_runtime().cloned().ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "scheduling feature is not enabled in this session".to_string(),
            )
        })?;

        let task = LoopTask::new_fixed(args.prompt.clone(), Duration::from_secs(args.interval_seconds));
        let task_id = runtime.registry.insert(task);

        let tx_sub = session.submission_tx();
        let registry = runtime.registry.clone();
        let task_id_for_task = task_id.clone();
        let prompt = args.prompt;
        let interval = Duration::from_secs(args.interval_seconds);

        let join_handle = tokio::spawn(async move {
            run_loop(task_id_for_task, prompt, interval, registry, tx_sub).await;
        });
        runtime.store_handle(task_id.clone(), join_handle.abort_handle());

        let response = LoopStartResponse {
            task_id: task_id.to_string(),
        };
        let body = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "loop_start response serialization failed: {err}"
            ))
        })?;
        Ok(FunctionToolOutput::from_text(body, Some(true)))
    }
}

async fn run_loop(
    task_id: TaskId,
    prompt: String,
    interval: Duration,
    registry: Arc<codex_scheduling::LoopRegistry>,
    tx_sub: async_channel::Sender<Submission>,
) {
    let mut tick = tokio::time::interval(interval);
    // Skip the first immediate tick so we don't fire at start time.
    tick.tick().await;
    loop {
        tick.tick().await;
        // If `loop_stop` already marked the task terminal but the tokio
        // abort hasn't unwound this task yet (or the interval is in
        // burst-catch-up mode), don't send another submission.
        if registry
            .status(&task_id)
            .is_none_or(|s| s.is_terminal())
        {
            return;
        }
        let now = Utc::now();
        registry.record_iteration(&task_id, now);
        let op = Op::UserInput {
            items: vec![UserInput::Text {
                text: format!("[loop {task_id}] {prompt}"),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        };
        // Encode task_id in the submission id so `submission_loop` can drop
        // already-queued firings after `loop_stop`. `__` is unambiguous
        // because UUIDs never contain underscores.
        let sub = Submission {
            id: format!("loop__{task_id}__{}", Uuid::now_v7()),
            op,
            trace: None,
        };
        if tx_sub.send(sub).await.is_err() {
            return; // submission channel closed
        }
    }
}
