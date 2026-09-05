//! Builder and internal bundle for imperative MCP embedder serve.

#[cfg(feature = "http")]
use crate::http;
use crate::{
    ClapMcpConfig, ClapMcpConfigProvider, ClapMcpError, ClapMcpSchemaMetadata,
    ClapMcpSchemaMetadataProvider, ClapMcpServeOptions, ClapMcpToolExecutor,
    ClapMcpToolExecutorWithState, InProcessToolHandler, McpListen, SubprocessStderr,
    build_mcp_blocking_runtime, prepare_derive_mcp_serve, prepare_derive_mcp_serve_with_state,
    server,
};
use clap::CommandFactory;
use std::{path::PathBuf, pin::Pin, sync::Arc};
use tokio::io::{AsyncRead, AsyncWrite};

/// Stdio transport source for [`ServeMcpBuilder`] (`McpListen::Stdio` only).
#[derive(Default)]
pub(crate) enum McpStdioIo {
    /// Process stdin/stdout (default).
    #[default]
    Process,
    /// Caller-supplied async read/write pair (for example a JSON-RPC multiplexer).
    Custom {
        read: Pin<Box<dyn AsyncRead + Send + Unpin>>,
        write: Pin<Box<dyn AsyncWrite + Send + Unpin>>,
    },
}

/// Bundled MCP serve parameters, produced by [`ServeMcpBuilder::build`].
///
/// Most callers use [`ServeMcpBuilder::serve`] or [`ServeMcpBuilder::serve_blocking`]
/// directly; build a [`ServeMcp`] when you need to validate once and serve later.
pub struct ServeMcp {
    listen: McpListen,
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: ClapMcpServeOptions,
    metadata: ClapMcpSchemaMetadata,
    stdio_io: McpStdioIo,
}

impl ServeMcp {
    /// Run MCP on the caller's tokio runtime (stdio or HTTP).
    pub async fn serve(self) -> Result<(), ClapMcpError> {
        validate_embedder_runtime(&self.config)?;
        match self.listen {
            McpListen::Stdio => {
                server::serve_schema_json_over_stdio(
                    self.schema_json,
                    self.executable_path,
                    self.config,
                    self.in_process_handler,
                    self.serve_options,
                    &self.metadata,
                    self.stdio_io,
                )
                .await
            }
            #[cfg(feature = "http")]
            McpListen::Http(addr) => {
                http::serve_schema_json_over_http(
                    addr,
                    self.schema_json,
                    self.executable_path,
                    self.config,
                    self.in_process_handler,
                    self.serve_options,
                    &self.metadata,
                )
                .await
            }
        }
    }

    /// Run MCP on an internally created tokio runtime.
    pub fn serve_blocking(self) -> Result<(), ClapMcpError> {
        build_mcp_blocking_runtime(&self.config)?.block_on(self.serve())
    }
}

/// Fluent builder for imperative MCP embedder serve.
///
/// Prefer [`ServeMcpBuilder::for_cli`] when serving a `#[derive(ClapMcp)]` CLI;
/// use [`ServeMcpBuilder::new`] for hand-built schemas.
///
/// For session state across tool calls, see [`Self::for_cli_with_state`] and
/// [`ClapMcpToolExecutorWithState`].
///
/// # Example (async embedder)
///
/// ```rust,ignore
/// use clap_mcp::{ServeMcpBuilder, McpListen, ClapMcpServeOptions};
///
/// #[tokio::main(flavor = "multi_thread")]
/// async fn main() -> Result<(), clap_mcp::ClapMcpError> {
///     ServeMcpBuilder::for_cli::<Cli>(McpListen::Stdio)
///         .serve_options(serve_options)
///         .serve()
///         .await
/// }
/// ```
///
/// # Example (sync main)
///
/// ```rust,ignore
/// use clap_mcp::{ServeMcpBuilder, McpListen};
///
/// ServeMcpBuilder::new()
///     .listen(McpListen::Stdio)
///     .schema_json(schema_json)
///     .config(ClapMcpConfig::default())
///     .metadata(ClapMcpSchemaMetadata::default())
///     .serve_blocking()?;
/// ```
#[derive(Default)]
pub struct ServeMcpBuilder {
    listen: Option<McpListen>,
    schema_json: Option<String>,
    config: Option<ClapMcpConfig>,
    metadata: Option<ClapMcpSchemaMetadata>,
    serve_options: ClapMcpServeOptions,
    executable_path: Option<PathBuf>,
    in_process_handler: Option<InProcessToolHandler>,
    stdio_io: McpStdioIo,
}

