//! # clap-mcp
//!
//! Expose your [clap](https://docs.rs/clap) CLI as an MCP (Model Context Protocol) server over stdio.
//!
//! ## Quick start
//!
//! Prefer a single `run` function with `#[clap_mcp_output_from = "run"]` so CLI and MCP
//! share one implementation (no duplicated logic).
//!
//! ```rust,ignore
//! use clap::Parser;
//! use clap_mcp::ClapMcp;
//!
//! #[derive(Parser, ClapMcp)]
//! #[clap_mcp(reinvocation_safe, parallel_safe = false)]
//! #[clap_mcp_output_from = "run"]
//! enum Cli {
//!     Greet { #[arg(long)] name: Option<String> },
//! }
//!
//! fn run(cmd: Cli) -> String {
//!     match cmd {
//!         Cli::Greet { name } => format!("Hello, {}!", name.as_deref().unwrap_or("world")),
//!     }
//! }
//!
//! fn main() {
//!     let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
//!     println!("{}", run(cli));
//! }
//! ```
//!
//! Run with `--mcp` to start the MCP server instead of executing the CLI.

use async_trait::async_trait;
use clap::{Arg, ArgAction, Command};
use rust_mcp_sdk::{
    McpServer, StdioTransport, TransportOptions,
    mcp_server::{McpServerOptions, ServerHandler, ToMcpServerHandler, server_runtime},
    schema::{
        CallToolError, CallToolRequestParams, CallToolResult, ContentBlock, GetPromptRequestParams,
        GetPromptResult, Implementation, InitializeResult, LATEST_PROTOCOL_VERSION,
        ListPromptsResult, ListResourcesResult, ListToolsResult, LoggingLevel,
        LoggingMessageNotificationParams, PaginatedRequestParams, Prompt, PromptMessage,
        ReadResourceContent, ReadResourceRequestParams, ReadResourceResult, Resource, Role,
        RpcError, ServerCapabilities, ServerCapabilitiesPrompts, ServerCapabilitiesResources,
        ServerCapabilitiesTools, TextResourceContents, Tool, ToolInputSchema, schema_utils,
    },
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

#[cfg(any(feature = "tracing", feature = "log"))]
pub mod logging;

/// Custom MCP resources and prompts, and skill export.
pub mod content;

#[cfg(feature = "derive")]
pub use clap_mcp_macros::ClapMcp;

/// Convenience macro for struct root + subcommand CLIs: parse root then run.
///
/// Expands to: parse the root with [`parse_or_serve_mcp_attr`], then evaluate the given
/// expression (which can use `args` for the parsed root). Use in `main` so the pattern
/// is one line and hard to forget.
///
/// # Example
///
/// ```rust,ignore
/// fn main() {
///     clap_mcp_main!(Cli, |args| match args.command {
///         None => println!("No subcommand"),
///         Some(cmd) => println!("{}", run(cmd)),
///     });
/// }
/// ```
///
/// For `Result`-returning run logic, use `?` in main or call [`run_or_serve_mcp`].
#[macro_export]
macro_rules! clap_mcp_main {
    ($root:ty, |$args:ident| $run_expr:expr) => {{
        let $args = $crate::parse_or_serve_mcp_attr::<$root>();
        $run_expr
    }};
    ($root:ty, $run_expr:expr) => {{
        macro_rules! __clap_mcp_with_args {
            ($args:ident, $expr:expr) => {{
                let $args = $crate::parse_or_serve_mcp_attr::<$root>();
                $expr
            }};
        }
        __clap_mcp_with_args!(args, $run_expr)
    }};
}

/// Long flag that triggers MCP server mode. Add to your CLI via [`command_with_mcp_flag`].
pub const MCP_FLAG_LONG: &str = "mcp";

/// Long flag that triggers [Agent Skills](https://agentskills.io/specification) export (generates SKILL.md). Add via [`command_with_export_skills_flag`].
pub const EXPORT_SKILLS_FLAG_LONG: &str = "export-skills";

/// URI for the clap schema resource exposed by the MCP server.
pub const MCP_RESOURCE_URI_SCHEMA: &str = "clap://schema";

/// Provides MCP execution safety configuration from `#[clap_mcp(...)]` attributes.
/// Implemented by the `#[derive(ClapMcp)]` macro.
///
/// # Example
///
/// ```rust
/// use clap::Parser;
/// use clap_mcp::ClapMcpConfigProvider;
/// use clap_mcp::ClapMcp;
///
/// #[derive(Debug, Parser, ClapMcp)]
/// #[clap_mcp(reinvocation_safe, parallel_safe = false)]
/// #[clap_mcp_output_from = "run"]
/// enum MyCli { Foo }
///
/// fn run(cmd: MyCli) -> String {
///     match cmd { MyCli::Foo => "ok".to_string() }
/// }
///
/// let config = MyCli::clap_mcp_config();
/// assert!(config.reinvocation_safe);
/// assert!(!config.parallel_safe);
/// ```
pub trait ClapMcpConfigProvider {
    fn clap_mcp_config() -> ClapMcpConfig;
}

/// Provides MCP schema metadata (skip, requires) from `#[clap_mcp(skip)]` and
/// `#[clap_mcp(requires = "arg_name")]` attributes.
///
/// Implemented by the `#[derive(ClapMcp)]` macro. For custom types, implement
/// with `fn clap_mcp_schema_metadata() -> ClapMcpSchemaMetadata { ClapMcpSchemaMetadata::default() }`.
pub trait ClapMcpSchemaMetadataProvider {
    fn clap_mcp_schema_metadata() -> ClapMcpSchemaMetadata;
}

/// Produces the output string for a parsed CLI value.
/// Used for in-process MCP tool execution when `reinvocation_safe` is true.
/// Implemented by the `#[derive(ClapMcp)]` macro via the blanket impl for `ClapMcpToolExecutor`.
pub trait ClapMcpRunnable {
    fn run(self) -> String;
}

/// Error produced when a tool's `run` function returns `Err(e)` (e.g. `Result<O, E>`).
///
/// When your `run` returns `Result<O, E>`, `Err(e)` is converted via [`IntoClapMcpToolError`]
/// into this type. Implement that trait for your error type to get structured JSON in the
/// response when `E: Serialize`.
#[derive(Debug, Clone)]
pub struct ClapMcpToolError {
    /// Human-readable error message for MCP content.
    pub message: String,
    /// Optional structured JSON when `E: Serialize` and [`IntoClapMcpToolError`] provides it.
    pub structured: Option<serde_json::Value>,
}

impl ClapMcpToolError {
    /// Create a plain text error.
    pub fn text(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            structured: None,
        }
    }

    /// Create an error with structured serialization.
    pub fn structured(message: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            message: message.into(),
            structured: Some(value),
        }
    }
}

impl From<String> for ClapMcpToolError {
    fn from(s: String) -> Self {
        Self::text(s)
    }
}

impl From<&str> for ClapMcpToolError {
    fn from(s: &str) -> Self {
        Self::text(s)
    }
}

/// Converts the return value of a `run` function (used with `#[clap_mcp_output_from]`) into
/// MCP tool output or error.
///
/// Implemented for:
/// - `String` / `&str` → text output
/// - [`AsStructured`]`<T>` where `T: Serialize` → structured JSON output
/// - `Option<O>` → `None` → empty text; `Some(o)` → `o.into_tool_result()`
/// - `Result<O, E>` → `Ok(o)` → output; `Err(e)` → `ClapMcpToolError`
///
/// `Result<AsStructured<T>, E>` is fully supported as a `run` return type; use it when you want
/// structured success payloads and a separate error type.
pub trait IntoClapMcpResult {
    fn into_tool_result(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError>;
}

impl IntoClapMcpResult for String {
    fn into_tool_result(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError> {
        Ok(ClapMcpToolOutput::Text(self))
    }
}

impl IntoClapMcpResult for &str {
    fn into_tool_result(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError> {
        Ok(ClapMcpToolOutput::Text(self.to_string()))
    }
}

/// Wrapper for structured (JSON) output when using `#[clap_mcp_output_from]`.
/// Use when your `run` function returns a type that implements `Serialize` but is not `String`/`&str`.
///
/// Fully supported when used as the `Ok` type in `Result<AsStructured<T>, E>`; there are no known
/// limitations for mixed success/error types. [`IntoClapMcpResult`] is implemented for
/// `AsStructured<T>` where `T: Serialize`.
///
/// # Example
///
/// ```rust,ignore
/// fn run(cmd: Cli) -> Result<clap_mcp::AsStructured<SubcommandResult>, Error> {
///     match cmd { ... }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AsStructured<T>(pub T);

impl<T: Serialize> IntoClapMcpResult for AsStructured<T> {
    fn into_tool_result(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError> {
        serde_json::to_value(&self.0)
            .map(ClapMcpToolOutput::Structured)
            .map_err(|e| ClapMcpToolError::text(e.to_string()))
    }
}

impl<O: IntoClapMcpResult> IntoClapMcpResult for Option<O> {
    fn into_tool_result(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError> {
        match self {
            None => Ok(ClapMcpToolOutput::Text(String::new())),
            Some(o) => o.into_tool_result(),
        }
    }
}

/// Converts an error type from a `run` function into [`ClapMcpToolError`].
/// Used when `run` returns `Result<O, E>` and the `Err` branch is taken.
///
/// Implement this for your error type when you need custom formatting or structured errors.
/// For plain string errors, you can use `String` or `&str`, which have built-in impls.
pub trait IntoClapMcpToolError {
    fn into_tool_error(self) -> ClapMcpToolError;
}

impl IntoClapMcpToolError for String {
    fn into_tool_error(self) -> ClapMcpToolError {
        ClapMcpToolError::text(self)
    }
}

impl IntoClapMcpToolError for &str {
    fn into_tool_error(self) -> ClapMcpToolError {
        ClapMcpToolError::text(self.to_string())
    }
}

impl<O: IntoClapMcpResult, E: IntoClapMcpToolError> IntoClapMcpResult for Result<O, E> {
    fn into_tool_result(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError> {
        match self {
            Ok(o) => o.into_tool_result(),
            Err(e) => Err(e.into_tool_error()),
        }
    }
}

/// Runs a closure with stdout captured. Returns `(result, captured_stdout)`.
/// Unix-only; on Windows returns empty captured string.
#[cfg(unix)]
fn run_with_stdout_capture<R, F>(f: F) -> (R, String)
where
    F: FnOnce() -> R,
{
    use std::io::{Read, Write};
    use std::os::unix::io::FromRawFd;

    // SAFETY: We use a pipe and dup2 to temporarily redirect stdout. All fds are either
    // created by pipe()/dup() or are well-known (STDOUT_FILENO). We close or restore every
    // fd on every path (success or error); from_raw_fd(read_fd) takes ownership of read_fd
    // so it is not double-closed. No fd is used after being closed.
    let mut fds: [libc::c_int; 2] = [0, 0];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return (f(), String::new());
    }
    let (read_fd, write_fd) = (fds[0], fds[1]);

    let stdout_fd = libc::STDOUT_FILENO;
    let saved_stdout = unsafe { libc::dup(stdout_fd) };
    if saved_stdout < 0 {
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
        return (f(), String::new());
    }

    if unsafe { libc::dup2(write_fd, stdout_fd) } < 0 {
        unsafe {
            libc::close(saved_stdout);
            libc::close(read_fd);
            libc::close(write_fd);
        }
        return (f(), String::new());
    }

    let result = f();

    let _ = std::io::stdout().flush();
    unsafe {
        libc::dup2(saved_stdout, stdout_fd);
        libc::close(saved_stdout);
        libc::close(write_fd);
    }

    let mut reader = unsafe { std::fs::File::from_raw_fd(read_fd) };
    let mut captured = String::new();
    let _ = reader.read_to_string(&mut captured);

    (result, captured)
}

#[cfg(not(unix))]
fn run_with_stdout_capture<R, F>(f: F) -> (R, String)
where
    F: FnOnce() -> R,
{
    (f(), String::new())
}

/// Output produced by a CLI command for MCP tool results.
///
/// Use `Text` for plain string output; use `Structured` for serializable JSON
/// (e.g. when using `#[clap_mcp_output_from = "run"]` with `AsStructured<T>`, or
/// (e.g. when using `#[clap_mcp_output_from = "run"]` with `AsStructured<T>`).
///
/// # Example
///
/// ```
/// use clap_mcp::ClapMcpToolOutput;
///
/// let text = ClapMcpToolOutput::Text("hello".into());
/// assert_eq!(text.into_string(), "hello");
///
/// let structured = ClapMcpToolOutput::Structured(serde_json::json!({"x": 1}));
/// assert!(structured.as_structured().unwrap().get("x").is_some());
/// ```
#[derive(Debug, Clone)]
pub enum ClapMcpToolOutput {
    /// Plain text output (stdout-style).
    Text(String),
    /// Structured JSON output for machine consumption.
    Structured(serde_json::Value),
}

impl ClapMcpToolOutput {
    /// Returns the text content if this is `Text`, or the JSON string if `Structured`.
    ///
    /// # Example
    ///
    /// ```
    /// use clap_mcp::ClapMcpToolOutput;
    ///
    /// assert_eq!(ClapMcpToolOutput::Text("hi".into()).into_string(), "hi");
    /// assert!(ClapMcpToolOutput::Structured(serde_json::json!({"a":1})).into_string().contains("a"));
    /// ```
    pub fn into_string(self) -> String {
        match self {
            ClapMcpToolOutput::Text(s) => s,
            ClapMcpToolOutput::Structured(v) => {
                serde_json::to_string(&v).unwrap_or_else(|_| v.to_string())
            }
        }
    }

