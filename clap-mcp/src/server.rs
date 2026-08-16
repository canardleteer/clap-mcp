//! MCP server transport and handler (rmcp `ServerHandler` + stdio).

// Logging types remain functional in rmcp 3.x but are deprecated by SEP-2577.
#![allow(deprecated)]

use crate::{
    CacheHints, ClapMcpConfig, ClapMcpError, ClapMcpSchemaMetadata, ClapMcpSerializeScope,
    ClapMcpServeOptions, ClapMcpToolError, ClapMcpToolOutput, InProcessToolHandler,
    LOG_INTERPRETATION_INSTRUCTIONS, LOGGING_GUIDE_CONTENT, MCP_RESOURCE_URI_SCHEMA,
    PROMPT_LOGGING_GUIDE, content,
    logging::LoggingMessageNotificationParams,
    protocol::{PROTOCOL_VERSION_STABLE, SUPPORTED_PROTOCOL_VERSIONS, negotiate_protocol_version},
    serialize_lock_key,
};
use rmcp::{
    ErrorData as McpError, Peer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams, ContentBlock,
        CreateTaskResult, GetPromptRequestParams, GetPromptResponse, GetPromptResult,
        GetTaskParams, GetTaskResult, Implementation, InitializeRequestParams, ListPromptsResult,
        ListResourceTemplatesResult, ListResourcesResult, ListToolsResult, LoggingLevel,
        LoggingMessageNotification, LoggingMessageNotificationParam, NotificationMetaObject,
        PaginatedRequestParams, PromptMessage, ProtocolVersion, ReadResourceRequestParams,
        ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, Role,
        ServerCapabilities, SetLevelRequestParams, SubscribeRequestParams, Tool,
        UnsubscribeRequestParams, UpdateTaskParams,
    },
    service::{RequestContext, RoleServer, serve_directly},
    task_manager::{TaskExit, TaskManager, TaskOptions},
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
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
    pub custom_resource_templates: Vec<content::CustomResourceTemplate>,
    pub custom_prompts: Vec<content::CustomPrompt>,
    /// Tool names from [`ClapMcpServeOptions::custom_tools`] (schema-only; not clap).
    pub custom_tool_names: HashSet<String>,
    pub cache_hints: CacheHints,
    pub resource_read_cache_hints: Option<CacheHints>,
    pub logging_enabled: bool,
    pub task_augmented_tools: bool,
    pub task_tool_filter: Option<HashSet<String>>,
    pub serialize_tools: HashMap<String, ClapMcpSerializeScope>,
    pub serialize_topic_args: HashMap<String, HashMap<String, crate::SerializeTopicSegmentFn>>,
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

        if self.custom_tool_names.contains(params.name.as_ref()) {
            return Ok(CallToolResult::success(vec![ContentBlock::text(
                "Custom tool acknowledged",
            )]));
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

/// Per-topic mutexes for topical serialization when `parallel_safe` is true.
struct TopicLockRegistry {
    locks: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl TopicLockRegistry {
    fn new() -> Self {
        Self {
            locks: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    async fn acquire(&self, key: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = {
            let mut map = self.locks.lock().await;
            map.entry(key.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        lock.lock_owned().await
    }
}

struct ExecutionGuardContext<'a> {
    parallel_safe: bool,
    global_lock: &'a Option<Arc<tokio::sync::Mutex<()>>>,
    topic_registry: &'a TopicLockRegistry,
    serialize_tools: &'a HashMap<String, ClapMcpSerializeScope>,
    serialize_topic_args: &'a HashMap<String, HashMap<String, crate::SerializeTopicSegmentFn>>,
}

async fn with_execution_guard<F, Fut, T>(
    ctx: &ExecutionGuardContext<'_>,
    tool_name: &str,
    args: &serde_json::Map<String, serde_json::Value>,
    f: F,
) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    if !ctx.parallel_safe {
        if let Some(lock) = ctx.global_lock {
            let _guard = lock.lock().await;
            return f().await;
        }
    } else if let Some(scope) = ctx.serialize_tools.get(tool_name) {
        let topic_fns = ctx.serialize_topic_args.get(tool_name);
        let key = serialize_lock_key(tool_name, args, scope, topic_fns);
        let _guard = ctx.topic_registry.acquire(&key).await;
        return f().await;
    }
    f().await
}

/// rmcp MCP server: clap schema tools, resources, prompts, optional tasks.
#[derive(Clone)]
pub(crate) struct ClapMcpServer {
    pub(crate) inner: Arc<ServeHandlerInner>,
    parallel_safe: bool,
    tool_execution_lock: Option<Arc<tokio::sync::Mutex<()>>>,
    topic_lock_registry: Arc<TopicLockRegistry>,
    log_peer: Arc<Mutex<Option<Peer<RoleServer>>>>,
    task_manager: TaskManager,
    /// URIs accepted via `resources/subscribe`. Bookkeeping only; update
    /// notifications are not emitted.
    subscribed_uris: Arc<Mutex<HashSet<String>>>,
}

impl ClapMcpServer {
    fn capture_peer(&self, context: &RequestContext<RoleServer>) {
        if let Ok(mut guard) = self.log_peer.lock() {
            *guard = Some(context.peer.clone());
        }
    }

    /// Record a resource subscription (no update notifications are sent).
    pub(crate) fn track_resource_subscribe(&self, uri: impl Into<String>) {
        if let Ok(mut guard) = self.subscribed_uris.lock() {
            guard.insert(uri.into());
        }
    }

    /// Remove a resource subscription. Unknown URIs are ignored.
    pub(crate) fn track_resource_unsubscribe(&self, uri: &str) {
        if let Ok(mut guard) = self.subscribed_uris.lock() {
            guard.remove(uri);
        }
    }

    /// Snapshot of currently subscribed resource URIs (test helper).
    #[cfg(test)]
    pub(crate) fn subscribed_resource_uris(&self) -> HashSet<String> {
        self.subscribed_uris
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl ServerHandler for ClapMcpServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        let logging_enabled = self.inner.logging_enabled;
        let task_augmented = self.inner.task_augmented_tools;
        // resources.subscribe is advertised; handlers accept subscribe/unsubscribe
        // but do not emit `notifications/resources/updated`.
        let capabilities = match (logging_enabled, task_augmented) {
            (true, true) => ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_resources_subscribe()
                .enable_prompts()
                .enable_logging()
                .enable_tasks()
                .build(),
            (true, false) => ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_resources_subscribe()
                .enable_prompts()
                .enable_logging()
                .build(),
            (false, true) => ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_resources_subscribe()
                .enable_prompts()
                .enable_tasks()
                .build(),
            (false, false) => ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_resources_subscribe()
                .enable_prompts()
                .build(),
        };
        let server_info = Implementation::new("clap-mcp", env!("CARGO_PKG_VERSION"))
            .with_title("clap-mcp")
            .with_description("Expose clap CLI schema over MCP (stdio)");
        let mut info = rmcp::model::ServerInfo::new(capabilities)
            .with_server_info(server_info)
            .with_protocol_version(PROTOCOL_VERSION_STABLE);
        if logging_enabled {
            info = info.with_instructions(LOG_INTERPRETATION_INSTRUCTIONS);
        }
        info
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(SUPPORTED_PROTOCOL_VERSIONS)
    }

    fn initialize(
        &self,
        mut request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::InitializeResult, McpError>> + Send + '_
    {
        // Restrict negotiation to conformance-tested versions. Stdio uses
        // `serve_directly`, so this handler must negotiate (and stamp peer_info)
        // itself. Streamable HTTP uses `supported_protocol_versions` for
        // discover, initialize, and per-request version checks (rmcp 3.1+).
        let negotiated = negotiate_protocol_version(&request.protocol_version);
        request.protocol_version = negotiated.clone();
        context.peer.set_peer_info(request);
        let mut info = self.get_info();
        info.protocol_version = negotiated;
        std::future::ready(Ok(info))
    }

    fn set_level(
        &self,
        _request: SetLevelRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), McpError>> + Send + '_ {
        self.capture_peer(&context);
        if self.inner.logging_enabled {
            std::future::ready(Ok(()))
        } else {
            std::future::ready(Err(McpError::method_not_found::<
                rmcp::model::SetLevelRequestMethod,
            >()))
        }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        async move {
            Ok(list_resources_result(
                &self.inner.custom_resources,
                self.inner.cache_hints,
            ))
        }
    }

    fn read_resource(
        &self,
        params: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResponse, McpError>> + Send + '_ {
        self.capture_peer(&context);
        let inner = self.inner.clone();
        async move {
            read_resource_result(
                &inner.schema_json,
                &inner.custom_resources,
                &inner.custom_resource_templates,
                inner.resource_read_cache_hints.unwrap_or(inner.cache_hints),
                params,
            )
            .await
            .map(Into::into)
        }
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_
    {
        self.capture_peer(&context);
        async move {
            Ok(list_resource_templates_result(
                &self.inner.custom_resource_templates,
                self.inner.cache_hints,
            ))
        }
    }

    fn subscribe(
        &self,
        request: SubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), McpError>> + Send + '_ {
        self.capture_peer(&context);
        self.track_resource_subscribe(request.uri);
        std::future::ready(Ok(()))
    }

    fn unsubscribe(
        &self,
        request: UnsubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), McpError>> + Send + '_ {
        self.capture_peer(&context);
        self.track_resource_unsubscribe(&request.uri);
        std::future::ready(Ok(()))
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        self.capture_peer(&context);
        async move {
            Ok(self
                .inner
                .cache_hints
                .apply_to_tools(ListToolsResult::with_all_items(self.inner.tools.clone())))
        }
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
                self.inner.cache_hints,
            ))
        }
    }

    fn get_prompt(
        &self,
        params: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetPromptResponse, McpError>> + Send + '_ {
        self.capture_peer(&context);
        let logging_enabled = self.inner.logging_enabled;
        let custom_prompts = self.inner.custom_prompts.clone();
        async move {
            get_prompt_result(logging_enabled, &custom_prompts, params)
                .await
                .map(Into::into)
        }
    }

    fn call_tool(
        &self,
        params: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResponse, McpError>> + Send + '_ {
        self.capture_peer(&context);
        let inner = self.inner.clone();
        let lock = self.tool_execution_lock.clone();
        let parallel_safe = self.parallel_safe;
        let topic_registry = self.topic_lock_registry.clone();
        let serialize_tools = self.inner.serialize_tools.clone();
        let serialize_topic_args = self.inner.serialize_topic_args.clone();
        let task_manager = self.task_manager.clone();
        async move {
            let client_tasks = context
                .client_capabilities()
                .is_some_and(|caps| caps.supports_tasks());
            // SEP-2663: tasks are server-directed. Eligible tools return
            // CreateTaskResult when the client declared the tasks extension.
            if inner.allows_task_tool(&params.name) && client_tasks {
                let future_request = params.clone();
                let future_context = context.clone();
                let future_inner = inner.clone();
                let future_lock = lock.clone();
                let future_parallel_safe = parallel_safe;
                let future_topic_registry = topic_registry.clone();
                let future_serialize_tools = serialize_tools.clone();
                let future_serialize_topic_args = serialize_topic_args.clone();
                let task_args = params.arguments.clone().unwrap_or_default();
                let catch_panics = future_inner.catch_in_process_panics;
                let tool_name = future_request.name.clone();

                let task = task_manager.spawn(
                    TaskOptions::new().with_status_message("Task accepted"),
                    move |task_ctx| {
                        let task_id = task_ctx.task_id().to_string();
                        Box::pin(async move {
                            let guard_ctx = ExecutionGuardContext {
                                parallel_safe: future_parallel_safe,
                                global_lock: &future_lock,
                                topic_registry: &future_topic_registry,
                                serialize_tools: &future_serialize_tools,
                                serialize_topic_args: &future_serialize_topic_args,
                            };
                            let result = with_execution_guard(
                                &guard_ctx,
                                &tool_name,
                                &task_args,
                                || async move {
                                    let run_body = async move {
                                        crate::logging::run_with_mcp_task_id(task_id, async move {
                                            future_inner
                                                .call_tool(&future_request, &future_context)
                                                .await
                                        })
                                        .await
                                    };
                                    if catch_panics {
                                        match tokio::task::spawn(run_body).await {
                                            Ok(r) => r,
                                            Err(join_err) if join_err.is_panic() => {
                                                Ok(call_tool_result_from_panic(
                                                    join_err.into_panic().as_ref(),
                                                ))
                                            }
                                            Err(join_err) => Err(McpError::internal_error(
                                                format!("task body join error: {join_err}"),
                                                None,
                                            )),
                                        }
                                    } else {
                                        run_body.await
                                    }
                                },
                            )
                            .await;
                            match result {
                                Ok(call_tool) => Ok(call_tool),
                                Err(err) => Err(TaskExit::Error(err)),
                            }
                        })
                    },
                );
                return Ok(CreateTaskResult::new(task).into());
            }

            let args = params.arguments.clone().unwrap_or_default();
            let guard_ctx = ExecutionGuardContext {
                parallel_safe,
                global_lock: &lock,
                topic_registry: &topic_registry,
                serialize_tools: &serialize_tools,
                serialize_topic_args: &serialize_topic_args,
            };
            with_execution_guard(&guard_ctx, &params.name, &args, || {
                inner.call_tool(&params, &context)
            })
            .await
            .map(Into::into)
        }
    }

    fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetTaskResult, McpError>> + Send + '_ {
        let task_manager = self.task_manager.clone();
        async move {
            let detailed = task_manager.get_task(&request.task_id)?;
            Ok(GetTaskResult::new(detailed))
        }
    }

    fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), McpError>> + Send + '_ {
        let task_manager = self.task_manager.clone();
        async move {
            task_manager.update_task(&request.task_id, request.input_responses)?;
            Ok(())
        }
    }

    fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), McpError>> + Send + '_ {
        let task_manager = self.task_manager.clone();
        async move {
            task_manager.cancel_task(&request.task_id)?;
            Ok(())
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

    let custom_tool_names: HashSet<String> = serve_options
        .custom_tools
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    let mut tools = tools;
    tools.extend(serve_options.custom_tools.iter().cloned());

    let inner = Arc::new(ServeHandlerInner {
        schema_json,
        tools,
        executable_path,
        in_process_handler,
        root_name,
        catch_in_process_panics: config.catch_in_process_panics,
        custom_resources: serve_options.custom_resources.clone(),
        custom_resource_templates: serve_options.custom_resource_templates.clone(),
        custom_prompts: serve_options.custom_prompts.clone(),
        custom_tool_names,
        cache_hints: serve_options.cache_hints,
        resource_read_cache_hints: serve_options.resource_read_cache_hints,
        logging_enabled,
        task_augmented_tools: metadata.task_augmented_tools,
        task_tool_filter,
        serialize_tools: metadata.serialize_tools.clone(),
        serialize_topic_args: metadata.serialize_topic_args.clone(),
    });

    Ok(ClapMcpServer {
        inner,
        parallel_safe: config.parallel_safe,
        tool_execution_lock,
        topic_lock_registry: Arc::new(TopicLockRegistry::new()),
        log_peer: Arc::new(Mutex::new(None)),
        task_manager: TaskManager::new(),
        subscribed_uris: Arc::new(Mutex::new(HashSet::new())),
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
    stdio_io: crate::serve::McpStdioIo,
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

    use rmcp::transport::IntoTransport;

    // `serve_directly` so stdio `initialize` uses this handler (and its
    // conformance-tested version set) instead of rmcp's default handshake.
    let service = match stdio_io {
        crate::serve::McpStdioIo::Process => serve_directly(server, rmcp::transport::stdio(), None),
        crate::serve::McpStdioIo::Custom { read, write } => {
            serve_directly(server, (read, write).into_transport(), None)
        }
    };
    service.waiting().await.map_err(ClapMcpError::Join)?;
    Ok(())
}

#[allow(deprecated)]
async fn notify_log(
    peer: &Peer<RoleServer>,
    params: LoggingMessageNotificationParams,
) -> Result<(), McpError> {
    let mut param = LoggingMessageNotificationParam::new(params.level, params.data);
    if let Some(logger) = params.logger {
        param = param.with_logger(logger);
    }
    let mut notification = LoggingMessageNotification::new(param);
    if let Some(meta_map) = params.meta {
        notification
            .extensions
            .insert(NotificationMetaObject::from(meta_map));
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
        ClapMcpToolOutput::Text(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
        ClapMcpToolOutput::Structured(value) => {
            let json_text =
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            let mut result = CallToolResult::success(vec![ContentBlock::text(json_text)]);
            result.structured_content = Some(value);
            result
        }
    }
}

pub(crate) fn call_tool_result_from_tool_error(error: ClapMcpToolError) -> CallToolResult {
    let mut result = CallToolResult::error(vec![ContentBlock::text(error.message)]);
    result.structured_content = error.structured.clone();
    result
}

pub(crate) fn call_tool_result_from_panic(
    panic_payload: &(dyn std::any::Any + Send),
) -> CallToolResult {
    let msg = crate::format_panic_payload(panic_payload);
    CallToolResult::error(vec![ContentBlock::text(format!("Tool panicked: {msg}"))])
}

pub(crate) fn schema_parse_failure_result() -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text("Failed to parse schema")])
}