impl ServeMcpBuilder {
    /// Start an empty builder (all required fields must be set before serve).
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-fill schema, config, metadata, handler, and executable path from a derive CLI type.
    pub fn for_cli<T>(listen: McpListen) -> Self
    where
        T: ClapMcpToolExecutor
            + ClapMcpSchemaMetadataProvider
            + ClapMcpConfigProvider
            + CommandFactory
            + clap::FromArgMatches
            + 'static,
    {
        let config = T::clap_mcp_config();
        let prepared = prepare_derive_mcp_serve::<T>(&config, &ClapMcpServeOptions::default());
        Self {
            listen: Some(listen),
            schema_json: Some(prepared.schema_json),
            config: Some(config),
            metadata: Some(prepared.metadata),
            serve_options: ClapMcpServeOptions::default(),
            executable_path: prepared.executable_path,
            in_process_handler: prepared.in_process_handler,
            stdio_io: McpStdioIo::default(),
        }
    }

    /// Like [`Self::for_cli`], but captures shared session [`ClapMcpToolExecutorWithState::State`]
    /// in the in-process handler for the server lifetime.
    ///
    /// Requires `reinvocation_safe` on the derive target. Pass the same `Arc` you would give to
    /// [`crate::ParseOrServeMcpWithState::parse_or_serve_mcp_with_state`].
    ///
    /// Session state is shared for the server process lifetime, not per MCP client. Intended for
    /// localhost or a single trusted operator; see [`ClapMcpToolExecutorWithState`] and
    /// [Security](https://github.com/canardleteer/clap-mcp/blob/main/docs/security.md).
    pub fn for_cli_with_state<T>(listen: McpListen, state: Arc<T::State>) -> Self
    where
        T: ClapMcpToolExecutorWithState
            + ClapMcpSchemaMetadataProvider
            + ClapMcpConfigProvider
            + CommandFactory
            + clap::FromArgMatches
            + 'static,
    {
        let config = T::clap_mcp_config();
        let prepared = prepare_derive_mcp_serve_with_state::<T>(
            &config,
            &ClapMcpServeOptions::default(),
            state,
        );
        Self {
            listen: Some(listen),
            schema_json: Some(prepared.schema_json),
            config: Some(config),
            metadata: Some(prepared.metadata),
            serve_options: ClapMcpServeOptions::default(),
            executable_path: prepared.executable_path,
            in_process_handler: prepared.in_process_handler,
            stdio_io: McpStdioIo::default(),
        }
    }

    /// MCP transport (stdio or HTTP).
    pub fn listen(mut self, listen: McpListen) -> Self {
        self.listen = Some(listen);
        self
    }

    /// Serialized clap schema JSON.
    pub fn schema_json(mut self, schema_json: impl Into<String>) -> Self {
        self.schema_json = Some(schema_json.into());
        self
    }