    /// Returns `Some(&str)` for `Text`, `None` for `Structured`.
    ///
    /// # Example
    ///
    /// ```
    /// use clap_mcp::ClapMcpToolOutput;
    ///
    /// assert_eq!(ClapMcpToolOutput::Text("hi".into()).as_text(), Some("hi"));
    /// assert!(ClapMcpToolOutput::Structured(serde_json::json!(1)).as_text().is_none());
    /// ```
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ClapMcpToolOutput::Text(s) => Some(s),
            ClapMcpToolOutput::Structured(_) => None,
        }
    }

    /// Returns `Some(&Value)` for `Structured`, `None` for `Text`.
    ///
    /// # Example
    ///
    /// ```
    /// use clap_mcp::ClapMcpToolOutput;
    ///
    /// let v = serde_json::json!({"sum": 10});
    /// assert_eq!(ClapMcpToolOutput::Structured(v.clone()).as_structured(), Some(&v));
    /// assert!(ClapMcpToolOutput::Text("x".into()).as_structured().is_none());
    /// ```
    pub fn as_structured(&self) -> Option<&serde_json::Value> {
        match self {
            ClapMcpToolOutput::Text(_) => None,
            ClapMcpToolOutput::Structured(v) => Some(v),
        }
    }
}

/// Produces MCP tool output (text or structured) for a parsed CLI value.
///
/// Implemented by the `#[derive(ClapMcp)]` macro. Used for in-process execution.
///
/// When using **`#[clap_mcp_output_from = "run"]`** on the enum (required), the macro
/// implements this trait by calling `run(self)` and converting the result via [`IntoClapMcpResult`].
/// CLI and MCP share a single implementation.
pub trait ClapMcpToolExecutor {
    fn execute_for_mcp(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError>;
}

impl<T: ClapMcpToolExecutor> ClapMcpRunnable for T {
    fn run(self) -> String {
        self.execute_for_mcp()
            .unwrap_or_else(|e| ClapMcpToolOutput::Text(e.message))
            .into_string()
    }
}

/// Errors that can occur when running the MCP server.
#[derive(Debug, thiserror::Error)]
pub enum ClapMcpError {
    #[error("failed to serialize clap schema to JSON: {0}")]
    SchemaJson(#[from] serde_json::Error),
    #[error("MCP transport error: {0}")]
    Transport(#[from] rust_mcp_sdk::TransportError),
    #[error("MCP runtime error: {0}")]
    McpSdk(#[from] rust_mcp_sdk::error::McpSdkError),
    #[error("I/O error during skill export: {0}")]
    Io(#[from] std::io::Error),
    #[error("tokio runtime context: {0}")]
    RuntimeContext(String),
    #[error("async tool thread panicked or failed: {0}")]
    ToolThread(String),
}

/// Configuration for execution safety when exposing a CLI over MCP.
///
/// Use this to declare whether your CLI tool can be safely invoked multiple times,
/// whether it can run in parallel with other tool calls, and how async tools run.
///
/// # Crash and panic behavior
///
/// - **Subprocess (`reinvocation_safe` = false):** If the tool process exits with a non-zero
///   status, the server returns an MCP tool result with `is_error: true` and a message
///   that includes the exit code (and stderr when non-empty).
/// - **In-process (`reinvocation_safe` = true), `catch_in_process_panics` = false:** Any panic
///   in tool code (including from [`run_async_tool`]) crashes the server.
/// - **In-process, `catch_in_process_panics` = true:** Panics are caught and returned as an
///   MCP error; the server stays up. After a caught panic, the process may no longer be
///   reinvocation_safe (global state may be corrupted); consider restarting the server.
///
/// # Example
///
/// ```
/// use clap_mcp::ClapMcpConfig;
///
/// // Default: subprocess per call, serialized
/// let config = ClapMcpConfig::default();
///
/// // In-process, parallel-safe
/// let config = ClapMcpConfig {
///     reinvocation_safe: true,
///     parallel_safe: true,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ClapMcpConfig {
    /// If true, the CLI can be invoked multiple times without tearing down the process.
    /// When false (default), each tool call spawns a fresh subprocess.
    /// When true, uses in-process execution (no subprocess).
    pub reinvocation_safe: bool,

    /// If true, tool calls may run concurrently. When false, calls are serialized.
    /// Default is false (serialize by default) for safety.
    pub parallel_safe: bool,

    /// When `reinvocation_safe` is true, controls how async tool execution runs.
    /// Only applies to in-process execution; ignored when `reinvocation_safe` is false.
    ///
    /// | Value | Behavior | When to use |
    /// |-------|----------|-------------|
    /// | `false` (default) | Dedicated thread with its own tokio runtime per tool call. No nesting, no special setup. | **Recommended.** Use unless you need deep integration. |
    /// | `true` | Shares the MCP server's tokio runtime. Uses a multi-thread runtime so `block_on` can run async work. | Advanced: share runtime state, spawn long-lived tasks, or integrate with other async code. |
    ///
    /// Use with [`run_async_tool`] in `#[clap_mcp_output]` for async subcommands.
    pub share_runtime: bool,

    /// When true and `reinvocation_safe` is true, panics in tool code are caught and returned
    /// as an MCP error (`is_error: true`) instead of crashing the server. Default is `false` (opt-in).
    ///
    /// **Warning:** After a caught panic, the process may no longer be reinvocation_safe: global
    /// state (e.g. static or process-wide resources) could be left in an inconsistent state.
    /// For reliability, restart the MCP server after a caught panic when using in-process execution.
    pub catch_in_process_panics: bool,

    /// When true (default), `myapp --mcp` starts the MCP server even when the root has
    /// `subcommand_required = true`, by checking argv before calling clap. Set to false to
    /// require a subcommand (and thus `Option<Commands>` + `subcommand_required = false`) for
    /// `--mcp` to parse.
    pub allow_mcp_without_subcommand: bool,
}

impl Default for ClapMcpConfig {
    fn default() -> Self {
        Self {
            reinvocation_safe: false,
            parallel_safe: false,
            share_runtime: false,
            catch_in_process_panics: false,
            allow_mcp_without_subcommand: true,
        }
    }
}

/// Optional configuration for MCP serve behavior (logging, etc.).
///
/// Pass to [`serve_schema_json_over_stdio`] or [`serve_schema_json_over_stdio_blocking`].
/// When `log_rx` is set, enables the logging capability and forwards messages to the MCP client.
///
/// # Example
///
/// ```rust,ignore
/// use clap_mcp::{ClapMcpServeOptions, logging::log_channel};
///
/// let (log_tx, log_rx) = log_channel(32);
/// let mut opts = ClapMcpServeOptions::default();
/// opts.log_rx = Some(log_rx);
/// // Pass opts to parse_or_serve_mcp_with_config_and_options or serve_schema_json_over_stdio_blocking
/// ```
#[derive(Debug, Default)]
pub struct ClapMcpServeOptions {
    /// When set, log messages received on this channel are forwarded to the MCP client
    /// via `notifications/message`. Enables the logging capability and instructions.
    pub log_rx: Option<tokio::sync::mpsc::Receiver<LoggingMessageNotificationParams>>,

    /// When true and running in-process, capture stdout written during tool execution
    /// and merge it with Text output. Only has effect when `reinvocation_safe` is true.
    /// Unix only; **not available on Windows** (this field does not exist there; code
    /// setting it will fail to compile on Windows).
    #[cfg(unix)]
    pub capture_stdout: bool,

    /// Custom MCP resources (static or async dynamic). Merged with the built-in `clap://schema` resource.
    pub custom_resources: Vec<content::CustomResource>,

    /// Custom MCP prompts (static or async dynamic). Merged with the built-in logging guide when logging is enabled.
    pub custom_prompts: Vec<content::CustomPrompt>,
}

/// Log interpretation hint for MCP clients (included in `instructions` when logging is enabled).
///
/// When changing logging behavior (logger names in `logging`, subprocess stderr handling below),
/// update this and [`LOGGING_GUIDE_CONTENT`].
pub const LOG_INTERPRETATION_INSTRUCTIONS: &str = r#"When this server emits log messages (notifications/message), the `logger` field indicates the source:
- "stderr": Subprocess stderr (CLI tools run as subprocesses)
- "app": In-process application logs
- Other: Application-defined logger names"#;

/// Name of the logging guide prompt.
pub const PROMPT_LOGGING_GUIDE: &str = "clap-mcp-logging-guide";

/// Full content for the logging guide prompt (returned when clients request `PROMPT_LOGGING_GUIDE`).
///
/// When changing logging behavior (logger names in `logging`, subprocess stderr handling below),
/// update this and [`LOG_INTERPRETATION_INSTRUCTIONS`].
pub const LOGGING_GUIDE_CONTENT: &str = r#"# clap-mcp Logging Guide

When this server emits log messages (notifications/message), use the `logger` field to interpret the source:

- **"stderr"**: Output from subprocess stderr (CLI tools run as subprocesses). The `meta` field may include `tool` for the command name.
- **"app"**: In-process application logs.
- **Other**: Application-defined logger names.

The `level` field uses RFC 5424 syslog severity: debug, info, notice, warning, error, critical, alert, emergency.
The `data` field contains the message (string or JSON object)."#;

/// Metadata for filtering and adjusting the MCP schema.
///
/// Use with [`schema_from_command_with_metadata`] to exclude commands/args from MCP
/// or to make optional args required in the MCP tool schema.
///
/// # Example (imperative)
///
/// ```rust
/// use clap::Command;
/// use clap_mcp::{schema_from_command_with_metadata, ClapMcpSchemaMetadata};
///
/// let mut metadata = ClapMcpSchemaMetadata::default();
/// metadata.skip_commands.push("internal".into());
/// metadata.skip_args.insert("mycmd".into(), vec!["verbose".into()]);
/// metadata.requires_args.insert("mycmd".into(), vec!["path".into()]);
///
/// let cmd = Command::new("myapp").subcommand(Command::new("mycmd").arg(clap::Arg::new("path")));
/// let schema = schema_from_command_with_metadata(&cmd, &metadata);
/// ```
#[derive(Debug, Clone, Default)]
pub struct ClapMcpSchemaMetadata {
    /// Command names to exclude from MCP exposure.
    pub skip_commands: Vec<String>,
    /// Per-command arg ids to exclude (command_name -> arg_ids).
    pub skip_args: std::collections::HashMap<String, Vec<String>>,
    /// Per-command arg ids to treat as required in MCP (command_name -> arg_ids).
    pub requires_args: std::collections::HashMap<String, Vec<String>>,
    /// When `true` and the root command has subcommands, the root is excluded from the
    /// MCP tool list (only subcommands become tools). Use when the meaningful tools are
    /// the leaf subcommands (e.g. explain, compare, sort) and the root is rarely invoked.
    pub skip_root_command_when_subcommands: bool,
    /// Optional JSON schema for tool output. When set (e.g. via `#[clap_mcp_output_type]` or
    /// `#[clap_mcp_output_one_of]` with the `output-schema` feature), this schema is attached
    /// to each tool's `output_schema` field.
    pub output_schema: Option<serde_json::Value>,
}

/// Builds a JSON schema for a single type. Used by the derive macro when `#[clap_mcp_output_type = "T"]` is set.
/// When the `output-schema` feature is enabled and `T: schemars::JsonSchema`, returns the schema; otherwise returns `None`.
#[cfg(feature = "output-schema")]
pub fn output_schema_for_type<T: schemars::JsonSchema>() -> Option<serde_json::Value> {
    serde_json::to_value(schemars::schema_for!(T)).ok()
}

#[cfg(not(feature = "output-schema"))]
pub fn output_schema_for_type<T>() -> Option<serde_json::Value> {
    let _ = std::marker::PhantomData::<T>;
    None
}

/// Builds a JSON schema with `oneOf` for the given types. Used by the derive macro when
/// `#[clap_mcp_output_one_of = "T1, T2, T3"]` is set. Requires the `output-schema` feature
/// and each type must implement `schemars::JsonSchema`.
#[macro_export]
macro_rules! output_schema_one_of {
    ($($T:ty),+ $(,)?) => {{
        #[cfg(feature = "output-schema")]
        {
            let mut one_of = vec![];
            $( one_of.push(serde_json::to_value(&schemars::schema_for!($T)).unwrap()); )+
            Some(serde_json::json!({ "oneOf": one_of }))
        }
        #[cfg(not(feature = "output-schema"))]
        {
            None::<serde_json::Value>
        }
    }};
}

/// Serializable schema extracted from a clap `Command`.
/// Used to build MCP tools and invoke the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClapSchema {
    pub root: ClapCommand,
}

/// A command or subcommand in the schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClapCommand {
    pub name: String,
    pub about: Option<String>,
    pub long_about: Option<String>,
    pub version: Option<String>,
    pub args: Vec<ClapArg>,
    pub subcommands: Vec<ClapCommand>,
}

impl ClapCommand {
    /// Returns this command and all subcommands in depth-first order.
    pub fn all_commands(&self) -> Vec<&ClapCommand> {
        let mut out = Vec::new();
        fn walk<'a>(cmd: &'a ClapCommand, acc: &mut Vec<&'a ClapCommand>) {
            acc.push(cmd);
            for sub in &cmd.subcommands {
                walk(sub, acc);
            }
        }
        walk(self, &mut out);
        out
    }
}

/// Arg IDs that are omitted from MCP tool arguments (built-in / default options).
fn is_builtin_arg(id: &str) -> bool {
    matches!(
        id,
        "help" | "version" | MCP_FLAG_LONG | EXPORT_SKILLS_FLAG_LONG
    )
}

/// Builds MCP tools from a clap schema.
///
/// One tool per command (root + every subcommand). Tool names match command names;
/// descriptions use the same text as `--help`; each tool's input schema lists the
/// command's arguments (excluding help/version/mcp).
///
/// # Example
///
/// ```rust
/// use clap::{CommandFactory, Parser};
/// use clap_mcp::{schema_from_command, tools_from_schema};
///
/// #[derive(Parser)]
/// #[command(name = "mycli")]
/// enum Cli { Foo }
///
/// let cmd = Cli::command();
/// let schema = schema_from_command(&cmd);
/// let tools = tools_from_schema(&schema);
/// assert!(!tools.is_empty());
/// ```
pub fn tools_from_schema(schema: &ClapSchema) -> Vec<Tool> {
    tools_from_schema_with_config(schema, &ClapMcpConfig::default())
}

/// Builds MCP tools from a clap schema with execution safety annotations.
///
/// Tools include `meta.clapMcp` with `reinvocationSafe` and `parallelSafe` hints
/// for MCP clients to make informed execution decisions.
///
/// # Example
///
/// ```rust
/// use clap::{CommandFactory, Parser};
/// use clap_mcp::{schema_from_command, tools_from_schema_with_config, ClapMcpConfig};
///
/// #[derive(Parser)]
/// #[command(name = "mycli")]
/// enum Cli { Foo }
///
/// let schema = schema_from_command(&Cli::command());
/// let config = ClapMcpConfig { reinvocation_safe: true, parallel_safe: false, ..Default::default() };
/// let tools = tools_from_schema_with_config(&schema, &config);
/// ```
pub fn tools_from_schema_with_config(schema: &ClapSchema, config: &ClapMcpConfig) -> Vec<Tool> {
    tools_from_schema_with_config_and_metadata(schema, config, &ClapMcpSchemaMetadata::default())
}

/// Builds MCP tools from a clap schema with config and optional metadata.
/// When `metadata.output_schema` is set, each tool's `output_schema` field is set to that value.
/// When `metadata.skip_root_command_when_subcommands` is true and the root has subcommands,
/// the root command is excluded from the tool list (only subcommands become tools).
pub fn tools_from_schema_with_config_and_metadata(
    schema: &ClapSchema,
    config: &ClapMcpConfig,
    metadata: &ClapMcpSchemaMetadata,
) -> Vec<Tool> {
    let commands: Vec<&ClapCommand> =
        if metadata.skip_root_command_when_subcommands && !schema.root.subcommands.is_empty() {
            schema
                .root
                .subcommands
                .iter()
                .flat_map(|c| c.all_commands())
                .collect()
        } else {
            schema.root.all_commands()
        };
    commands
        .into_iter()
        .map(|cmd| command_to_tool_with_config(cmd, config, metadata.output_schema.as_ref()))
        .collect()
}

fn command_to_tool_with_config(
    cmd: &ClapCommand,
    config: &ClapMcpConfig,
    output_schema: Option<&serde_json::Value>,
) -> Tool {
    let args: Vec<&ClapArg> = cmd
        .args
        .iter()
        .filter(|a| !is_builtin_arg(a.id.as_str()))
        .collect();

    let mut properties: HashMap<String, serde_json::Map<String, serde_json::Value>> =
        HashMap::new();
    for arg in &args {
        let mut prop = serde_json::Map::new();
        let (json_type, items) = mcp_type_for_arg(arg);
        prop.insert("type".to_string(), json_type);
        if let Some(items) = items {
            prop.insert("items".to_string(), items);
        }
        let desc = arg
            .long_help
            .as_deref()
            .or(arg.help.as_deref())
            .map(String::from);
        let mut desc = desc.unwrap_or_default();
        if let Some(hint) = mcp_action_description_hint(arg) {
            desc.push_str(&hint);
        }
        if !desc.is_empty() {
            prop.insert("description".to_string(), serde_json::Value::String(desc));
        }
        properties.insert(arg.id.clone(), prop);
    }

    let required: Vec<String> = args
        .iter()
        .filter(|a| a.required)
        .map(|a| a.id.clone())
        .collect();

    let input_schema = ToolInputSchema::new(required, Some(properties), None);

    let description = cmd
        .long_about
        .as_deref()
        .or(cmd.about.as_deref())
        .map(String::from);
    let title = cmd.about.as_ref().map(String::from);

    let meta = {
        let mut m = serde_json::Map::new();
        m.insert(
            "clapMcp".into(),
            serde_json::json!({
                "reinvocationSafe": config.reinvocation_safe,
                "parallelSafe": config.parallel_safe,
                "shareRuntime": config.share_runtime,
            }),
        );
        Some(m)
    };

    Tool {
        name: cmd.name.clone(),
        title,
        description,
        input_schema,
        annotations: None,
        execution: None,
        icons: vec![],
        meta,
        output_schema: output_schema
            .cloned()
            .and_then(|v| serde_json::from_value::<rust_mcp_sdk::schema::ToolOutputSchema>(v).ok()),
    }
}

/// Serializable representation of a clap argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClapArg {
    pub id: String,
    pub long: Option<String>,
    pub short: Option<char>,
    pub help: Option<String>,
    pub long_help: Option<String>,
    pub required: bool,
    pub global: bool,
    pub index: Option<usize>,
    pub action: Option<String>,
    pub value_names: Vec<String>,
    pub num_args: Option<String>,
}

/// Returns the MCP input schema type for an argument based on its action (and num_args).
/// - SetTrue / SetFalse: boolean
/// - Count: integer
/// - Append (or multi-value num_args): array of strings
/// - Set / default: string
///
/// When the arg has a single value_name (e.g. VERSION), the array items schema gets a description
/// so clients know what each element represents.
fn mcp_type_for_arg(arg: &ClapArg) -> (serde_json::Value, Option<serde_json::Value>) {
    let action = arg.action.as_deref().unwrap_or("Set");
    let is_multi = matches!(action, "Append")
        || arg
            .num_args
            .as_deref()
            .is_some_and(|n| n.contains("..") && !n.contains("=1"));
    let (json_type, items) = if matches!(action, "SetTrue" | "SetFalse") {
        (serde_json::json!("boolean"), None)
    } else if action == "Count" {
        (serde_json::json!("integer"), None)
    } else if is_multi {
        let item_desc = arg
            .value_names
            .first()
            .map(|name| format!("A {} value", name));
        let items_schema = match item_desc {
            Some(desc) => serde_json::json!({ "type": "string", "description": desc }),
            None => serde_json::json!({ "type": "string" }),
        };
        (serde_json::json!("array"), Some(items_schema))
    } else {
        (serde_json::json!("string"), None)
    };
    (json_type, items)
}

/// Optional description suffix so MCP clients know what to pass for flags/count/list.
fn mcp_action_description_hint(arg: &ClapArg) -> Option<String> {
    let action = arg.action.as_deref()?;
    let hint: String = match action {
        "SetTrue" => " Boolean flag: set to true to pass this flag.".into(),
        "SetFalse" => " Boolean flag: set to false to pass this flag (e.g. --no-xxx).".into(),
        "Count" => " Number of times the flag is passed (e.g. -vvv).".into(),
        "Append" => {
            if let Some(name) = arg.value_names.first() {
                format!(
                    " List of {} values; pass a JSON array (e.g. [\"a\", \"b\"]).",
                    name
                )
            } else {
                " List of values; pass a JSON array (e.g. [\"a\", \"b\"]).".into()
            }
        }
        _ => return None,
    };
    Some(hint)
}

/// Adds a root-level `--mcp` flag to a `clap::Command` (imperative clap usage).
///
/// When present, the CLI should start an MCP server instead of normal execution.
/// If an arg with `--mcp` already exists, this is a no-op.
///
/// # Example
///
/// ```rust
/// use clap::Command;
/// use clap_mcp::command_with_mcp_flag;
///
/// let cmd = Command::new("myapp");
/// let cmd = command_with_mcp_flag(cmd);
/// assert!(cmd.get_arguments().any(|a| a.get_long() == Some("mcp")));
/// ```
pub fn command_with_mcp_flag(mut cmd: Command) -> Command {
    let already = cmd
        .get_arguments()
        .any(|a| a.get_long().is_some_and(|l| l == MCP_FLAG_LONG));
    if already {
        return cmd;
    }

    cmd = cmd.arg(
        Arg::new(MCP_FLAG_LONG)
            .long(MCP_FLAG_LONG)
            .help("Run an MCP server over stdio that exposes this CLI's clap schema")
            .action(ArgAction::SetTrue)
            .global(true),
    );

    cmd
}

/// Adds a root-level `--export-skills` flag (optional value for output directory) to a `clap::Command`.
///
/// When present, the CLI should generate [Agent Skills](https://agentskills.io/specification)
/// (SKILL.md) and exit. If an arg with `--export-skills` already exists, this is a no-op.
///
/// # Example
///
/// ```rust
/// use clap::Command;
/// use clap_mcp::command_with_export_skills_flag;
///
/// let cmd = Command::new("myapp");
/// let cmd = command_with_export_skills_flag(cmd);
/// ```
pub fn command_with_export_skills_flag(mut cmd: Command) -> Command {
    let already = cmd
        .get_arguments()
        .any(|a| a.get_long().is_some_and(|l| l == EXPORT_SKILLS_FLAG_LONG));
    if already {
        return cmd;
    }

    cmd = cmd.arg(
        Arg::new(EXPORT_SKILLS_FLAG_LONG)
            .long(EXPORT_SKILLS_FLAG_LONG)
            .value_name("DIR")
            .help("Generate Agent Skills (SKILL.md) from tools, resources, and prompts, then exit")
            .action(ArgAction::Set)
            .required(false)
            .global(true),
    );

    cmd
}

/// Adds both `--mcp` and `--export-skills` flags to the command.
/// Use this so schema extraction omits both; check for export-skills before mcp in the parse flow.
pub fn command_with_mcp_and_export_skills_flags(cmd: Command) -> Command {
    command_with_export_skills_flag(command_with_mcp_flag(cmd))
}

/// Returns true if argv contains `--mcp` and no token is a root-level subcommand name.
/// Used to start MCP server before calling get_matches() when subcommand_required would otherwise fail.
fn argv_requests_mcp_without_subcommand(cmd: &Command) -> bool {
    let argv: Vec<String> = std::env::args().collect();
    let args = &argv[1..];
    let subcommand_names: std::collections::HashSet<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    let has_mcp = args.iter().any(|a| a == "--mcp");
    let has_subcommand = args.iter().any(|a| subcommand_names.contains(a.as_str()));
    has_mcp && !has_subcommand
}

/// Returns `Some(None)` if argv contains `--export-skills` with no value (use default dir),
/// `Some(Some(path))` if `--export-skills=DIR` is present, and `None` if the flag is not present.
fn argv_export_skills_dir() -> Option<Option<std::path::PathBuf>> {
    let argv: Vec<String> = std::env::args().collect();
    let args = &argv[1..];
    for (i, arg) in args.iter().enumerate() {
        if arg == "--export-skills" {
            return Some(
                args.get(i + 1)
                    .filter(|s| !s.starts_with('-'))
                    .map(std::path::PathBuf::from),
            );
        }
        if let Some(dir) = arg.strip_prefix("--export-skills=") {
            return Some(Some(std::path::PathBuf::from(dir)));
        }
    }
    None
}

/// Extracts a serializable schema from a `clap::Command` (imperative clap usage).
///
/// The schema reflects the CLI as defined by the application. Any `--mcp` flag
/// added via [`command_with_mcp_flag`] is intentionally omitted.
///
/// # Example
///
/// ```rust
/// use clap::{CommandFactory, Parser};
/// use clap_mcp::schema_from_command;
///
/// #[derive(Parser)]
/// #[command(name = "mycli")]
/// enum Cli { Foo }
///
/// let schema = schema_from_command(&Cli::command());
/// assert_eq!(schema.root.name, "mycli");
/// ```
pub fn schema_from_command(cmd: &Command) -> ClapSchema {
    schema_from_command_with_metadata(cmd, &ClapMcpSchemaMetadata::default())
}

/// Extracts a schema from a `clap::Command` with MCP metadata applied.
///
/// Use [`ClapMcpSchemaMetadata`] to skip commands/args or make optional args required in MCP.
pub fn schema_from_command_with_metadata(
    cmd: &Command,
    metadata: &ClapMcpSchemaMetadata,
) -> ClapSchema {
    let skip_commands: std::collections::HashSet<_> =
        metadata.skip_commands.iter().cloned().collect();
    ClapSchema {
        root: command_to_schema_with_metadata(cmd, metadata, &skip_commands),
    }
}

fn command_to_schema_with_metadata(
    cmd: &Command,
    metadata: &ClapMcpSchemaMetadata,
    skip_commands: &std::collections::HashSet<String>,
) -> ClapCommand {
    let mut args: Vec<ClapArg> = cmd
        .get_arguments()
        .filter(|a| {
            let long = a.get_long();
            long != Some(MCP_FLAG_LONG) && long != Some(EXPORT_SKILLS_FLAG_LONG)
        })
        .map(arg_to_schema)
        .collect();

    let cmd_name = cmd.get_name().to_string();
    let skip_args: std::collections::HashSet<_> = metadata
        .skip_args
        .get(&cmd_name)
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default();

    let requires_args: std::collections::HashSet<_> = metadata
        .requires_args
        .get(&cmd_name)
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default();

    args.retain(|a| !skip_args.contains(&a.id));
    for arg in &mut args {
        if requires_args.contains(&arg.id) {
            arg.required = true;
        }
    }
    args.sort_by(|a, b| a.id.cmp(&b.id));

    let subcommands: Vec<ClapCommand> = cmd
        .get_subcommands()
        .filter(|s| !skip_commands.contains(&s.get_name().to_string()))
        .map(|s| command_to_schema_with_metadata(s, metadata, skip_commands))
        .collect();

    ClapCommand {
        name: cmd.get_name().to_string(),
        about: cmd.get_about().map(|s| s.to_string()),
        long_about: cmd.get_long_about().map(|s| s.to_string()),
        version: cmd.get_version().map(|s| s.to_string()),
        args,
        subcommands,
    }
}

/// Imperative clap entrypoint.
///
/// - Adds `--mcp` to the command (if not already present)
/// - If `--mcp` is present, starts an MCP stdio server and exits the process
/// - Otherwise, returns `ArgMatches` for normal app execution
///
/// # Example
///
/// ```rust,ignore
/// use clap::Command;
/// use clap_mcp::{command_with_mcp_flag, get_matches_or_serve_mcp};
///
/// let cmd = command_with_mcp_flag(Command::new("myapp"));
/// let matches = get_matches_or_serve_mcp(cmd);
/// // If we get here, --mcp was not passed
/// ```
pub fn get_matches_or_serve_mcp(cmd: Command) -> clap::ArgMatches {
    get_matches_or_serve_mcp_with_config(cmd, ClapMcpConfig::default())
}

/// Imperative clap entrypoint with execution safety configuration.
///
/// See [`get_matches_or_serve_mcp`] for behavior. Use `config` to declare
/// reinvocation and parallel execution safety for tool execution.
pub fn get_matches_or_serve_mcp_with_config(
    cmd: Command,
    config: ClapMcpConfig,
) -> clap::ArgMatches {
    get_matches_or_serve_mcp_with_config_and_metadata(
        cmd,
        config,
        &ClapMcpSchemaMetadata::default(),
    )
}

/// Imperative clap entrypoint with execution safety configuration and schema metadata.
///
/// Use `metadata` for `#[clap_mcp(skip)]` and `#[clap_mcp(requires = "arg_name")]` behavior.
pub fn get_matches_or_serve_mcp_with_config_and_metadata(
    cmd: Command,
    config: ClapMcpConfig,
    metadata: &ClapMcpSchemaMetadata,
) -> clap::ArgMatches {
    let schema = schema_from_command_with_metadata(&cmd, metadata);
    let cmd = command_with_mcp_and_export_skills_flags(cmd);

    if let Some(maybe_dir) = argv_export_skills_dir() {
        let tools = tools_from_schema_with_config_and_metadata(&schema, &config, metadata);
        let output_dir = maybe_dir.unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
        let app_name = schema.root.name.as_str();
        let serve_options = ClapMcpServeOptions::default();
        if let Err(e) = content::export_skills(
            &schema,
            metadata,
            &tools,
            &serve_options.custom_resources,
            &serve_options.custom_prompts,
            &output_dir,
            app_name,
        ) {
            eprintln!("export-skills failed: {}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    if config.allow_mcp_without_subcommand && argv_requests_mcp_without_subcommand(&cmd) {
        let schema_json = match serde_json::to_string_pretty(&schema) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to serialize CLI schema: {}", e);
                std::process::exit(1);
            }
        };
        if let Err(e) = serve_schema_json_over_stdio_blocking(
            schema_json,
            None,
            config,
            None,
            ClapMcpServeOptions::default(),
            metadata,
        ) {
            eprintln!("MCP server error: {}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    let matches = cmd.get_matches();
    if matches.get_flag(MCP_FLAG_LONG) {
        let schema_json = match serde_json::to_string_pretty(&schema) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to serialize CLI schema: {}", e);
                std::process::exit(1);
            }
        };
        if let Err(e) = serve_schema_json_over_stdio_blocking(
            schema_json,
            None,
            config,
            None,
            ClapMcpServeOptions::default(),
            metadata,
        ) {
            eprintln!("MCP server error: {}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    matches
}

/// Canonical entrypoint for derive-based CLIs: parse (or serve if `--mcp`) and return self.
///
/// With the trait in scope, use `Args::parse_or_serve_mcp()` instead of
/// `parse_or_serve_mcp_attr::<Args>()`. Equivalent to calling [`parse_or_serve_mcp_attr`];
/// that free function remains available if you prefer it.
///
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
/// use clap_mcp::{ClapMcp, ParseOrServeMcp};
///
/// #[derive(Parser, ClapMcp)]
/// #[clap_mcp(reinvocation_safe, parallel_safe = false)]
/// enum Cli { Foo }
///
/// fn main() {
///     let cli = Cli::parse_or_serve_mcp();
///     // ...
/// }
/// ```
pub trait ParseOrServeMcp {
    fn parse_or_serve_mcp() -> Self;
}

impl<T> ParseOrServeMcp for T
where
    T: ClapMcpConfigProvider
        + ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutor
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    fn parse_or_serve_mcp() -> Self {
        parse_or_serve_mcp_attr::<T>()
    }
}

/// High-level helper for `clap` derive-based CLIs.
///
/// - Adds `--mcp` to the command
/// - If `--mcp` is present, starts an MCP stdio server and exits the process
/// - Otherwise, returns the parsed CLI type
///
/// Uses default [`ClapMcpConfig`]. For config from `#[clap_mcp(...)]` attributes,
/// use [`parse_or_serve_mcp_attr`].
///
/// For a **struct root with subcommand**, parse the root type then call your run
/// logic on the subcommand (e.g. `run(args.command)`). See the crate README
/// section "Struct root with subcommand".
///
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
/// use clap_mcp::ClapMcp;
///
/// #[derive(Parser, ClapMcp)]
/// enum Cli { Foo }
///
/// fn main() {
///     let cli = clap_mcp::parse_or_serve_mcp::<Cli>();
///     // If we get here, --mcp was not passed
/// }
/// ```
pub fn parse_or_serve_mcp<T>() -> T
where
    T: ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutor
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    parse_or_serve_mcp_with_config::<T>(ClapMcpConfig::default())
}

/// High-level helper for `clap` derive-based CLIs with config from `#[clap_mcp(...)]` attributes.
///
/// Use `#[derive(ClapMcp)]` and `#[clap_mcp(reinvocation_safe, parallel_safe = false)]` on your CLI type,
/// then call this instead of [`parse_or_serve_mcp`]. Config is taken from `T::clap_mcp_config()`.
///
/// For a **struct root with subcommand**, parse the root type then call your run
/// logic on the subcommand (e.g. `run(args.command)` or `match args.command { ... }`).
/// See the crate README section "Struct root with subcommand" and [`ParseOrServeMcp`].
///
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
/// use clap_mcp::ClapMcp;
///
/// #[derive(Parser, ClapMcp)]
/// #[clap_mcp(reinvocation_safe, parallel_safe = false)]
/// enum Cli {
///     #[clap_mcp_output_literal = "done"]
///     Foo,
/// }
///
/// fn main() {
///     let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
///     match cli { Cli::Foo => println!("done") }
/// }
/// ```
pub fn parse_or_serve_mcp_attr<T>() -> T
where
    T: ClapMcpConfigProvider
        + ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutor
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    parse_or_serve_mcp_with_config::<T>(T::clap_mcp_config())
}

/// Run parsed CLI through a closure, or serve MCP if `--mcp` is present.
///
/// If `--mcp` is passed, starts the MCP server and does not return. Otherwise,
/// parses the CLI type `A`, calls `f(args)`, and returns the result. Use this
/// when you want the "parse then run" flow in one place (e.g. `run_or_serve_mcp::<Cli, _>(|c| Ok(run(c)))`)
/// instead of parsing and then calling `run` in main. For a simple "parse then branch"
/// style, use [`ParseOrServeMcp::parse_or_serve_mcp`] or [`parse_or_serve_mcp_attr`].
///
/// # Example
///
/// ```rust,ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     clap_mcp::run_or_serve_mcp::<Cli, _, _, _>(|cli| Ok(run(cli)))
/// }
/// ```
pub fn run_or_serve_mcp<A, F, R, E>(f: F) -> Result<R, E>
where
    A: ClapMcpConfigProvider
        + ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutor
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
    F: FnOnce(A) -> Result<R, E>,
{
    let args = parse_or_serve_mcp_attr::<A>();
    f(args)
}

/// High-level helper for `clap` derive-based CLIs with execution safety configuration.
///
/// See [`parse_or_serve_mcp`] for behavior. Use `config` to declare reinvocation
/// and parallel execution safety. When `reinvocation_safe` is true, uses in-process
/// execution; requires `T: ClapMcpToolExecutor`.
pub fn parse_or_serve_mcp_with_config<T>(config: ClapMcpConfig) -> T
where
    T: ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutor
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    parse_or_serve_mcp_with_config_and_options::<T>(config, ClapMcpServeOptions::default())
}

/// Like [`parse_or_serve_mcp_with_config`] but with custom serve options (e.g. logging).
///
/// Use `serve_options.log_rx` to forward log messages to the MCP client.
/// See [`ClapMcpServeOptions`] and the `logging` module.
pub fn parse_or_serve_mcp_with_config_and_options<T>(
    config: ClapMcpConfig,
    serve_options: ClapMcpServeOptions,
) -> T
where
    T: ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutor
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    let mut cmd = T::command();
    cmd = command_with_mcp_and_export_skills_flags(cmd);