pub(crate) fn command_launch_failure_result(error: &std::io::Error) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(format!(
        "Failed to run command: {error}"
    ))])
}

pub(crate) fn placeholder_tool_result(
    name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> CallToolResult {
    let args_json = serde_json::Value::Object(arguments.clone());
    CallToolResult::success(vec![ContentBlock::text(format!(
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
        return CallToolResult::error(vec![ContentBlock::text(msg)]);
    }
    let text = if stderr.is_empty() {
        stdout.trim().to_string()
    } else {
        format!("{}\nstderr:\n{}", stdout.trim(), stderr.trim())
    };
    CallToolResult::success(vec![ContentBlock::text(text)])
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
    Resource::new(MCP_RESOURCE_URI_SCHEMA, "clap-schema")
        .with_title("Clap CLI schema")
        .with_description("JSON schema extracted from clap Command definitions")
        .with_mime_type("application/json")
}

pub(crate) fn list_resources_result(
    custom_resources: &[content::CustomResource],
    cache_hints: CacheHints,
) -> ListResourcesResult {
    let mut resources = vec![clap_schema_resource()];
    for resource in custom_resources {
        resources.push(resource.to_list_resource());
    }
    cache_hints.apply_to_resources(ListResourcesResult::with_all_items(resources))
}

pub(crate) fn list_resource_templates_result(
    custom_resource_templates: &[content::CustomResourceTemplate],
    cache_hints: CacheHints,
) -> ListResourceTemplatesResult {
    let templates = custom_resource_templates
        .iter()
        .map(|template| template.to_list_resource_template())
        .collect();
    cache_hints.apply_to_resource_templates(ListResourceTemplatesResult::with_all_items(templates))
}

fn resource_contents_from_body(
    body: content::ResolvedResourceBody,
    uri: String,
    mime_type: &Option<String>,
) -> ResourceContents {
    match body {
        content::ResolvedResourceBody::Text(text) => {
            let mut contents = ResourceContents::text(text, uri);
            if let ResourceContents::TextResourceContents {
                mime_type: slot, ..
            } = &mut contents
            {
                *slot = mime_type.clone();
            }
            contents
        }
        content::ResolvedResourceBody::Blob { base64 } => {
            let mut contents = ResourceContents::blob(base64, uri);
            if let Some(mime) = mime_type {
                contents = contents.with_mime_type(mime.clone());
            }
            contents
        }
    }
}

pub(crate) async fn read_resource_result(
    schema_json: &str,
    custom_resources: &[content::CustomResource],
    custom_resource_templates: &[content::CustomResourceTemplate],
    cache_hints: CacheHints,
    params: ReadResourceRequestParams,
) -> Result<ReadResourceResult, McpError> {
    if params.uri == MCP_RESOURCE_URI_SCHEMA {
        return Ok(cache_hints.apply_to_read(ReadResourceResult::new(vec![
            ResourceContents::text(schema_json, params.uri).with_mime_type("application/json"),
        ])));
    }
    if let Some(resource) = custom_resources
        .iter()
        .find(|resource| resource.uri == params.uri)
    {
        let body = content::resolve_resource_content(resource, &params.uri).await?;
        return Ok(cache_hints.apply_to_read(ReadResourceResult::new(vec![
            resource_contents_from_body(body, params.uri, &resource.mime_type),
        ])));
    }
    for template in custom_resource_templates {
        if let Some(captures) = content::match_uri_template(&template.uri_template, &params.uri) {
            let body = content::resolve_template_resource_content(template, &params.uri, &captures)
                .await?;
            return Ok(cache_hints.apply_to_read(ReadResourceResult::new(vec![
                resource_contents_from_body(body, params.uri, &template.mime_type),
            ])));
        }
    }
    // SEP-2164: include the requested URI in error `data` (rmcp upgrades
    // RESOURCE_NOT_FOUND to INVALID_PARAMS for protocol 2026-07-28+).
    Err(McpError::resource_not_found(
        "Resource not found",
        Some(serde_json::json!({ "uri": params.uri })),
    ))
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
    cache_hints: CacheHints,
) -> ListPromptsResult {
    let mut prompts = Vec::new();
    if logging_enabled {
        prompts.push(logging_guide_prompt());
    }
    for prompt in custom_prompts {
        prompts.push(prompt.to_list_prompt());
    }
    cache_hints.apply_to_prompts(ListPromptsResult::with_all_items(prompts))
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
            Role::User,
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
