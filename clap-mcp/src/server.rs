//! MCP server transport and handler (rmcp `ServerHandler` + stdio).

use crate::{
    ClapMcpConfig, ClapMcpError, ClapMcpSchemaMetadata, ClapMcpServeOptions, ClapMcpToolError,
    ClapMcpToolOutput, InProcessToolHandler, LOG_INTERPRETATION_INSTRUCTIONS,
    LOGGING_GUIDE_CONTENT, MCP_RESOURCE_URI_SCHEMA, PROMPT_LOGGING_GUIDE, content,
    logging::LoggingMessageNotificationParams,
};
use rmcp::{
    ErrorData as McpError, Peer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, Content, CreateTaskResult, GetPromptRequestParams,
        GetPromptResult, GetTaskInfoParams, GetTaskPayloadResult, GetTaskResult,
        GetTaskResultParams, Implementation, ListPromptsResult, ListResourcesResult,
        ListToolsResult, LoggingLevel, LoggingMessageNotification, LoggingMessageNotificationParam,
        Meta, PaginatedRequestParams, PromptMessage, PromptMessageRole, RawResource,
        ReadResourceRequestParams, ReadResourceResult, Resource, ResourceContents,
        ServerCapabilities, Task, TaskStatus, Tool,
    },
    service::{RequestContext, RoleServer},
    task_manager::{
        OperationDescriptor, OperationMessage, OperationProcessor, ToolCallTaskResult,
        current_timestamp,
    },
};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// Shared handler state: tool execution, resources, prompts, and task policy.
pub(crate) struct ServeHandlerInner {
    pub schema_json: String,
    pub tools: Vec<Tool>,
    pub executable_path: Option<PathBuf>,
    pub in_process_handler: Option<InProcessToolHandler>,
    pub root_name: String,
    pub catch_in_process_panics: bool,
    pub custom_resources: Vec<content::CustomResource>,
    pub custom_prompts: Vec<content::CustomPrompt>,
    pub logging_enabled: bool,
    pub task_augmented_tools: bool,
    pub task_tool_filter: Option<HashSet<String>>,
    #[cfg(feature = "elicitation")]
    pub elicitation_enabled: bool,
}

impl ServeHandlerInner {
    pub fn allows_task_tool(&self, name: &str) -> bool {
        if !self.task_augmented_tools {
            return false;
        }
        match &self.task_tool_filter {
            None => true,
            Some(set) => set.contains(name),
        }
    }

    pub async fn call_tool(
        &self,
        params: &CallToolRequestParams,
        context: &RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool = self.tools.iter().find(|t| t.name == params.name);
        let Some(tool) = tool else {
            return Err(McpError::invalid_params(
                format!("unknown tool: {}", params.name),
                None,
            ));
        };

        let args_map = params.arguments.clone().unwrap_or_default();
        validate_tool_argument_names(tool, &params.name, &args_map)?;

        #[cfg(feature = "elicitation")]
        if self.elicitation_enabled && params.name == "confirm-echo" {
            use rmcp::model::{
                CreateElicitationRequestParams, ElicitationAction, ElicitationSchemaBuilder,
            };
            let prompt = format!(
                "Confirm running confirm-echo with arguments: {}",
                serde_json::to_string(&args_map).unwrap_or_else(|_| "{}".into())
            );
            let schema = ElicitationSchemaBuilder::new()
                .required_string_property("value", |s| s)
                .build_unchecked();
            let response = context
                .peer
                .create_elicitation_with_timeout(
                    CreateElicitationRequestParams::FormElicitationParams {
                        meta: None,
                        message: prompt,
                        requested_schema: schema,
                    },
                    None,
                )
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return match response.action {
                ElicitationAction::Accept => {
                    let answer = response
                        .content
                        .and_then(|v| v.get("value").and_then(|x| x.as_str()).map(str::to_string))
                        .unwrap_or_else(|| "accepted".into());
                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "confirmed: {answer}"
                    ))]))
                }
                ElicitationAction::Decline | ElicitationAction::Cancel => Ok(
                    CallToolResult::success(vec![Content::text("elicitation declined")]),
                ),
            };
        }

        if let Some(ref handler) = self.in_process_handler {
            let name = params.name.to_string();
            let args = args_map;
            let result = if self.catch_in_process_panics {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&name, args)))
            } else {
                Ok(handler(&name, args))
            };
            return match result {
                Ok(Ok(output)) => Ok(call_tool_result_from_output(output)),
                Ok(Err(error)) => Ok(call_tool_result_from_tool_error(error)),
                Err(panic_payload) => Ok(call_tool_result_from_panic(panic_payload.as_ref())),
            };
        }

        if let Some(ref exe) = self.executable_path {
            let schema: crate::ClapSchema = match serde_json::from_str(&self.schema_json) {
                Ok(schema) => schema,
                Err(_) => return Ok(schema_parse_failure_result()),
            };
            if let Err(e) = crate::validate_required_args(&schema, &params.name, &args_map) {
                return Ok(call_tool_result_from_tool_error(ClapMcpToolError::text(e)));
            }
            let mut cmd =
                build_execution_command(exe, &schema, &self.root_name, &params.name, &args_map);
            match cmd.output() {
                Ok(output) => {
                    if let Some(log_params) = subprocess_stderr_log_params(
                        &params.name,
                        &String::from_utf8_lossy(&output.stderr),
                    ) {
                        let _ = notify_log(&context.peer, log_params).await;
                    }
                    return Ok(call_tool_result_from_subprocess_output(&output));
                }
                Err(error) => return Ok(command_launch_failure_result(&error)),
            }
        }

        Ok(placeholder_tool_result(&params.name, &args_map))
    }
}