    if let Some(maybe_dir) = argv_export_skills_dir() {
        let base_cmd = T::command();
        let metadata = T::clap_mcp_schema_metadata();
        let schema = schema_from_command_with_metadata(&base_cmd, &metadata);
        let tools = tools_from_schema_with_config_and_metadata(&schema, &config, &metadata);
        let output_dir = maybe_dir.unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
        let app_name = schema.root.name.as_str();
        if let Err(e) = content::export_skills(
            &schema,
            &metadata,
            &tools,
            &serve_options.custom_resources,
            &serve_options.custom_prompts,
            &output_dir,
            app_name,
        ) {
            eprintln!("export-skills failed: {}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    if config.allow_mcp_without_subcommand && argv_requests_mcp_without_subcommand(&cmd) {
        let base_cmd = T::command();
        let metadata = T::clap_mcp_schema_metadata();
        let schema = schema_from_command_with_metadata(&base_cmd, &metadata);
        let schema_json = match serde_json::to_string_pretty(&schema) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to serialize CLI schema: {}", e);
                std::process::exit(1);
            }
        };
        let exe = std::env::current_exe().ok();

        let in_process_handler = if config.reinvocation_safe {
            #[cfg(unix)]
            let capture_stdout = serve_options.capture_stdout;
            #[cfg(not(unix))]
            let capture_stdout = false;
            Some(make_in_process_handler::<T>(schema.clone(), capture_stdout))
        } else {
            None
        };

        if let Err(e) = serve_schema_json_over_stdio_blocking(
            schema_json,
            if config.reinvocation_safe { None } else { exe },
            config,
            in_process_handler,
            serve_options,
            &metadata,
        ) {
            eprintln!("MCP server error: {}", e);
            std::process::exit(1);
        }

        std::process::exit(0);
    }

