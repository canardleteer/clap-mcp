//! Builder and internal bundle for imperative MCP embedder serve.

#[cfg(feature = "http")]
use crate::http;
use crate::{
    ClapMcpConfig, ClapMcpConfigProvider, ClapMcpError, ClapMcpSchemaMetadata,
    ClapMcpSchemaMetadataProvider, ClapMcpServeOptions, ClapMcpToolExecutor,
    ClapMcpToolExecutorWithState, InProcessToolHandler, McpListen, build_mcp_blocking_runtime,
    make_in_process_handler, make_in_process_handler_with_state, schema_from_command_with_metadata,
    server,
};
use clap::CommandFactory;
use std::path::PathBuf;
use std::sync::Arc;

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
        let metadata = T::clap_mcp_schema_metadata();
        let base_cmd = T::command();
        let schema = schema_from_command_with_metadata(&base_cmd, &metadata);
        let schema_json = serde_json::to_string_pretty(&schema).expect("schema should serialize");

        let in_process_handler = if config.reinvocation_safe {
            #[cfg(unix)]
            let capture_stdout = false;
            #[cfg(not(unix))]
            let capture_stdout = false;
            Some(make_in_process_handler::<T>(schema, capture_stdout))
        } else {
            None
        };

        let executable_path = if config.reinvocation_safe {
            None
        } else {
            std::env::current_exe().ok()
        };

        Self {
            listen: Some(listen),
            schema_json: Some(schema_json),
            config: Some(config),
            metadata: Some(metadata),
            serve_options: ClapMcpServeOptions::default(),
            executable_path,
            in_process_handler,
        }
    }

    /// Like [`Self::for_cli`], but captures shared `state` in the in-process handler.
    pub fn for_cli_with_state<T, S>(listen: McpListen, state: Arc<S>) -> Self
    where
        T: ClapMcpToolExecutorWithState<S>
            + ClapMcpSchemaMetadataProvider
            + ClapMcpConfigProvider
            + CommandFactory
            + clap::FromArgMatches
            + 'static,
        S: Send + Sync + 'static,
    {
        let config = T::clap_mcp_config();
        let metadata = T::clap_mcp_schema_metadata();
        let base_cmd = T::command();
        let schema = schema_from_command_with_metadata(&base_cmd, &metadata);
        let schema_json = serde_json::to_string_pretty(&schema).expect("schema should serialize");

        let in_process_handler = if config.reinvocation_safe {
            #[cfg(unix)]
            let capture_stdout = false;
            #[cfg(not(unix))]
            let capture_stdout = false;
            Some(make_in_process_handler_with_state::<T, S>(
                schema,
                state,
                capture_stdout,
            ))
        } else {
            None
        };

        let executable_path = if config.reinvocation_safe {
            None
        } else {
            std::env::current_exe().ok()
        };

        Self {
            listen: Some(listen),
            schema_json: Some(schema_json),
            config: Some(config),
            metadata: Some(metadata),
            serve_options: ClapMcpServeOptions::default(),
            executable_path,
            in_process_handler,
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

    /// Optional serve behavior (logging, custom resources, etc.).
    pub fn serve_options(mut self, serve_options: ClapMcpServeOptions) -> Self {
        self.serve_options = serve_options;
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

    /// Validate required fields and return a [`ServeMcp`] bundle.
    pub fn build(self) -> Result<ServeMcp, ClapMcpError> {
        let listen = self.listen.ok_or_else(|| missing_field("listen"))?;
        let schema_json = self
            .schema_json
            .ok_or_else(|| missing_field("schema_json"))?;
        let config = self.config.ok_or_else(|| missing_field("config"))?;
        let metadata = self.metadata.ok_or_else(|| missing_field("metadata"))?;
        Ok(ServeMcp {
            listen,
            schema_json,
            executable_path: self.executable_path,
            config,
            in_process_handler: self.in_process_handler,
            serve_options: self.serve_options,
            metadata,
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