/// rmcp MCP server: clap schema tools, resources, prompts, optional tasks.
#[derive(Clone)]
pub(crate) struct ClapMcpServer {
    pub(crate) inner: Arc<ServeHandlerInner>,
    tool_execution_lock: Option<Arc<tokio::sync::Mutex<()>>>,
    log_peer: Arc<Mutex<Option<Peer<RoleServer>>>>,
    processor: Arc<tokio::sync::Mutex<OperationProcessor>>,
}

impl ClapMcpServer {
    fn capture_peer(&self, context: &RequestContext<RoleServer>) {
        if let Ok(mut guard) = self.log_peer.lock()
            && guard.is_none()
        {
            *guard = Some(context.peer.clone());
        }
    }
}

impl ServerHandler for ClapMcpServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        let logging_enabled = self.inner.logging_enabled;
        let task_augmented = self.inner.task_augmented_tools;
        let capabilities = match (logging_enabled, task_augmented) {
            (true, true) => ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .enable_logging()
                .enable_tasks()
                .build(),
            (true, false) => ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .enable_logging()
                .build(),
            (false, true) => ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .enable_tasks()
                .build(),
            (false, false) => ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        };
        let server_info = Implementation::new("clap-mcp", env!("CARGO_PKG_VERSION"))
            .with_title("clap-mcp")
            .with_description("Expose clap CLI schema over MCP (stdio)");
        let mut info = rmcp::model::ServerInfo::new(capabilities).with_server_info(server_info);
        if logging_enabled {
            info = info.with_instructions(LOG_INTERPRETATION_INSTRUCTIONS);
        }
        info
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        async move { Ok(list_resources_result(&self.inner.custom_resources)) }
    }

    fn read_resource(
        &self,
        params: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        let inner = self.inner.clone();
        async move { read_resource_result(&inner.schema_json, &inner.custom_resources, params).await }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        async move { Ok(ListToolsResult::with_all_items(self.inner.tools.clone())) }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        async move {
            Ok(list_prompts_result(
                self.inner.logging_enabled,
                &self.inner.custom_prompts,
            ))
        }
    }

    fn get_prompt(
        &self,
        params: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetPromptResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        let logging_enabled = self.inner.logging_enabled;
        let custom_prompts = self.inner.custom_prompts.clone();
        async move { get_prompt_result(logging_enabled, &custom_prompts, params).await }
    }

    fn call_tool(
        &self,
        params: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        let inner = self.inner.clone();
        let lock = self.tool_execution_lock.clone();
        async move {
            let _guard = if let Some(ref lock) = lock {
                Some(lock.lock().await)
            } else {
                None
            };
            inner.call_tool(&params, &context).await
        }
    }

    fn enqueue_task(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CreateTaskResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        let inner = self.inner.clone();
        let lock = self.tool_execution_lock.clone();
        let processor = self.processor.clone();
        async move {
            if !inner.allows_task_tool(&request.name) {
                return Err(McpError::invalid_params(
                    "task-augmented execution is not enabled for this tool (see list_tools meta.clapMcp.taskAugmented)",
                    None,
                ));
            }

            let task_id = context.id.to_string();
            let descriptor = OperationDescriptor::new(task_id.clone(), request.name.to_string())
                .with_context(context.clone());

            let future_request = request.clone();
            let future_context = context.clone();
            let future_inner = inner.clone();
            let future_lock = lock.clone();
            let task_id_for_body = task_id.clone();

            let catch_panics = future_inner.catch_in_process_panics;
            let task_id_result = task_id_for_body.clone();
            let future = Box::pin(async move {
                let _guard = if let Some(l) = &future_lock {
                    Some(l.lock().await)
                } else {
                    None
                };
                let run_body = async move {
                    crate::logging::run_with_mcp_task_id(task_id_for_body, async move {
                        future_inner
                            .call_tool(&future_request, &future_context)
                            .await
                    })
                    .await
                };
                let result = if catch_panics {
                    match tokio::task::spawn(run_body).await {
                        Ok(r) => r,
                        Err(join_err) if join_err.is_panic() => {
                            Ok(call_tool_result_from_panic(join_err.into_panic().as_ref()))
                        }
                        Err(join_err) => Err(McpError::internal_error(
                            format!("task body join error: {join_err}"),
                            None,
                        )),
                    }
                } else {
                    run_body.await
                };
                Ok(Box::new(ToolCallTaskResult::new(task_id_result, result))
                    as Box<dyn rmcp::task_manager::OperationResultTransport>)
            });

            processor
                .lock()
                .await
                .submit_operation(OperationMessage::new(descriptor, future))
                .map_err(|err| {
                    McpError::internal_error(format!("failed to enqueue task: {err}"), None)
                })?;

            let timestamp = current_timestamp();
            let task = Task::new(task_id, TaskStatus::Working, timestamp.clone(), timestamp)
                .with_status_message("Task accepted");

            Ok(CreateTaskResult::new(task))
        }
    }

    fn get_task_info(
        &self,
        request: GetTaskInfoParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetTaskResult, McpError>> + Send + '_ {
        let processor = self.processor.clone();
        async move {
            let task_id = request.task_id.clone();
            let mut processor = processor.lock().await;

            let completed = processor
                .peek_completed()
                .iter()
                .rev()
                .find(|r| r.descriptor.operation_id == task_id);
            if let Some(completed_result) = completed {
                let status = match &completed_result.result {
                    Ok(boxed) => {
                        if let Some(tool) = boxed.as_any().downcast_ref::<ToolCallTaskResult>() {
                            match &tool.result {
                                Ok(_) => TaskStatus::Completed,
                                Err(_) => TaskStatus::Failed,
                            }
                        } else {
                            TaskStatus::Completed
                        }
                    }
                    Err(_) => TaskStatus::Failed,
                };
                let timestamp = current_timestamp();
                let task = Task::new(task_id, status, timestamp.clone(), timestamp);
                return Ok(GetTaskResult { meta: None, task });
            }

            let running = processor.list_running();
            if running.into_iter().any(|id| id == task_id) {
                let timestamp = current_timestamp();
                let task = Task::new(task_id, TaskStatus::Working, timestamp.clone(), timestamp);
                return Ok(GetTaskResult { meta: None, task });
            }

            Err(McpError::resource_not_found(
                format!("task not found: {task_id}"),
                None,
            ))
        }
    }

    fn get_task_result(
        &self,
        request: GetTaskResultParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetTaskPayloadResult, McpError>> + Send + '_ {
        let processor = self.processor.clone();
        async move {
            let task_id = request.task_id.clone();
            loop {
                {
                    let mut processor = processor.lock().await;
                    if let Some(task_result) = processor.take_completed_result(&task_id) {
                        match task_result.result {
                            Ok(boxed) => {
                                if let Some(tool) =
                                    boxed.as_any().downcast_ref::<ToolCallTaskResult>()
                                {
                                    match &tool.result {
                                        Ok(call_tool) => {
                                            let value =
                                                serde_json::to_value(call_tool).unwrap_or_default();
                                            return Ok(GetTaskPayloadResult::new(value));
                                        }
                                        Err(err) => {
                                            return Err(McpError::internal_error(
                                                format!("task failed: {err}"),
                                                None,
                                            ));
                                        }
                                    }
                                }
                                return Err(McpError::internal_error(
                                    "unsupported task result transport",
                                    None,
                                ));
                            }
                            Err(err) => {
                                return Err(McpError::internal_error(
                                    format!("task execution error: {err}"),
                                    None,
                                ));
                            }
                        }
                    }
                    let running = processor.list_running();
                    if !running.iter().any(|id| id == &task_id) {
                        return Err(McpError::resource_not_found(
                            format!("task not found: {task_id}"),
                            None,
                        ));
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_clap_mcp_server(
    schema_json: String,
    tools: Vec<Tool>,
    executable_path: Option<PathBuf>,
    in_process_handler: Option<InProcessToolHandler>,
    root_name: String,
    config: &ClapMcpConfig,
    serve_options: &ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> Result<ClapMcpServer, ClapMcpError> {
    if metadata.task_augmented_tools {
        if !config.reinvocation_safe {
            return Err(ClapMcpError::InvalidConfig(
                "task_augmented_tools requires reinvocation_safe (in-process execution)".into(),
            ));
        }
        if in_process_handler.is_none() {
            return Err(ClapMcpError::InvalidConfig(
                "task_augmented_tools requires in-process execution (derive with reinvocation_safe and #[clap_mcp_output_from = \"run\"] or equivalent)"
                    .into(),
            ));
        }
    }

    let tool_execution_lock = if config.parallel_safe {
        None
    } else {
        Some(Arc::new(tokio::sync::Mutex::new(())))
    };

    let logging_enabled = serve_options.log_rx.is_some();
    let task_tool_filter = if metadata.task_augmented_tools && !metadata.task_tool_names.is_empty()
    {
        Some(
            metadata
                .task_tool_names
                .iter()
                .cloned()
                .collect::<HashSet<_>>(),
        )
    } else {
        None
    };

    let inner = Arc::new(ServeHandlerInner {
        schema_json,
        tools,
        executable_path,
        in_process_handler,
        root_name,
        catch_in_process_panics: config.catch_in_process_panics,
        custom_resources: serve_options.custom_resources.clone(),
        custom_prompts: serve_options.custom_prompts.clone(),
        logging_enabled,
        task_augmented_tools: metadata.task_augmented_tools,
        task_tool_filter,
        #[cfg(feature = "elicitation")]
        elicitation_enabled: serve_options.elicitation_enabled,
    });

    Ok(ClapMcpServer {
        inner,
        tool_execution_lock,
        log_peer: Arc::new(Mutex::new(None)),
        processor: Arc::new(tokio::sync::Mutex::new(OperationProcessor::new())),
    })
}

pub(crate) fn spawn_log_forwarder(
    server: &ClapMcpServer,
    log_rx: Option<tokio::sync::mpsc::Receiver<LoggingMessageNotificationParams>>,
) {
    if let Some(mut log_rx) = log_rx {
        let log_peer = server.log_peer.clone();
        tokio::spawn(async move {
            while let Some(params) = log_rx.recv().await {
                let peer = log_peer.lock().ok().and_then(|g| g.clone());
                let Some(peer) = peer else {
                    continue;
                };
                let _ = notify_log(&peer, params).await;
            }
        });
    }
}

/// Starts an MCP server over stdio exposing `clap://schema` with the provided JSON payload.
pub(crate) async fn serve_schema_json_over_stdio(
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    mut serve_options: ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> Result<(), ClapMcpError> {
    let schema: crate::ClapSchema = serde_json::from_str(&schema_json)?;
    let tools = crate::tools_from_schema_with_metadata(&schema, &config, metadata);
    let root_name = schema.root.name.clone();

    let server = build_clap_mcp_server(
        schema_json,
        tools,
        executable_path,
        in_process_handler,
        root_name,
        &config,
        &serve_options,
        metadata,
    )?;

    spawn_log_forwarder(&server, serve_options.log_rx.take());

    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| ClapMcpError::Mcp(Box::new(rmcp::RmcpError::ServerInitialize(e))))?;
    service.waiting().await.map_err(ClapMcpError::Join)?;
    Ok(())
}

async fn notify_log(
    peer: &Peer<RoleServer>,
    params: LoggingMessageNotificationParams,
) -> Result<(), McpError> {
    let mut notification = LoggingMessageNotification::new(LoggingMessageNotificationParam {
        level: params.level,
        logger: params.logger,
        data: params.data,
    });
    if let Some(meta_map) = params.meta {
        notification.extensions.insert(Meta(meta_map));
    }
    peer.send_notification(rmcp::model::ServerNotification::LoggingMessageNotification(
        notification,
    ))
    .await
    .map_err(|e: rmcp::ServiceError| match e {
        rmcp::ServiceError::McpError(err) => err,
        other => McpError::internal_error(other.to_string(), None),
    })
}

pub(crate) fn call_tool_result_from_output(output: ClapMcpToolOutput) -> CallToolResult {
    match output {
        ClapMcpToolOutput::Text(text) => CallToolResult::success(vec![Content::text(text)]),
        ClapMcpToolOutput::Structured(value) => {
            let json_text =
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            let mut result = CallToolResult::success(vec![Content::text(json_text)]);
            result.structured_content = Some(value);
            result
        }
    }
}

pub(crate) fn call_tool_result_from_tool_error(error: ClapMcpToolError) -> CallToolResult {
    let mut result = CallToolResult::error(vec![Content::text(error.message)]);
    result.structured_content = error.structured.clone();
    result
}

pub(crate) fn call_tool_result_from_panic(
    panic_payload: &(dyn std::any::Any + Send),
) -> CallToolResult {
    let msg = crate::format_panic_payload(panic_payload);
    CallToolResult::error(vec![Content::text(format!("Tool panicked: {msg}"))])
}

pub(crate) fn schema_parse_failure_result() -> CallToolResult {
    CallToolResult::error(vec![Content::text("Failed to parse schema")])
}

pub(crate) fn command_launch_failure_result(error: &std::io::Error) -> CallToolResult {
    CallToolResult::error(vec![Content::text(format!(
        "Failed to run command: {error}"
    ))])
}

pub(crate) fn placeholder_tool_result(
    name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> CallToolResult {
    let args_json = serde_json::Value::Object(arguments.clone());
    CallToolResult::success(vec![Content::text(format!(
        "Would invoke clap command '{name}' with arguments: {args_json:?}"
    ))])
}

pub(crate) fn build_execution_command(
    executable_path: &std::path::Path,
    schema: &crate::ClapSchema,
    root_name: &str,
    tool_name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> std::process::Command {
    let argv = crate::build_tool_argv(schema, tool_name, arguments.clone());
    let mut command = std::process::Command::new(executable_path);
    if let Some(path) = crate::command_path(schema, tool_name) {
        for segment in path.into_iter().skip(1) {
            command.arg(segment);
        }
    } else if tool_name != root_name {
        command.arg(tool_name);
    }
    for arg in &argv {
        command.arg(arg);
    }
    command
}

pub(crate) fn subprocess_stderr_log_params(
    tool_name: &str,
    stderr: &str,
) -> Option<LoggingMessageNotificationParams> {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut meta = serde_json::Map::new();
    meta.insert(
        "tool".to_string(),
        serde_json::Value::String(tool_name.to_string()),
    );
    Some(LoggingMessageNotificationParams {
        data: serde_json::Value::String(trimmed.to_string()),
        level: LoggingLevel::Info,
        logger: Some("stderr".to_string()),
        meta: Some(meta),
    })
}

pub(crate) fn call_tool_result_from_subprocess_output(
    output: &std::process::Output,
) -> CallToolResult {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let code = output
            .status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let mut msg = format!("Tool process exited with non-zero status (code: {code})");
        if !stderr.is_empty() {
            msg.push_str("\nstderr:\n");
            msg.push_str(stderr.trim());
        }
        return CallToolResult::error(vec![Content::text(msg)]);
    }
    let text = if stderr.is_empty() {
        stdout.trim().to_string()
    } else {
        format!("{}\nstderr:\n{}", stdout.trim(), stderr.trim())
    };
    CallToolResult::success(vec![Content::text(text)])
}

pub(crate) fn validate_tool_argument_names(
    tool: &Tool,
    tool_name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), McpError> {
    let schema = tool.input_schema.as_ref();
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for key in arguments.keys() {
            if !props.contains_key(key) {
                return Err(McpError::invalid_params(
                    format!("unknown argument: {key} (tool: {tool_name})"),
                    None,
                ));
            }
        }
    }
    Ok(())
}

fn clap_schema_resource() -> Resource {
    Resource::new(
        RawResource::new(MCP_RESOURCE_URI_SCHEMA, "clap-schema")
            .with_title("Clap CLI schema")
            .with_description("JSON schema extracted from clap Command definitions")
            .with_mime_type("application/json"),
        None,
    )
}

pub(crate) fn list_resources_result(
    custom_resources: &[content::CustomResource],
) -> ListResourcesResult {
    let mut resources = vec![clap_schema_resource()];
    for resource in custom_resources {
        resources.push(resource.to_list_resource());
    }
    ListResourcesResult {
        resources,
        meta: None,
        next_cursor: None,
    }
}

pub(crate) async fn read_resource_result(
    schema_json: &str,
    custom_resources: &[content::CustomResource],
    params: ReadResourceRequestParams,
) -> Result<ReadResourceResult, McpError> {
    if params.uri == MCP_RESOURCE_URI_SCHEMA {
        return Ok(ReadResourceResult::new(vec![
            ResourceContents::TextResourceContents {
                uri: params.uri,
                mime_type: Some("application/json".into()),
                text: schema_json.to_string(),
                meta: None,
            },
        ]));
    }
    let custom = custom_resources
        .iter()
        .find(|resource| resource.uri == params.uri);
    let Some(resource) = custom else {
        return Err(McpError::invalid_params(
            format!("unknown resource uri: {}", params.uri),
            None,
        ));
    };
    let text = content::resolve_resource_content(resource, &params.uri).await?;
    Ok(ReadResourceResult::new(vec![
        ResourceContents::TextResourceContents {
            uri: params.uri.clone(),
            mime_type: resource.mime_type.clone(),
            text,
            meta: None,
        },
    ]))
}

fn logging_guide_prompt() -> rmcp::model::Prompt {
    rmcp::model::Prompt::new(
        PROMPT_LOGGING_GUIDE,
        Some("How to interpret log messages from this clap-mcp server"),
        None,
    )
    .with_title("clap-mcp Logging Guide")
}

pub(crate) fn list_prompts_result(
    logging_enabled: bool,
    custom_prompts: &[content::CustomPrompt],
) -> ListPromptsResult {
    let mut prompts = Vec::new();
    if logging_enabled {
        prompts.push(logging_guide_prompt());
    }
    for prompt in custom_prompts {
        prompts.push(prompt.to_list_prompt());
    }
    ListPromptsResult {
        prompts,
        meta: None,
        next_cursor: None,
    }
}

pub(crate) async fn get_prompt_result(
    logging_enabled: bool,
    custom_prompts: &[content::CustomPrompt],
    params: GetPromptRequestParams,
) -> Result<GetPromptResult, McpError> {
    if params.name == PROMPT_LOGGING_GUIDE {
        if !logging_enabled {
            return Err(McpError::invalid_params(
                format!("unknown prompt: {}", params.name),
                None,
            ));
        }
        return Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            LOGGING_GUIDE_CONTENT,
        )])
        .with_description("How to interpret log messages from this clap-mcp server"));
    }
    let custom = custom_prompts
        .iter()
        .find(|prompt| prompt.name == params.name);
    let Some(prompt) = custom else {
        return Err(McpError::invalid_params(
            format!("unknown prompt: {}", params.name),
            None,
        ));
    };
    let arguments = params.arguments.clone().unwrap_or_default();
    let messages = content::resolve_prompt_content(prompt, &params.name, &arguments).await?;
    let mut result = GetPromptResult::new(messages);
    if let Some(description) = prompt.description.clone() {
        result = result.with_description(description);
    }
    Ok(result)
}