    let matches = cmd.get_matches();
    let mcp_requested = matches.get_flag(MCP_FLAG_LONG);

    if mcp_requested {
        let base_cmd = T::command();
        let metadata = T::clap_mcp_schema_metadata();
        let schema = schema_from_command_with_metadata(&base_cmd, &metadata);
        let schema_json = match serde_json::to_string_pretty(&schema) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to serialize CLI schema: {}", e);
                std::process::exit(1);
            }
        };
        let exe = std::env::current_exe().ok();

        let in_process_handler = if config.reinvocation_safe {
            #[cfg(unix)]
            let capture_stdout = serve_options.capture_stdout;
            #[cfg(not(unix))]
            let capture_stdout = false;
            Some(make_in_process_handler::<T>(schema.clone(), capture_stdout))
        } else {
            None
        };

        if let Err(e) = serve_schema_json_over_stdio_blocking(
            schema_json,
            if config.reinvocation_safe { None } else { exe },
            config,
            in_process_handler,
            serve_options,
            &metadata,
        ) {
            eprintln!("MCP server error: {}", e);
            std::process::exit(1);
        }

        std::process::exit(0);
    }

    T::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}

fn arg_to_schema(arg: &clap::Arg) -> ClapArg {
    let value_names = arg
        .get_value_names()
        .map(|names| names.iter().map(|n| n.to_string()).collect())
        .unwrap_or_default();

    ClapArg {
        id: arg.get_id().to_string(),
        long: arg.get_long().map(|s| s.to_string()),
        short: arg.get_short(),
        help: arg.get_help().map(|s| s.to_string()),
        long_help: arg.get_long_help().map(|s| s.to_string()),
        required: arg.is_required_set(),
        global: arg.is_global_set(),
        index: arg.get_index(),
        action: Some(format!("{:?}", arg.get_action())),
        value_names,
        num_args: arg.get_num_args().map(|r| format!("{r:?}")),
    }
}