    /// Execution safety configuration.
    pub fn config(mut self, config: ClapMcpConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Schema metadata (skip, requires, task tools).
    pub fn metadata(mut self, metadata: ClapMcpSchemaMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Optional serve behavior (logging, custom resources, cache hints, etc.).
    ///
    /// Note: This replaces the entire [`ClapMcpServeOptions`] struct on the builder.
    /// Call this before setting individual options (such as [`Self::instructions`],
    /// [`Self::server_info`], or [`Self::tool_annotation`]) if you want individual
    /// builder setters to take precedence over fields in `serve_options`.
    pub fn serve_options(mut self, serve_options: ClapMcpServeOptions) -> Self {
        self.serve_options = serve_options;
        self
    }

    /// SEP-2549 cache hints for list results (and `resources/read` when no read override).
    pub fn cache_hints(mut self, cache_hints: crate::CacheHints) -> Self {
        self.serve_options.cache_hints = cache_hints;
        self
    }

    /// Optional SEP-2549 cache hints for `resources/read` only.
    pub fn resource_read_cache_hints(
        mut self,
        resource_read_cache_hints: Option<crate::CacheHints>,
    ) -> Self {
        self.serve_options.resource_read_cache_hints = resource_read_cache_hints;
        self
    }

    /// Application-provided server instructions advertised in initialize and discover.
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.serve_options.instructions = Some(instructions.into());
        self
    }

    /// Application server implementation identity advertised in initialize and discover.
    pub fn server_info(mut self, server_info: rmcp::model::Implementation) -> Self {
        self.serve_options.server_info = Some(server_info);
        self
    }

    /// Attach annotations to an advertised tool by name.
    pub fn tool_annotation(
        mut self,
        tool_name: impl Into<String>,
        annotations: rmcp::model::ToolAnnotations,
    ) -> Self {
        self.serve_options
            .tool_annotations
            .insert(tool_name.into(), annotations);
        self
    }

    /// Register a custom (schema-only) MCP tool.
    pub fn custom_tool(mut self, tool: rmcp::model::Tool) -> Self {
        self.serve_options.custom_tools.push(tool);
        self
    }

    /// Register custom (schema-only) MCP tools.
    pub fn custom_tools(mut self, tools: impl IntoIterator<Item = rmcp::model::Tool>) -> Self {
        self.serve_options.custom_tools.extend(tools);
        self
    }

    /// Set the subprocess stderr policy.
    pub fn subprocess_stderr(mut self, policy: SubprocessStderr) -> Self {
        self.serve_options.subprocess_stderr = policy;
        self
    }

    /// Set a per-tool output schema override.
    pub fn tool_output_schema(
        mut self,
        tool_name: impl Into<String>,
        schema: serde_json::Value,
    ) -> Self {
        self.serve_options
            .tool_output_schemas
            .insert(tool_name.into(), schema);
        self
    }

    /// Exclude a global argument id from all advertised MCP tool schemas.
    pub fn skip_global_arg(mut self, arg_id: impl Into<String>) -> Self {
        self.serve_options.skip_global_args.push(arg_id.into());
        self
    }

    /// Exclude multiple global argument ids from all advertised MCP tool schemas.
    pub fn skip_global_args(
        mut self,
        arg_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.serve_options
            .skip_global_args
            .extend(arg_ids.into_iter().map(Into::into));
        self
    }

    /// Exclude an argument id from a specific tool's schema.
    pub fn skip_arg(mut self, command_name: impl Into<String>, arg_id: impl Into<String>) -> Self {
        self.serve_options
            .skip_args
            .entry(command_name.into())
            .or_default()
            .push(arg_id.into());
        self
    }

    /// Exclude argument ids from a specific tool's schema.
    pub fn skip_args(
        mut self,
        command_name: impl Into<String>,
        arg_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.serve_options
            .skip_args
            .entry(command_name.into())
            .or_default()
            .extend(arg_ids.into_iter().map(Into::into));
        self
    }

    /// Omit a clap default from the advertised MCP schema for one tool (`"*"` = all).
    pub fn hide_default(
        mut self,
        command_name: impl Into<String>,
        arg_id: impl Into<String>,
    ) -> Self {
        self.serve_options
            .hide_defaults
            .entry(command_name.into())
            .or_default()
            .push(arg_id.into());
        self
    }

    /// Replace the advertised MCP default for one tool argument (`"*"` = all).
    pub fn override_default(
        mut self,
        command_name: impl Into<String>,
        arg_id: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.serve_options
            .override_defaults
            .entry(command_name.into())
            .or_default()
            .insert(arg_id.into(), value);
        self
    }

    /// Subprocess executable for tool calls when not in-process.
    pub fn executable_path(mut self, executable_path: Option<PathBuf>) -> Self {
        self.executable_path = executable_path;
        self
    }

    /// In-process tool handler when `reinvocation_safe`.
    pub fn in_process_handler(mut self, in_process_handler: Option<InProcessToolHandler>) -> Self {
        self.in_process_handler = in_process_handler;
        self
    }

    /// Use custom async I/O for stdio MCP instead of process stdin/stdout.
    ///
    /// Only valid with [`McpListen::Stdio`]. Default is process stdio. Use when
    /// multiplexing MCP over an existing JSON-RPC channel or socket pair.
    pub fn stdio_io<R, W>(mut self, read: R, write: W) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        self.stdio_io = McpStdioIo::Custom {
            read: Box::pin(read),
            write: Box::pin(write),
        };
        self
    }

