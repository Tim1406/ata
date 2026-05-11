use chrono::Utc;
use codex_scheduling::CronJob;
use codex_tools::ToolName;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::cron_spec::CRON_CREATE_TOOL_NAME;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::CronCreateArgs;
use super::CronCreateResponse;

pub struct CronCreateHandler;

impl ToolHandler for CronCreateHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(CRON_CREATE_TOOL_NAME)
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
                    "cron_create handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: CronCreateArgs = parse_arguments(&arguments)?;

        let registry = session.cron_registry().ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "scheduling feature is not enabled in this session".to_string(),
            )
        })?;

        let job = CronJob::new(args.cron_expr, args.prompt).map_err(|err| {
            FunctionCallError::RespondToModel(format!("cron_create rejected: {err}"))
        })?;

        let now = Utc::now();
        let task_id = registry.insert(job, now);

        let next_fire_at = registry
            .list()
            .into_iter()
            .find(|j| j.id == task_id)
            .and_then(|j| j.next_fire_at)
            .map(|t| t.to_rfc3339());

        let response = CronCreateResponse {
            task_id: task_id.to_string(),
            next_fire_at,
        };

        let body = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::RespondToModel(format!("cron_create response serialization failed: {err}"))
        })?;

        Ok(FunctionToolOutput::from_text(body, Some(true)))
    }
}