/// Validates that all required args for the command are present in the arguments map.
/// Returns Err with a clear message if any required arg is missing.
fn validate_required_args(
    schema: &ClapSchema,
    command_name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let cmd = schema
        .root
        .all_commands()
        .into_iter()
        .find(|c| c.name == command_name);
    let Some(cmd) = cmd else {
        return Ok(());
    };
    let missing: Vec<_> = cmd
        .args
        .iter()
        .filter(|a| {
            if !a.required || is_builtin_arg(a.id.as_str()) {
                return false;
            }
            let has_value = arguments.get(&a.id).map(|v| {
                let action = a.action.as_deref().unwrap_or("Set");
                if matches!(action, "SetTrue" | "SetFalse" | "Count") {
                    // Flag/count: key present is enough (value can be false/0)
                    true
                } else if action == "Append" || v.is_array() {
                    !value_to_strings(v).is_some_and(|s| s.is_empty())
                } else {
                    value_to_string(v).is_some_and(|s| !s.is_empty())
                }
            });
            !has_value.unwrap_or(false)
        })
        .map(|a| a.id.clone())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Missing required argument(s): {}. The MCP tool schema marks these as required.",
            missing.join(", ")
        ))
    }
}

/// Builds full argv for clap's `get_matches_from` (program name + subcommand + args).
fn build_argv_for_clap(
    schema: &ClapSchema,
    command_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let args = build_tool_argv(schema, command_name, arguments);
    let mut argv = vec!["cli".to_string()]; // program name for parsing
    if let Some(path) = command_path(schema, command_name) {
        argv.extend(path.into_iter().skip(1));
    }
    argv.extend(args);
    argv
}

fn command_path(schema: &ClapSchema, command_name: &str) -> Option<Vec<String>> {
    fn walk(cmd: &ClapCommand, command_name: &str, path: &mut Vec<String>) -> bool {
        path.push(cmd.name.clone());
        if cmd.name == command_name {
            return true;
        }
        for subcommand in &cmd.subcommands {
            if walk(subcommand, command_name, path) {
                return true;
            }
        }
        path.pop();
        false
    }

    let mut path = Vec::new();
    if walk(&schema.root, command_name, &mut path) {
        Some(path)
    } else {
        None
    }
}

/// Builds argv for the executable from the schema and tool arguments.
///
/// Positional args (no long form) are passed in index order; optional args as `--long value`.
fn build_tool_argv(
    schema: &ClapSchema,
    command_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let cmd = schema
        .root
        .all_commands()
        .into_iter()
        .find(|c| c.name == command_name);
    let Some(cmd) = cmd else {
        return Vec::new();
    };

    let args: Vec<&ClapArg> = cmd
        .args
        .iter()
        .filter(|a| !is_builtin_arg(a.id.as_str()))
        .collect();

    let mut positionals: Vec<&ClapArg> =
        args.iter().filter(|a| a.long.is_none()).copied().collect();
    positionals.sort_by_key(|a| a.index.unwrap_or(0));
    let optionals: Vec<&ClapArg> = args.iter().filter(|a| a.long.is_some()).copied().collect();

    let mut out = Vec::new();

    for arg in positionals {
        if let Some(v) = arguments.get(&arg.id)
            && let Some(strings) = value_to_strings(v)
        {
            for s in strings {
                out.push(s);
            }
        }
    }
    for arg in optionals {
        if let Some(long) = &arg.long {
            let action = arg.action.as_deref().unwrap_or("Set");
            let v = arguments.get(&arg.id);
            match action {
                "SetTrue" => {
                    if v.and_then(value_to_string).is_some_and(|s| s == "true")
                        || v.and_then(|x| x.as_bool()).is_some_and(|b| b)
                    {
                        out.push(format!("--{long}"));
                    }
                }
                "SetFalse" => {
                    if v.and_then(value_to_string).is_some_and(|s| s == "false")
                        || v.and_then(|x| x.as_bool()).is_some_and(|b| !b)
                    {
                        out.push(format!("--{long}"));
                    }
                }
                "Count" => {
                    let n = v.and_then(|x| x.as_i64()).unwrap_or(0).clamp(0, i64::MAX) as usize;
                    for _ in 0..n {
                        out.push(format!("--{long}"));
                    }
                }
                "Append" => {
                    if let Some(v) = v.and_then(value_to_strings) {
                        for s in v {
                            if !s.is_empty() {
                                out.push(format!("--{long}"));
                                out.push(s);
                            }
                        }
                    } else if let Some(s) = v.and_then(value_to_string)
                        && !s.is_empty()
                    {
                        out.push(format!("--{long}"));
                        out.push(s);
                    }
                }
                _ => {
                    if let Some(s) = v.and_then(value_to_string)
                        && !s.is_empty()
                    {
                        out.push(format!("--{long}"));
                        out.push(s);
                    }
                }
            }
        }
    }

    out
}

/// Type for in-process tool execution handler.
///
/// Called with `(command_name, arguments)` and returns `Result<ClapMcpToolOutput, ClapMcpToolError>`.
/// Used when `reinvocation_safe` is true to avoid spawning subprocesses.
pub type InProcessToolHandler = Arc<
    dyn Fn(
            &str,
            serde_json::Map<String, serde_json::Value>,
        ) -> Result<ClapMcpToolOutput, ClapMcpToolError>
        + Send
        + Sync,
>;

fn merge_captured_stdout(
    result: Result<ClapMcpToolOutput, ClapMcpToolError>,
    captured: String,
) -> Result<ClapMcpToolOutput, ClapMcpToolError> {
    match result {
        Ok(ClapMcpToolOutput::Text(text)) if !captured.is_empty() => {
            let merged = if text.is_empty() {
                captured.trim().to_string()
            } else {
                let cap = captured.trim();
                if cap.is_empty() {
                    text
                } else {
                    format!("{text}\n{cap}")
                }
            };
            Ok(ClapMcpToolOutput::Text(merged))
        }
        other => other,
    }
}

fn execute_in_process_command<T>(
    schema: &ClapSchema,
    command_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
    capture_stdout: bool,
) -> Result<ClapMcpToolOutput, ClapMcpToolError>
where
    T: ClapMcpToolExecutor + clap::CommandFactory + clap::FromArgMatches,
{
    validate_required_args(schema, command_name, &arguments).map_err(ClapMcpToolError::text)?;
    let argv = build_argv_for_clap(schema, command_name, arguments.clone());
    let matches = T::command()
        .try_get_matches_from(&argv)
        .map_err(|e| ClapMcpToolError::text(e.to_string()))?;
    let cli = T::from_arg_matches(&matches).map_err(|e| ClapMcpToolError::text(e.to_string()))?;

    if capture_stdout {
        let (result, captured) =
            run_with_stdout_capture(|| <T as ClapMcpToolExecutor>::execute_for_mcp(cli));
        merge_captured_stdout(result, captured)
    } else {
        <T as ClapMcpToolExecutor>::execute_for_mcp(cli)
    }
}

fn make_in_process_handler<T>(schema: ClapSchema, capture_stdout: bool) -> InProcessToolHandler
where
    T: ClapMcpToolExecutor + clap::CommandFactory + clap::FromArgMatches + 'static,
{
    Arc::new(
        move |cmd: &str, args: serde_json::Map<String, serde_json::Value>| {
            execute_in_process_command::<T>(&schema, cmd, args, capture_stdout)
        },
    ) as InProcessToolHandler
}

fn format_panic_payload(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "<panic>".to_string()
}

fn value_to_string(v: &serde_json::Value) -> Option<String> {
    if v.is_null() {
        return None;
    }
    Some(match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    })
}

/// Returns one or more string values for MCP input. For arrays, returns each element as string; otherwise single value.
fn value_to_strings(v: &serde_json::Value) -> Option<Vec<String>> {
    if v.is_null() {
        return None;
    }
    match v {
        serde_json::Value::Array(arr) => {
            let out: Vec<String> = arr
                .iter()
                .filter_map(value_to_string)
                .filter(|s| !s.is_empty())
                .collect();
            Some(out)
        }
        _ => value_to_string(v).map(|s| vec![s]),
    }
}

fn clap_schema_resource() -> Resource {
    Resource {
        name: "clap-schema".into(),
        uri: MCP_RESOURCE_URI_SCHEMA.into(),
        title: Some("Clap CLI schema".into()),
        description: Some("JSON schema extracted from clap Command definitions".into()),
        mime_type: Some("application/json".into()),
        annotations: None,
        icons: vec![],
        meta: None,
        size: None,
    }
}

fn list_resources_result(custom_resources: &[content::CustomResource]) -> ListResourcesResult {
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

async fn read_resource_result(
    schema_json: &str,
    custom_resources: &[content::CustomResource],
    params: ReadResourceRequestParams,
) -> std::result::Result<ReadResourceResult, RpcError> {
    if params.uri == MCP_RESOURCE_URI_SCHEMA {
        return Ok(ReadResourceResult {
            contents: vec![ReadResourceContent::TextResourceContents(
                TextResourceContents {
                    uri: params.uri,
                    mime_type: Some("application/json".into()),
                    text: schema_json.to_string(),
                    meta: None,
                },
            )],
            meta: None,
        });
    }
    let custom = custom_resources
        .iter()
        .find(|resource| resource.uri == params.uri);
    let Some(resource) = custom else {
        return Err(RpcError::invalid_params()
            .with_message(format!("unknown resource uri: {}", params.uri)));
    };
    let text = content::resolve_resource_content(resource, &params.uri).await?;
    Ok(ReadResourceResult {
        contents: vec![ReadResourceContent::TextResourceContents(
            TextResourceContents {
                uri: params.uri.clone(),
                mime_type: resource.mime_type.clone(),
                text,
                meta: None,
            },
        )],
        meta: None,
    })
}

fn logging_guide_prompt() -> Prompt {
    Prompt {
        name: PROMPT_LOGGING_GUIDE.to_string(),
        description: Some("How to interpret log messages from this clap-mcp server".to_string()),
        arguments: vec![],
        icons: vec![],
        meta: None,
        title: Some("clap-mcp Logging Guide".to_string()),
    }
}

fn list_prompts_result(
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

async fn get_prompt_result(
    logging_enabled: bool,
    custom_prompts: &[content::CustomPrompt],
    params: GetPromptRequestParams,
) -> std::result::Result<GetPromptResult, RpcError> {
    if params.name == PROMPT_LOGGING_GUIDE {
        if !logging_enabled {
            return Err(
                RpcError::invalid_params().with_message(format!("unknown prompt: {}", params.name))
            );
        }
        return Ok(GetPromptResult {
            description: Some(
                "How to interpret log messages from this clap-mcp server".to_string(),
            ),
            messages: vec![PromptMessage {
                content: ContentBlock::text_content(LOGGING_GUIDE_CONTENT.to_string()),
                role: Role::User,
            }],
            meta: None,
        });
    }
    let custom = custom_prompts
        .iter()
        .find(|prompt| prompt.name == params.name);
    let Some(prompt) = custom else {
        return Err(
            RpcError::invalid_params().with_message(format!("unknown prompt: {}", params.name))
        );
    };
    let arguments: serde_json::Map<String, serde_json::Value> = params
        .arguments
        .as_ref()
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
                .collect()
        })
        .unwrap_or_default();
    let messages = content::resolve_prompt_content(prompt, &params.name, &arguments).await?;
    Ok(GetPromptResult {
        description: prompt.description.clone(),
        messages,
        meta: None,
    })
}

fn validate_tool_argument_names(
    tool: &Tool,
    tool_name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<(), CallToolError> {
    if let Some(ref props) = tool.input_schema.properties {
        for key in arguments.keys() {
            if !props.contains_key(key) {
                return Err(CallToolError::invalid_arguments(
                    tool_name,
                    Some(format!("unknown argument: {key}")),
                ));
            }
        }
    }
    Ok(())
}

fn call_tool_result_from_output(output: ClapMcpToolOutput) -> CallToolResult {
    let (content, structured_content) = match output {
        ClapMcpToolOutput::Text(text) => (vec![ContentBlock::text_content(text)], None),
        ClapMcpToolOutput::Structured(value) => {
            let json_text =
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            let structured = value.as_object().cloned();
            (vec![ContentBlock::text_content(json_text)], structured)
        }
    };
    CallToolResult {
        content,
        is_error: None,
        meta: None,
        structured_content,
    }
}

fn call_tool_result_from_tool_error(error: ClapMcpToolError) -> CallToolResult {
    let structured_content = error
        .structured
        .as_ref()
        .and_then(|value| value.as_object().cloned());
    CallToolResult {
        content: vec![ContentBlock::text_content(error.message)],
        is_error: Some(true),
        meta: None,
        structured_content,
    }
}

fn call_tool_result_from_panic(panic_payload: &(dyn std::any::Any + Send)) -> CallToolResult {
    let msg = format_panic_payload(panic_payload);
    CallToolResult {
        content: vec![ContentBlock::text_content(format!(
            "Tool panicked: {}",
            msg
        ))],
        is_error: Some(true),
        meta: None,
        structured_content: None,
    }
}

fn schema_parse_failure_result() -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::text_content("Failed to parse schema".into())],
        is_error: Some(true),
        meta: None,
        structured_content: None,
    }
}