    /// Validate required fields and return a [`ServeMcp`] bundle.
    pub fn build(self) -> Result<ServeMcp, ClapMcpError> {
        let listen = self.listen.ok_or_else(|| missing_field("listen"))?;
        #[cfg(feature = "http")]
        if matches!(listen, McpListen::Http(_))
            && matches!(self.stdio_io, McpStdioIo::Custom { .. })
        {
            return Err(ClapMcpError::InvalidConfig(
                "ServeMcpBuilder::stdio_io is only valid with McpListen::Stdio".into(),
            ));
        }
        let schema_json = self
            .schema_json
            .ok_or_else(|| missing_field("schema_json"))?;
        let config = self.config.ok_or_else(|| missing_field("config"))?;
        let mut metadata = self.metadata.ok_or_else(|| missing_field("metadata"))?;
        crate::merge_serve_options_into_metadata(&mut metadata, &self.serve_options);
        if !self.serve_options.skip_global_args.is_empty()
            || !self.serve_options.skip_args.is_empty()
        {
            let schema: crate::ClapSchema = serde_json::from_str(&schema_json)?;
            crate::validate_serve_option_skip_ids(&schema, &self.serve_options)?;
        }
        Ok(ServeMcp {
            listen,
            schema_json,
            executable_path: self.executable_path,
            config,
            in_process_handler: self.in_process_handler,
            serve_options: self.serve_options,
            metadata,
            stdio_io: self.stdio_io,
        })
    }

    /// Build and run MCP on the caller's tokio runtime.
    pub async fn serve(self) -> Result<(), ClapMcpError> {
        self.build()?.serve().await
    }

    /// Build and run MCP on an internally created tokio runtime.
    pub fn serve_blocking(self) -> Result<(), ClapMcpError> {
        self.build()?.serve_blocking()
    }
}

fn missing_field(field: &str) -> ClapMcpError {
    ClapMcpError::InvalidConfig(format!("ServeMcpBuilder missing required field `{field}`"))
}

pub(crate) fn validate_embedder_runtime(config: &ClapMcpConfig) -> Result<(), ClapMcpError> {
    if !config.needs_multi_thread_runtime() {
        return Ok(());
    }
    if let Ok(handle) = tokio::runtime::Handle::try_current()
        && handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::CurrentThread
    {
        return Err(ClapMcpError::RequiresMultiThreadRuntime {
            reason: "serve_mcp requires a multi-thread tokio runtime when reinvocation_safe is \
                    true and share_runtime or parallel_safe is enabled; use \
                    #[tokio::main(flavor = \"multi_thread\")] or serve_mcp_blocking"
                .into(),
        });
    }
    Ok(())
}