fn command_launch_failure_result(error: &std::io::Error) -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::text_content(format!(
            "Failed to run command: {}",
            error
        ))],
        is_error: Some(true),
        meta: None,
        structured_content: None,
    }
}

fn placeholder_tool_result(
    name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> CallToolResult {
    let args_json = serde_json::Value::Object(arguments.clone());
    CallToolResult::from_content(vec![ContentBlock::text_content(format!(
        "Would invoke clap command '{name}' with arguments: {args_json:?}"
    ))])
}

fn build_execution_command(
    executable_path: &std::path::Path,
    schema: &ClapSchema,
    root_name: &str,
    tool_name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> std::process::Command {
    let argv = build_tool_argv(schema, tool_name, arguments.clone());
    let mut command = std::process::Command::new(executable_path);
    if let Some(path) = command_path(schema, tool_name) {
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

fn subprocess_stderr_log_params(
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

fn call_tool_result_from_subprocess_output(output: &std::process::Output) -> CallToolResult {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let code = output
            .status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let mut msg = format!("Tool process exited with non-zero status (code: {})", code);
        if !stderr.is_empty() {
            msg.push_str("\nstderr:\n");
            msg.push_str(stderr.trim());
        }
        return CallToolResult {
            content: vec![ContentBlock::text_content(msg)],
            is_error: Some(true),
            meta: None,
            structured_content: None,
        };
    }
    let text = if stderr.is_empty() {
        stdout.trim().to_string()
    } else {
        format!("{}\nstderr:\n{}", stdout.trim(), stderr.trim())
    };
    CallToolResult::from_content(vec![ContentBlock::text_content(text)])
}

/// Starts an MCP server over stdio exposing `clap://schema` with the provided JSON payload.
///
/// - When `in_process_handler` is `Some`, tool calls use it instead of spawning a subprocess.
/// - When `None` and `executable_path` is `Some`, tool calls run that executable.
/// - When both are `None`, returns a placeholder message for unknown tools.
///
/// Use `config` to declare reinvocation and parallel execution safety. When
/// `parallel_safe` is false, tool calls are serialized.
///
/// Use `serve_options.log_rx` to forward log messages to the MCP client.
///
/// Use `metadata` to attach an optional output schema to each tool (e.g. from
/// `#[clap_mcp_output_type]` or `#[clap_mcp_output_one_of]` with the `output-schema`
/// feature). Pass [`ClapMcpSchemaMetadata::default()`] when you have none.
///
/// # Example
///
/// ```rust,ignore
/// let schema_json = serde_json::to_string(&schema)?;
/// let metadata = clap_mcp::ClapMcpSchemaMetadata::default();
/// clap_mcp::serve_schema_json_over_stdio(
///     schema_json,
///     Some(std::env::current_exe()?),
///     clap_mcp::ClapMcpConfig::default(),
///     None,
///     clap_mcp::ClapMcpServeOptions::default(),
///     &metadata,
/// ).await?;
/// ```
pub async fn serve_schema_json_over_stdio(
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> std::result::Result<(), ClapMcpError> {
    let schema: ClapSchema = serde_json::from_str(&schema_json)?;
    let tools = tools_from_schema_with_config_and_metadata(&schema, &config, metadata);
    let root_name = schema.root.name.clone();

    let tool_execution_lock: Option<Arc<tokio::sync::Mutex<()>>> = if config.parallel_safe {
        None
    } else {
        Some(Arc::new(tokio::sync::Mutex::new(())))
    };

    let logging_enabled = serve_options.log_rx.is_some();
    let (runtime_tx, runtime_rx) = if logging_enabled {
        let (tx, rx) = tokio::sync::oneshot::channel::<Arc<dyn rust_mcp_sdk::McpServer>>();
        (
            Some(std::sync::Arc::new(std::sync::Mutex::new(Some(tx)))),
            Some(rx),
        )
    } else {
        (None, None)
    };

    if let (Some(mut log_rx), Some(runtime_rx)) = (serve_options.log_rx, runtime_rx) {
        tokio::spawn(async move {
            let Ok(runtime) = runtime_rx.await else {
                return;
            };
            while let Some(params) = log_rx.recv().await {
                let _ = runtime.notify_log_message(params).await;
            }
        });
    }

    type RuntimeTx = Option<
        Arc<
            std::sync::Mutex<
                Option<tokio::sync::oneshot::Sender<Arc<dyn rust_mcp_sdk::McpServer>>>,
            >,
        >,
    >;

    struct Handler {
        schema_json: String,
        tools: Vec<Tool>,
        executable_path: Option<PathBuf>,
        in_process_handler: Option<InProcessToolHandler>,
        root_name: String,
        tool_execution_lock: Option<Arc<tokio::sync::Mutex<()>>>,
        runtime_tx: RuntimeTx,
        catch_in_process_panics: bool,
        custom_resources: Vec<content::CustomResource>,
        custom_prompts: Vec<content::CustomPrompt>,
        logging_enabled: bool,
    }

    #[async_trait]
    impl ServerHandler for Handler {
        async fn handle_list_resources_request(
            &self,
            _params: Option<PaginatedRequestParams>,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<ListResourcesResult, RpcError> {
            Ok(list_resources_result(&self.custom_resources))
        }

        async fn handle_read_resource_request(
            &self,
            params: ReadResourceRequestParams,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<ReadResourceResult, RpcError> {
            read_resource_result(&self.schema_json, &self.custom_resources, params).await
        }

        async fn handle_list_tools_request(
            &self,
            _params: Option<PaginatedRequestParams>,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<ListToolsResult, RpcError> {
            Ok(ListToolsResult {
                tools: self.tools.clone(),
                meta: None,
                next_cursor: None,
            })
        }

        async fn handle_list_prompts_request(
            &self,
            _params: Option<PaginatedRequestParams>,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<ListPromptsResult, RpcError> {
            Ok(list_prompts_result(
                self.logging_enabled,
                &self.custom_prompts,
            ))
        }

        async fn handle_get_prompt_request(
            &self,
            params: GetPromptRequestParams,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<GetPromptResult, RpcError> {
            get_prompt_result(self.logging_enabled, &self.custom_prompts, params).await
        }

        async fn handle_call_tool_request(
            &self,
            params: CallToolRequestParams,
            runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<CallToolResult, CallToolError> {
            if let Some(ref tx) = self.runtime_tx
                && let Ok(mut guard) = tx.lock()
                && let Some(sender) = guard.take()
            {
                let _ = sender.send(runtime.clone());
            }

            let tool = self.tools.iter().find(|t| t.name == params.name);
            let Some(tool) = tool else {
                return Err(CallToolError::unknown_tool(params.name.clone()));
            };

            // Reject unknown argument names — do not trust client to send only schema-defined args
            let args_map = params.arguments.unwrap_or_default();
            validate_tool_argument_names(tool, &params.name, &args_map)?;

            let _guard = if let Some(ref lock) = self.tool_execution_lock {
                Some(lock.lock().await)
            } else {
                None
            };

            if let Some(ref handler) = self.in_process_handler {
                let name = params.name.clone();
                let args = args_map;
                let result = if self.catch_in_process_panics {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&name, args)))
                } else {
                    Ok(handler(&name, args))
                };
                match result {
                    Ok(Ok(output)) => return Ok(call_tool_result_from_output(output)),
                    Ok(Err(error)) => return Ok(call_tool_result_from_tool_error(error)),
                    Err(panic_payload) => {
                        return Ok(call_tool_result_from_panic(panic_payload.as_ref()));
                    }
                }
            }

            if let Some(ref exe) = self.executable_path {
                let schema: ClapSchema = match serde_json::from_str(&self.schema_json) {
                    Ok(schema) => schema,
                    Err(_) => return Ok(schema_parse_failure_result()),
                };
                if let Err(e) = validate_required_args(&schema, &params.name, &args_map) {
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
                            // When changing stderr logging behavior, update LOG_INTERPRETATION_INSTRUCTIONS and LOGGING_GUIDE_CONTENT.
                            let _ = runtime.notify_log_message(log_params).await;
                        }
                        return Ok(call_tool_result_from_subprocess_output(&output));
                    }
                    Err(error) => return Ok(command_launch_failure_result(&error)),
                }
            }

            Ok(placeholder_tool_result(&params.name, &args_map))
        }
    }

    let meta = {
        let mut m = serde_json::Map::new();
        m.insert(
            "clapMcp".into(),
            serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "commit": env!("CLAP_MCP_GIT_COMMIT"),
                "buildDate": env!("CLAP_MCP_BUILD_DATE"),
            }),
        );
        Some(m)
    };

    let server_details = InitializeResult {
        server_info: Implementation {
            name: "clap-mcp".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            title: Some("clap-mcp".into()),
            description: Some("Expose clap CLI schema over MCP (stdio)".into()),
            icons: vec![],
            website_url: None,
        },
        capabilities: ServerCapabilities {
            resources: Some(ServerCapabilitiesResources {
                list_changed: Some(false),
                subscribe: Some(false),
            }),
            tools: Some(ServerCapabilitiesTools {
                list_changed: Some(false),
            }),
            logging: if logging_enabled {
                Some(serde_json::Map::new())
            } else {
                None
            },
            prompts: Some(ServerCapabilitiesPrompts {
                list_changed: Some(false),
            }),
            ..Default::default()
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        instructions: if logging_enabled {
            Some(LOG_INTERPRETATION_INSTRUCTIONS.to_string())
        } else {
            None
        },
        meta,
    };

    // Conservative timeout; mostly irrelevant for server-side stdio.
    let transport_options = TransportOptions {
        timeout: Duration::from_secs(30),
    };
    // For server-side stdio transport, use the ClientMessage dispatcher direction expected by ServerRuntime.
    let transport = StdioTransport::<schema_utils::ClientMessage>::new(transport_options)?;

    let handler = Handler {
        schema_json,
        tools,
        executable_path,
        in_process_handler,
        root_name,
        tool_execution_lock,
        runtime_tx,
        catch_in_process_panics: config.catch_in_process_panics,
        custom_resources: serve_options.custom_resources.clone(),
        custom_prompts: serve_options.custom_prompts.clone(),
        logging_enabled,
    }
    .to_mcp_server_handler();
    let server = server_runtime::create_server(McpServerOptions {
        server_details,
        transport,
        handler,
        task_store: None,
        client_task_store: None,
    });

    server.start().await?;
    Ok(())
}

/// Convenience wrapper for [`serve_schema_json_over_stdio`] that blocks on a tokio runtime.
///
/// Use when you cannot use `async fn main`. Spawns a runtime internally.
///
/// # Runtime selection
///
/// | `reinvocation_safe` | `share_runtime` | Runtime type |
/// |---------------------|----------------|--------------|
/// | `false` | any | `current_thread` |
/// | `true` | `false` | `current_thread` |
/// | `true` | `true` | `multi_thread` (so [`run_async_tool`] with `share_runtime` can use `block_on`) |
pub fn serve_schema_json_over_stdio_blocking(
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> std::result::Result<(), ClapMcpError> {
    let use_multi_thread = config.reinvocation_safe && config.share_runtime;
    let rt = if use_multi_thread {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
    };
    rt.block_on(serve_schema_json_over_stdio(
        schema_json,
        executable_path,
        config,
        in_process_handler,
        serve_options,
        metadata,
    ))
}

/// Runs an async future for MCP tool execution, respecting `share_runtime` in config.
///
/// **Idiomatic approach:** with `#[clap_mcp_output_from = "run"]`, do async work inside your
/// `run` function (e.g. use a runtime handle or call this function). The closure must return
/// a `Future` that produces the tool output.
///
/// Returns [`Ok`] with the future's output, or [`Err`](ClapMcpError) if the runtime could
/// not be created, the current context is invalid (`share_runtime` without a tokio runtime),
/// or the async thread panicked.
///
/// # Runtime selection
///
/// | `reinvocation_safe` | `share_runtime` | Behavior |
/// |---------------------|----------------|----------|
/// | `false` | any | Dedicated thread (subprocess mode; `share_runtime` ignored) |
/// | `true` | `false` | Dedicated thread with its own tokio runtime (default, recommended) |
/// | `true` | `true` | Uses `Handle::current().block_on()` on the MCP server's runtime |
///
/// When `share_runtime` is true, uses `block_in_place` + `block_on` so the async
/// work runs on the MCP server's multi-thread runtime without deadlock.
///
/// # Example (async inside `run`)
///
/// ```rust,ignore
/// fn run(cmd: Cli) -> SleepResult {
///     match cmd {
///         Cli::SleepDemo => clap_mcp::run_async_tool(&Cli::clap_mcp_config(), run_sleep_demo).expect("async tool failed"),
///     }
/// }
/// ```
pub fn run_async_tool<Fut, O>(
    config: &ClapMcpConfig,
    f: impl FnOnce() -> Fut + Send,
) -> std::result::Result<O, ClapMcpError>
where
    Fut: std::future::Future<Output = O> + Send,
    O: Send,
{
    if config.reinvocation_safe && config.share_runtime {
        tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::try_current()
                .map_err(|e| ClapMcpError::RuntimeContext(e.to_string()))?;
            Ok(handle.block_on(f()))
        })
    } else {
        std::thread::scope(|s| {
            let join_handle = s.spawn(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                Ok(rt.block_on(f()))
            });
            match join_handle.join() {
                Ok(inner) => inner,
                Err(e) => Err(ClapMcpError::ToolThread(format!("{:?}", e))),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{ArgAction, CommandFactory};
    use serde_json::json;
    use std::error::Error;
    use std::sync::Mutex;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    fn sample_helper_schema() -> ClapSchema {
        schema_from_command(
            &Command::new("sample")
                .arg(Arg::new("input").help("Input file").required(true).index(1))
                .arg(
                    Arg::new("verbose")
                        .long("verbose")
                        .help("Verbose mode")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("no-cache")
                        .long("no-cache")
                        .help("Disable cache")
                        .action(ArgAction::SetFalse),
                )
                .arg(
                    Arg::new("level")
                        .long("level")
                        .help("Verbosity level")
                        .action(ArgAction::Count),
                )
                .arg(
                    Arg::new("tag")
                        .long("tag")
                        .help("Tags to include")
                        .action(ArgAction::Append)
                        .value_name("TAG"),
                )
                .arg(
                    Arg::new("mode")
                        .long("mode")
                        .help("Execution mode")
                        .action(ArgAction::Set),
                )
                .subcommand(Command::new("serve").about("Serve the sample app")),
        )
    }

    fn nested_schema() -> ClapSchema {
        schema_from_command(
            &Command::new("sample")
                .subcommand(
                    Command::new("parent")
                        .subcommand(Command::new("child").arg(Arg::new("value").long("value"))),
                )
                .subcommand(Command::new("echo").arg(Arg::new("message").long("message"))),
        )
    }

    #[derive(Debug)]
    struct TestError(&'static str);

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for TestError {}

    struct TestPromptProvider {
        response: Result<Vec<PromptMessage>, &'static str>,
        seen: Mutex<Vec<(String, serde_json::Map<String, serde_json::Value>)>>,
    }

    #[async_trait]
    impl content::PromptContentProvider for TestPromptProvider {
        async fn get(
            &self,
            name: &str,
            arguments: &serde_json::Map<String, serde_json::Value>,
        ) -> std::result::Result<Vec<PromptMessage>, Box<dyn Error + Send + Sync>> {
            self.seen
                .lock()
                .expect("prompt provider mutex should lock")
                .push((name.to_string(), arguments.clone()));
            match &self.response {
                Ok(messages) => Ok(messages.clone()),
                Err(message) => Err(Box::new(TestError(message))),
            }
        }
    }

    struct TestResourceProvider {
        response: Result<String, &'static str>,
    }

    #[async_trait]
    impl content::ResourceContentProvider for TestResourceProvider {
        async fn read(
            &self,
            _uri: &str,
        ) -> std::result::Result<String, Box<dyn Error + Send + Sync>> {
            match &self.response {
                Ok(text) => Ok(text.clone()),
                Err(message) => Err(Box::new(TestError(message))),
            }
        }
    }

    #[derive(Debug, clap::Parser)]
    #[command(name = "exec-cli", subcommand_required = true)]
    enum ExecCli {
        PrintOnly,
        PrintAndText,
        Structured,
        Echo {
            #[arg(long)]
            value: String,
        },
    }

    impl ClapMcpToolExecutor for ExecCli {
        fn execute_for_mcp(self) -> Result<ClapMcpToolOutput, ClapMcpToolError> {
            match self {
                Self::PrintOnly => {
                    print!("captured only");
                    Ok(ClapMcpToolOutput::Text(String::new()))
                }
                Self::PrintAndText => {
                    print!("captured extra");
                    Ok(ClapMcpToolOutput::Text("returned text".to_string()))
                }
                Self::Structured => {
                    print!("ignored capture");
                    Ok(ClapMcpToolOutput::Structured(json!({ "status": "ok" })))
                }
                Self::Echo { value } => Ok(ClapMcpToolOutput::Text(value)),
            }
        }
    }

    #[test]
    fn test_format_panic_payload() {
        let s: Box<dyn std::any::Any + Send> = Box::new("hello");
        assert_eq!(format_panic_payload(s.as_ref()), "hello");
        let s: Box<dyn std::any::Any + Send> = Box::new("world".to_string());
        assert_eq!(format_panic_payload(s.as_ref()), "world");
        let n: Box<dyn std::any::Any + Send> = Box::new(42i32);
        assert_eq!(format_panic_payload(n.as_ref()), "<panic>");
    }

    #[test]
    fn test_mcp_type_for_arg_and_description_hints() {
        let boolean_arg = ClapArg {
            id: "verbose".to_string(),
            long: Some("verbose".to_string()),
            short: None,
            help: Some("Verbose mode".to_string()),
            long_help: None,
            required: false,
            global: false,
            index: None,
            action: Some("SetTrue".to_string()),
            value_names: vec![],
            num_args: None,
        };
        let (json_type, items) = mcp_type_for_arg(&boolean_arg);
        assert_eq!(json_type, json!("boolean"));
        assert!(items.is_none());
        assert_eq!(
            mcp_action_description_hint(&boolean_arg),
            Some(" Boolean flag: set to true to pass this flag.".to_string())
        );

        let false_arg = ClapArg {
            action: Some("SetFalse".to_string()),
            ..boolean_arg.clone()
        };
        assert_eq!(mcp_type_for_arg(&false_arg).0, json!("boolean"));
        assert_eq!(
            mcp_action_description_hint(&false_arg),
            Some(" Boolean flag: set to false to pass this flag (e.g. --no-xxx).".to_string())
        );

        let count_arg = ClapArg {
            action: Some("Count".to_string()),
            ..boolean_arg.clone()
        };
        assert_eq!(mcp_type_for_arg(&count_arg).0, json!("integer"));
        assert_eq!(
            mcp_action_description_hint(&count_arg),
            Some(" Number of times the flag is passed (e.g. -vvv).".to_string())
        );

        let append_arg = ClapArg {
            action: Some("Append".to_string()),
            value_names: vec!["TAG".to_string()],
            ..boolean_arg
        };
        let (json_type, items) = mcp_type_for_arg(&append_arg);
        assert_eq!(json_type, json!("array"));
        assert_eq!(
            items,
            Some(json!({ "type": "string", "description": "A TAG value" }))
        );
        assert_eq!(
            mcp_action_description_hint(&append_arg),
            Some(" List of TAG values; pass a JSON array (e.g. [\"a\", \"b\"]).".to_string())
        );

        let multi_value_arg = ClapArg {
            id: "names".to_string(),
            long: Some("name".to_string()),
            short: None,
            help: None,
            long_help: None,
            required: false,
            global: false,
            index: None,
            action: Some("Set".to_string()),
            value_names: vec!["NAME".to_string()],
            num_args: Some("1..".to_string()),
        };
        let (json_type, items) = mcp_type_for_arg(&multi_value_arg);
        assert_eq!(json_type, json!("array"));
        assert_eq!(
            items,
            Some(json!({ "type": "string", "description": "A NAME value" }))
        );
    }

    #[test]
    fn test_command_to_tool_with_config_reflects_arg_shapes() {
        let schema = sample_helper_schema();
        let tool = command_to_tool_with_config(
            &schema.root,
            &ClapMcpConfig {
                reinvocation_safe: true,
                parallel_safe: false,
                share_runtime: true,
                ..Default::default()
            },
            None,
        );

        assert_eq!(tool.name, "sample");
        assert_eq!(tool.description, None);

        let props = tool
            .input_schema
            .properties
            .expect("tool should include input schema properties");
        assert_eq!(tool.input_schema.required, vec!["input".to_string()]);
        assert_eq!(
            props["verbose"]
                .get("type")
                .and_then(|value| value.as_str()),
            Some("boolean")
        );
        assert!(
            props["verbose"]["description"]
                .as_str()
                .expect("verbose description")
                .contains("Boolean flag")
        );
        assert_eq!(
            props["level"].get("type").and_then(|value| value.as_str()),
            Some("integer")
        );
        assert_eq!(
            props["tag"].get("type").and_then(|value| value.as_str()),
            Some("array")
        );
        assert_eq!(
            props["tag"]["items"]["description"].as_str(),
            Some("A TAG value")
        );
        assert_eq!(
            tool.meta
                .as_ref()
                .and_then(|meta| meta.get("clapMcp"))
                .and_then(|value| value.get("shareRuntime"))
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_validate_required_args_handles_missing_empty_and_flag_values() {
        let schema = sample_helper_schema();
        let mut provided = serde_json::Map::new();
        provided.insert("verbose".to_string(), json!(false));
        provided.insert("level".to_string(), json!(0));
        provided.insert("input".to_string(), json!("input.txt"));
        assert!(validate_required_args(&schema, "sample", &provided).is_ok());

        let mut missing_text = serde_json::Map::new();
        missing_text.insert("input".to_string(), json!(""));
        let error = validate_required_args(&schema, "sample", &missing_text)
            .expect_err("empty required string should fail");
        assert!(error.contains("Missing required argument(s): input"));

        let mut missing_array = serde_json::Map::new();
        missing_array.insert("input".to_string(), json!([]));
        let error = validate_required_args(&schema, "sample", &missing_array)
            .expect_err("empty array should fail");
        assert!(error.contains("input"));

        assert!(validate_required_args(&schema, "unknown", &serde_json::Map::new()).is_ok());
    }

    #[test]
    fn test_build_tool_argv_handles_positional_flags_and_lists() {
        let schema = sample_helper_schema();
        let arguments = serde_json::Map::from_iter([
            ("input".to_string(), json!("input.txt")),
            ("verbose".to_string(), json!(true)),
            ("no-cache".to_string(), json!(false)),
            ("level".to_string(), json!(2)),
            ("tag".to_string(), json!(["alpha", "", "beta"])),
            ("mode".to_string(), json!("fast")),
        ]);

        let argv = build_tool_argv(&schema, "sample", arguments);
        assert_eq!(
            argv,
            vec![
                "input.txt",
                "--level",
                "--level",
                "--mode",
                "fast",
                "--no-cache",
                "--tag",
                "alpha",
                "--tag",
                "beta",
                "--verbose",
            ]
        );
    }

    #[test]
    fn test_value_to_string_and_value_to_strings_cover_scalar_and_array_inputs() {
        assert_eq!(value_to_string(&json!("hello")), Some("hello".to_string()));
        assert_eq!(value_to_string(&json!(3)), Some("3".to_string()));
        assert_eq!(value_to_string(&json!(false)), Some("false".to_string()));
        assert_eq!(value_to_string(&serde_json::Value::Null), None);
        assert_eq!(
            value_to_string(&json!({"name":"sample"})),
            Some("{\"name\":\"sample\"}".to_string())
        );

        assert_eq!(
            value_to_strings(&json!(["alpha", "", 3, null, false])),
            Some(vec![
                "alpha".to_string(),
                "3".to_string(),
                "false".to_string()
            ])
        );
        assert_eq!(
            value_to_strings(&json!("solo")),
            Some(vec!["solo".to_string()])
        );
        assert_eq!(value_to_strings(&serde_json::Value::Null), None);
    }

    #[test]
    fn test_command_flag_helpers_are_idempotent() {
        let cmd = command_with_mcp_flag(command_with_mcp_flag(Command::new("sample")));
        let mcp_args = cmd
            .get_arguments()
            .filter(|arg| arg.get_long() == Some(MCP_FLAG_LONG))
            .count();
        assert_eq!(mcp_args, 1);

        let cmd = command_with_export_skills_flag(command_with_export_skills_flag(Command::new(
            "sample",
        )));
        let export_args = cmd
            .get_arguments()
            .filter(|arg| arg.get_long() == Some(EXPORT_SKILLS_FLAG_LONG))
            .count();
        assert_eq!(export_args, 1);
    }

    #[tokio::test]
    async fn test_resource_helpers_cover_builtin_custom_and_error_paths() {
        let custom = vec![content::CustomResource {
            uri: "test://dynamic".to_string(),
            name: "dynamic".to_string(),
            title: None,
            description: Some("dynamic resource".to_string()),
            mime_type: Some("text/plain".to_string()),
            content: content::ResourceContent::Dynamic(Arc::new(TestResourceProvider {
                response: Ok("dynamic body".to_string()),
            })),
        }];

        let listed = list_resources_result(&custom);
        assert_eq!(listed.resources.len(), 2);
        assert_eq!(listed.resources[0].uri, MCP_RESOURCE_URI_SCHEMA);
        assert_eq!(listed.resources[1].uri, "test://dynamic");

        let schema_read = read_resource_result(
            "{\"name\":\"sample\"}",
            &custom,
            ReadResourceRequestParams {
                uri: MCP_RESOURCE_URI_SCHEMA.to_string(),
                meta: None,
            },
        )
        .await
        .expect("schema resource should resolve");
        let text = match &schema_read.contents[0] {
            ReadResourceContent::TextResourceContents(text) => &text.text,
            other => panic!("unexpected content: {other:?}"),
        };
        assert!(text.contains("\"name\":\"sample\""));

        let custom_read = read_resource_result(
            "{}",
            &custom,
            ReadResourceRequestParams {
                uri: "test://dynamic".to_string(),
                meta: None,
            },
        )
        .await
        .expect("custom resource should resolve");
        let text = match &custom_read.contents[0] {
            ReadResourceContent::TextResourceContents(text) => &text.text,
            other => panic!("unexpected content: {other:?}"),
        };
        assert_eq!(text, "dynamic body");

        let missing = read_resource_result(
            "{}",
            &custom,
            ReadResourceRequestParams {
                uri: "test://missing".to_string(),
                meta: None,
            },
        )
        .await
        .expect_err("missing resource should error");
        assert!(missing.message.contains("unknown resource uri"));

        let failing_resources = vec![content::CustomResource {
            uri: "test://broken".to_string(),
            name: "broken".to_string(),
            title: None,
            description: None,
            mime_type: None,
            content: content::ResourceContent::Dynamic(Arc::new(TestResourceProvider {
                response: Err("read failed"),
            })),
        }];
        let failing = read_resource_result(
            "{}",
            &failing_resources,
            ReadResourceRequestParams {
                uri: "test://broken".to_string(),
                meta: None,
            },
        )
        .await
        .expect_err("provider failure should map to rpc error");
        assert_eq!(failing.message, "read failed");
    }

    #[tokio::test]
    async fn test_prompt_helpers_cover_logging_custom_and_error_paths() {
        let provider = Arc::new(TestPromptProvider {
            response: Ok(vec![PromptMessage {
                role: Role::User,
                content: ContentBlock::text_content("dynamic prompt".to_string()),
            }]),
            seen: Mutex::new(Vec::new()),
        });
        let prompts = vec![content::CustomPrompt {
            name: "dynamic".to_string(),
            title: Some("Dynamic".to_string()),
            description: Some("dynamic prompt".to_string()),
            arguments: vec![],
            content: content::PromptContent::Dynamic(provider.clone()),
        }];

        let listed = list_prompts_result(true, &prompts);
        assert_eq!(listed.prompts.len(), 2);
        assert_eq!(listed.prompts[0].name, PROMPT_LOGGING_GUIDE);
        assert_eq!(listed.prompts[1].name, "dynamic");

        let logging_prompt = get_prompt_result(
            true,
            &prompts,
            GetPromptRequestParams {
                name: PROMPT_LOGGING_GUIDE.to_string(),
                arguments: None,
                meta: None,
            },
        )
        .await
        .expect("logging guide should resolve");
        assert!(
            logging_prompt.messages[0]
                .content
                .as_text_content()
                .expect("logging guide should be text")
                .text
                .contains("logger")
        );

        let dynamic_prompt = get_prompt_result(
            false,
            &prompts,
            GetPromptRequestParams {
                name: "dynamic".to_string(),
                arguments: Some(std::collections::HashMap::from([(
                    "topic".to_string(),
                    "coverage".to_string(),
                )])),
                meta: None,
            },
        )
        .await
        .expect("dynamic prompt should resolve");
        assert_eq!(
            dynamic_prompt.description.as_deref(),
            Some("dynamic prompt")
        );
        assert_eq!(
            provider
                .seen
                .lock()
                .expect("provider seen mutex should lock")[0]
                .1
                .get("topic")
                .and_then(|value| value.as_str()),
            Some("coverage")
        );

        let unknown_logging = get_prompt_result(
            false,
            &prompts,
            GetPromptRequestParams {
                name: PROMPT_LOGGING_GUIDE.to_string(),
                arguments: None,
                meta: None,
            },
        )
        .await
        .expect_err("logging guide should error when logging disabled");
        assert!(unknown_logging.message.contains("unknown prompt"));

        let failing_prompts = vec![content::CustomPrompt {
            name: "broken".to_string(),
            title: None,
            description: None,
            arguments: vec![],
            content: content::PromptContent::Dynamic(Arc::new(TestPromptProvider {
                response: Err("prompt failed"),
                seen: Mutex::new(Vec::new()),
            })),
        }];
        let failing = get_prompt_result(
            false,
            &failing_prompts,
            GetPromptRequestParams {
                name: "broken".to_string(),
                arguments: None,
                meta: None,
            },
        )
        .await
        .expect_err("provider failure should map to rpc error");
        assert_eq!(failing.message, "prompt failed");
    }

    #[test]
    fn test_call_tool_result_helpers_cover_text_structured_errors_and_panics() {
        let text = call_tool_result_from_output(ClapMcpToolOutput::Text("hello".to_string()));
        assert_eq!(text.is_error, None);
        assert_eq!(
            text.content[0]
                .as_text_content()
                .expect("text result should be text")
                .text,
            "hello"
        );

        let structured = call_tool_result_from_output(ClapMcpToolOutput::Structured(json!({
            "sum": 5
        })));
        assert_eq!(
            structured
                .structured_content
                .as_ref()
                .and_then(|content| content.get("sum"))
                .and_then(|value| value.as_i64()),
            Some(5)
        );
        assert!(
            structured.content[0]
                .as_text_content()
                .expect("structured result should emit text")
                .text
                .contains("\"sum\": 5")
        );

        let non_object = call_tool_result_from_output(ClapMcpToolOutput::Structured(json!(["a"])));
        assert!(non_object.structured_content.is_none());

        let error = call_tool_result_from_tool_error(ClapMcpToolError::structured(
            "bad",
            json!({ "code": 7 }),
        ));
        assert_eq!(error.is_error, Some(true));
        assert_eq!(
            error
                .structured_content
                .as_ref()
                .and_then(|content| content.get("code"))
                .and_then(|value| value.as_i64()),
            Some(7)
        );

        let panic_payload: Box<dyn std::any::Any + Send> = Box::new("boom");
        let panic_result = call_tool_result_from_panic(panic_payload.as_ref());
        assert_eq!(panic_result.is_error, Some(true));
        assert!(
            panic_result.content[0]
                .as_text_content()
                .expect("panic result should be text")
                .text
                .contains("Tool panicked: boom")
        );
    }

    #[test]
    fn test_subprocess_helpers_cover_command_building_logging_and_result_shapes() {
        let schema = nested_schema();
        let args = serde_json::Map::from_iter([(
            "value".to_string(),
            serde_json::Value::String("ok".to_string()),
        )]);
        let command = build_execution_command(
            std::path::Path::new("/tmp/example"),
            &schema,
            "sample",
            "child",
            &args,
        );
        assert_eq!(command.get_program(), std::ffi::OsStr::new("/tmp/example"));
        let actual_args: Vec<_> = command.get_args().collect();
        assert_eq!(
            actual_args,
            vec![
                std::ffi::OsStr::new("parent"),
                std::ffi::OsStr::new("child"),
                std::ffi::OsStr::new("--value"),
                std::ffi::OsStr::new("ok"),
            ]
        );

        let log_params = subprocess_stderr_log_params("child", "warning on stderr\n")
            .expect("stderr should produce logging params");
        assert_eq!(log_params.logger.as_deref(), Some("stderr"));
        assert_eq!(
            log_params.meta.as_ref().and_then(|meta| meta.get("tool")),
            Some(&serde_json::Value::String("child".to_string()))
        );
        assert!(subprocess_stderr_log_params("child", "   ").is_none());

        #[cfg(unix)]
        {
            let success_output = std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"done\n".to_vec(),
                stderr: b"note\n".to_vec(),
            };
            let success = call_tool_result_from_subprocess_output(&success_output);
            assert_eq!(success.is_error, None);
            assert!(
                success.content[0]
                    .as_text_content()
                    .expect("success result should be text")
                    .text
                    .contains("stderr:\nnote")
            );

            let failure_output = std::process::Output {
                status: std::process::ExitStatus::from_raw(256),
                stdout: Vec::new(),
                stderr: b"boom\n".to_vec(),
            };
            let failure = call_tool_result_from_subprocess_output(&failure_output);
            assert_eq!(failure.is_error, Some(true));
            assert!(
                failure.content[0]
                    .as_text_content()
                    .expect("failure result should be text")
                    .text
                    .contains("non-zero status")
            );
        }

        let launch_error = command_launch_failure_result(&std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        ));
        assert_eq!(launch_error.is_error, Some(true));
        assert!(
            launch_error.content[0]
                .as_text_content()
                .expect("launch error should be text")
                .text
                .contains("Failed to run command")
        );

        let placeholder = placeholder_tool_result(
            "echo",
            &serde_json::Map::from_iter([("message".to_string(), json!("hi"))]),
        );
        assert!(
            placeholder.content[0]
                .as_text_content()
                .expect("placeholder result should be text")
                .text
                .contains("Would invoke clap command 'echo'")
        );

        let parse_failure = schema_parse_failure_result();
        assert_eq!(parse_failure.is_error, Some(true));
        assert_eq!(
            parse_failure.content[0]
                .as_text_content()
                .expect("schema parse failure should be text")
                .text,
            "Failed to parse schema"
        );
    }

    #[test]
    fn test_validate_tool_argument_names_rejects_unknown_keys() {
        let tool = command_to_tool_with_config(
            &sample_helper_schema().root,
            &ClapMcpConfig::default(),
            None,
        );
        let ok_args = serde_json::Map::from_iter([("input".to_string(), json!("in.txt"))]);
        assert!(validate_tool_argument_names(&tool, &tool.name, &ok_args).is_ok());

        let bad_args = serde_json::Map::from_iter([("bogus".to_string(), json!(1))]);
        let err = validate_tool_argument_names(&tool, &tool.name, &bad_args)
            .expect_err("unknown key should error");
        assert!(format!("{err:?}").contains("unknown argument: bogus"));
    }

    #[test]
    fn test_into_clap_mcp_result_and_error_impls_cover_basic_conversions() {
        assert!(matches!(
            String::from("hello")
                .into_tool_result()
                .expect("string should convert"),
            ClapMcpToolOutput::Text(text) if text == "hello"
        ));
        assert!(matches!(
            "world"
                .into_tool_result()
                .expect("str should convert"),
            ClapMcpToolOutput::Text(text) if text == "world"
        ));

        let structured = AsStructured(json!({ "ok": true }))
            .into_tool_result()
            .expect("structured value should convert");
        assert!(matches!(structured, ClapMcpToolOutput::Structured(_)));

        let empty = Option::<String>::None
            .into_tool_result()
            .expect("none should convert");
        assert!(matches!(empty, ClapMcpToolOutput::Text(text) if text.is_empty()));

        let some = Some("x").into_tool_result().expect("some should convert");
        assert!(matches!(some, ClapMcpToolOutput::Text(text) if text == "x"));

        let ok_result: Result<&str, &str> = Ok("done");
        assert!(matches!(
            ok_result.into_tool_result().expect("ok result should convert"),
            ClapMcpToolOutput::Text(text) if text == "done"
        ));

        let err_result: Result<&str, &str> = Err("boom");
        let err = err_result
            .into_tool_result()
            .expect_err("err result should map to tool error");
        assert_eq!(err.message, "boom");

        assert_eq!(ClapMcpToolError::from("oops").message, "oops");
        assert_eq!(ClapMcpToolError::from(String::from("ouch")).message, "ouch");
        assert_eq!(String::from("bad").into_tool_error().message, "bad");
        assert_eq!("worse".into_tool_error().message, "worse");
    }

    #[test]
    fn test_merge_captured_stdout_only_changes_text_outputs() {
        let merged = merge_captured_stdout(
            Ok(ClapMcpToolOutput::Text(String::new())),
            "captured only\n".to_string(),
        )
        .expect("merge should succeed");
        assert!(matches!(merged, ClapMcpToolOutput::Text(text) if text == "captured only"));

        let appended = merge_captured_stdout(
            Ok(ClapMcpToolOutput::Text("returned".to_string())),
            "captured\n".to_string(),
        )
        .expect("append should succeed");
        assert!(matches!(appended, ClapMcpToolOutput::Text(text) if text == "returned\ncaptured"));

        let structured = merge_captured_stdout(
            Ok(ClapMcpToolOutput::Structured(json!({"ok": true}))),
            "captured\n".to_string(),
        )
        .expect("structured output should pass through");
        assert!(matches!(structured, ClapMcpToolOutput::Structured(_)));
    }

    #[test]
    fn test_execute_in_process_command_and_handler_cover_capture_stdout_paths() {
        let schema = schema_from_command(&ExecCli::command());

        let structured = execute_in_process_command::<ExecCli>(
            &schema,
            "structured",
            serde_json::Map::new(),
            false,
        )
        .expect("structured should execute");
        assert!(matches!(structured, ClapMcpToolOutput::Structured(_)));

        let echo_args = serde_json::Map::from_iter([("value".to_string(), json!("hello"))]);
        let handler = make_in_process_handler::<ExecCli>(schema.clone(), false);
        let echoed = handler("echo", echo_args).expect("handler should execute");
        assert!(matches!(echoed, ClapMcpToolOutput::Text(text) if text == "hello"));

        let missing =
            execute_in_process_command::<ExecCli>(&schema, "echo", serde_json::Map::new(), false)
                .expect_err("missing required arg should fail");
        assert!(
            missing
                .message
                .contains("Missing required argument(s): value")
        );
    }
}
