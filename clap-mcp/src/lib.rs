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
//!     let cli = Cli::parse_or_serve_mcp();
//!     println!("{}", run(cli));
//! }
//! ```
//!
//! Run with `--mcp` to start the MCP server instead of executing the CLI.

use clap::{Arg, ArgAction, Command};
use rmcp::model::{Meta, TaskSupport, Tool, ToolExecution};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

mod server;

#[cfg(feature = "http")]
mod http;

mod serve;

#[cfg(feature = "http-oauth")]
pub mod oauth;

pub use rmcp::model::ErrorData as ClapMcpErrorData;

pub mod logging;

/// Custom MCP resources and prompts, and skill export.
pub mod content;

#[cfg(feature = "derive")]
pub use clap_mcp_macros::ClapMcp;
pub use serve::{ServeMcp, ServeMcpBuilder};

/// Convenience macro for struct root + subcommand CLIs: parse root then run.
///
/// Expands to: parse the root with [`ParseOrServeMcp::parse_or_serve_mcp`], then evaluate the given
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
        let $args = <$root as $crate::ParseOrServeMcp>::parse_or_serve_mcp();
        $run_expr
    }};
    ($root:ty, $run_expr:expr) => {{
        macro_rules! __clap_mcp_with_args {
            ($args:ident, $expr:expr) => {{
                let $args = <$root as $crate::ParseOrServeMcp>::parse_or_serve_mcp();
                $expr
            }};
        }
        __clap_mcp_with_args!(args, $run_expr)
    }};
}

/// Long flag that triggers MCP server mode. Add to your CLI via [`command_with_mcp_flag`].
pub const MCP_FLAG_LONG: &str = "mcp";

/// Stable clap arg id for the stdio MCP flag (internal; [`ClapMcpBuiltinFlags::stdio_long`] is user-facing).
pub const CLAP_MCP_STDIO_FLAG_ID: &str = "clap_mcp_stdio";

/// Stable clap arg id for the HTTP MCP flag (`http` feature).
#[cfg(feature = "http")]
pub const CLAP_MCP_HTTP_FLAG_ID: &str = "clap_mcp_http";

/// Stable clap arg id for the export-skills flag.
pub const CLAP_MCP_EXPORT_SKILLS_FLAG_ID: &str = "clap_mcp_export_skills";

/// Legacy arg id used before stable ids; still recognized in [`matches_stdio_flag`].
pub(crate) const CLAP_MCP_STDIO_FLAG_ID_LEGACY: &str = "mcp";

/// Long flag for Streamable HTTP MCP server (`http` feature).
#[cfg(feature = "http")]
pub const MCP_HTTP_FLAG_LONG: &str = "mcp-http";

/// Environment variable for HTTP bind host when [`MCP_HTTP_LISTEN_ENV`] is unset.
#[cfg(feature = "http")]
pub const MCP_HTTP_BIND_ENV: &str = "CLAP_MCP_HTTP_BIND";

/// Environment variable for HTTP listen address (`host:port`).
#[cfg(feature = "http")]
pub const MCP_HTTP_LISTEN_ENV: &str = "CLAP_MCP_HTTP_LISTEN";

/// Environment variable for HTTP port when [`MCP_HTTP_LISTEN_ENV`] is unset (requires [`MCP_HTTP_BIND_ENV`]).
#[cfg(feature = "http")]
pub const MCP_HTTP_PORT_ENV: &str = "CLAP_MCP_HTTP_PORT";

/// Long flag that triggers [Agent Skills](https://agentskills.io/specification) export (generates SKILL.md). Add via [`command_with_export_skills_flag`].
pub const EXPORT_SKILLS_FLAG_LONG: &str = "export-skills";

/// User-facing long names for clap-mcp builtin global flags (stdio, HTTP, export-skills).
///
/// Override via `#[clap_mcp(mcp_flag = "...")]` on the derive or [`ClapMcpConfig::builtin_flags`]
/// when your CLI already uses `--mcp` for something else. Values must be `'static` str literals
/// (clap stores long names with static lifetime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClapMcpBuiltinFlags {
    /// Long name for stdio MCP (default [`MCP_FLAG_LONG`]).
    pub stdio_long: &'static str,
    /// Long name for HTTP MCP (default [`MCP_HTTP_FLAG_LONG`], `http` feature).
    #[cfg(feature = "http")]
    pub http_long: &'static str,
    /// Long name for export-skills (default [`EXPORT_SKILLS_FLAG_LONG`]).
    pub export_skills_long: &'static str,
}

impl Default for ClapMcpBuiltinFlags {
    fn default() -> Self {
        Self {
            stdio_long: MCP_FLAG_LONG,
            #[cfg(feature = "http")]
            http_long: MCP_HTTP_FLAG_LONG,
            export_skills_long: EXPORT_SKILLS_FLAG_LONG,
        }
    }
}

impl ClapMcpBuiltinFlags {
    /// Override the stdio MCP flag long name (without `--`).
    pub const fn with_stdio_long(mut self, long: &'static str) -> Self {
        self.stdio_long = long;
        self
    }

    /// Override the export-skills flag long name (without `--`).
    pub const fn with_export_skills_long(mut self, long: &'static str) -> Self {
        self.export_skills_long = long;
        self
    }

    /// Override the HTTP MCP flag long name (`http` feature).
    #[cfg(feature = "http")]
    pub const fn with_http_long(mut self, long: &'static str) -> Self {
        self.http_long = long;
        self
    }
}

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

/// Provides MCP schema metadata (skip, requires, task tools) from `#[clap_mcp(skip)]`,
/// `#[clap_mcp(requires = "arg_name")]`, and optional `#[clap_mcp(task)]` on variants.
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

/// In-process MCP tool execution with session state shared across `tools/call` invocations.
///
/// Implemented by `#[derive(ClapMcp)]` when using `#[clap_mcp_output_from_with_state = "run"]`.
/// The MCP server stores state in an [`Arc`] for its lifetime and passes **`&Self::State`** on
/// each tool call (not `&Arc<…>` — the shared pointer is an implementation detail).
///
/// # Setup
///
/// * Set [`ClapMcpConfig::reinvocation_safe`](ClapMcpConfig::reinvocation_safe) — subprocess mode
///   cannot share in-process state.
/// * On the **leaf** command enum, set `#[clap_mcp_output_from_with_state = "run"]` and
///   `#[clap_mcp_state_type = "…"]`. The state type must match the second parameter of your
///   `run` function (e.g. `run(cmd, state: &Mutex<CounterState>)` →
///   `#[clap_mcp_state_type = "Mutex<CounterState>"]`).
/// * On struct roots or intermediate subcommand enums, add `#[clap_mcp(stateful)]` and delegate;
///   `State` is inferred from the subcommand field (no duplicate `state_type`).
///
/// # Example
///
/// ```rust,ignore
/// use clap::{Parser, Subcommand};
/// use clap_mcp::{ClapMcp, ParseOrServeMcpWithState};
/// use std::sync::{Arc, Mutex};
///
/// #[derive(Default)]
/// struct CounterState { count: u64 }
///
/// #[derive(Parser, ClapMcp)]
/// #[clap_mcp(reinvocation_safe = true, stateful)]
/// struct App {
///     #[command(subcommand)]
///     command: Command,
/// }
///
/// #[derive(Subcommand, ClapMcp)]
/// #[clap_mcp(reinvocation_safe = true)]
/// #[clap_mcp_output_from_with_state = "run"]
/// #[clap_mcp_state_type = "Mutex<CounterState>"]
/// enum Command { Increment, Read }
///
/// fn run(cmd: Command, state: &Mutex<CounterState>) -> String { /* ... */ }
///
/// fn main() {
///     let state = Arc::new(Mutex::new(CounterState::default()));
///     let _ = App::parse_or_serve_mcp_with_state(state);
/// }
/// ```
///
/// See the `stateful_counter` example and
/// [PR #11](https://github.com/canardleteer/clap-mcp/pull/11) (Eddy Stefes / fneddy) for the
/// original motivation: counters, open handles, and in-memory caches across MCP tool calls
/// without globals or manual handler wiring.
pub trait ClapMcpToolExecutorWithState {
    /// Session state type; must match the second parameter of your stateful `run` function.
    type State: Send + Sync + 'static;

    /// Run this parsed CLI value as an MCP tool, reading/updating shared `state`.
    fn execute_for_mcp_with_state(
        self,
        state: &Self::State,
    ) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError>;
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
    #[error("invalid MCP configuration: {0}")]
    InvalidConfig(String),
    /// Returned when [`ServeMcpBuilder::serve`] or [`serve_mcp`] runs on a
    /// `current_thread` runtime but [`ClapMcpConfig::needs_multi_thread_runtime`] is true.
    #[error("multi-thread tokio runtime required: {reason}")]
    RequiresMultiThreadRuntime { reason: String },
    #[error("failed to serialize clap schema to JSON: {0}")]
    SchemaJson(#[from] serde_json::Error),
    #[error("MCP service error: {0}")]
    Mcp(#[from] Box<rmcp::RmcpError>),
    #[error("MCP service runtime error: {0}")]
    Service(#[from] rmcp::ServiceError),
    #[error("MCP join error: {0}")]
    Join(#[from] tokio::task::JoinError),
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

    /// When true (default), `myapp --mcp` (or `--mcp-http`) may start the MCP server without a
    /// subcommand on the argv, by inspecting argv **before** clap runs. This preserves CLIs that
    /// use `subcommand_required = true` — you do not need `Option<Commands>` for MCP.
    ///
    /// When false, `--mcp` alone goes through normal clap parsing; use `subcommand_required =
    /// false` (typically with `Option<Commands>`) if clap must accept `--mcp` without a subcommand
    /// token.
    pub allow_mcp_without_subcommand: bool,

    /// Long names for clap-mcp builtin global flags (`--mcp`, `--mcp-http`, `--export-skills`).
    pub builtin_flags: ClapMcpBuiltinFlags,
}

impl Default for ClapMcpConfig {
    fn default() -> Self {
        Self {
            reinvocation_safe: false,
            parallel_safe: false,
            share_runtime: false,
            catch_in_process_panics: false,
            allow_mcp_without_subcommand: true,
            builtin_flags: ClapMcpBuiltinFlags::default(),
        }
    }
}

impl ClapMcpConfig {
    /// Whether in-process MCP needs a multi-thread tokio runtime
    /// (`share_runtime` or `parallel_safe` with `reinvocation_safe`).
    ///
    /// When true, async [`ServeMcpBuilder::serve`] on an existing runtime requires
    /// `#[tokio::main(flavor = "multi_thread")]`. [`ServeMcpBuilder::serve_blocking`]
    /// and [`serve_mcp_blocking`] create a suitable runtime internally.
    pub fn needs_multi_thread_runtime(&self) -> bool {
        self.reinvocation_safe && (self.share_runtime || self.parallel_safe)
    }
}

pub(crate) fn build_mcp_blocking_runtime(
    config: &ClapMcpConfig,
) -> Result<tokio::runtime::Runtime, ClapMcpError> {
    let rt = if config.needs_multi_thread_runtime() {
        tokio::runtime::Builder::new_multi_thread()
    } else {
        tokio::runtime::Builder::new_current_thread()
    }
    .enable_all()
    .build()?;
    Ok(rt)
}

/// Derive-path and imperative MCP run options (execution config + serve behavior).
#[derive(Debug)]
pub struct ClapMcpRunOptions {
    pub config: ClapMcpConfig,
    pub serve: ClapMcpServeOptions,
}

impl ClapMcpRunOptions {
    /// Build options with default [`ClapMcpServeOptions`].
    pub fn from_config(config: ClapMcpConfig) -> Self {
        Self {
            config,
            serve: ClapMcpServeOptions::default(),
        }
    }
}

impl From<ClapMcpConfig> for ClapMcpRunOptions {
    fn from(config: ClapMcpConfig) -> Self {
        Self::from_config(config)
    }
}

/// MCP transport listen target for low-level embedders.
///
/// Use with [`ServeMcpBuilder`] (recommended), [`serve_mcp`], or [`serve_mcp_blocking`].
#[derive(Debug, Clone, Copy)]
pub enum McpListen {
    /// Stdio MCP (default `--mcp` mode).
    Stdio,
    /// Streamable HTTP at the given socket address (`http` feature).
    #[cfg(feature = "http")]
    Http(std::net::SocketAddr),
}

/// Optional configuration for MCP serve behavior (logging, etc.).
///
/// Pass to [`parse_or_serve_mcp_with`], [`ServeMcpBuilder::serve_options`],
/// or the lower-level [`serve_mcp`] / [`serve_mcp_blocking`] functions.
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
/// // Pass opts to parse_or_serve_mcp_with or ServeMcpBuilder::serve_options
/// ```
#[derive(Debug, Default)]
pub struct ClapMcpServeOptions {
    /// When set, log messages received on this channel are forwarded to the MCP client
    /// via `notifications/message`. Enables the logging capability and instructions.
    pub log_rx: Option<tokio::sync::mpsc::Receiver<logging::LoggingMessageNotificationParams>>,

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

    /// When true (requires `elicitation` feature), tools named `confirm-echo` may call `peer.elicit` during execution.
    pub elicitation_enabled: bool,
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
    /// Subcommand tool names that may be invoked with MCP task-augmented `tools/call` when
    /// [`ClapMcpSchemaMetadata::task_augmented_tools`] is enabled. Populated by `#[clap_mcp(task)]` on
    /// enum variants. When **empty**, every tool is eligible for task augmentation (when enabled
    /// in metadata). When **non-empty**, only listed tool names are eligible.
    pub task_tool_names: Vec<String>,
    /// When true, advertise MCP task support and handle task-augmented `tools/call`.
    /// Set by `#[clap_mcp(task_augmented_tools)]` on the derive (requires `reinvocation_safe`).
    pub task_augmented_tools: bool,
    /// Optional JSON schema for tool output. When set (e.g. via `#[clap_mcp_output_type]` or
    /// `#[clap_mcp_output_one_of]` with the `output-schema` feature), this schema is attached
    /// to each tool's `output_schema` field.
    pub output_schema: Option<serde_json::Value>,
}

pub(crate) fn tool_task_eligible(tool_name: &str, metadata: &ClapMcpSchemaMetadata) -> bool {
    if !metadata.task_augmented_tools {
        return false;
    }
    if metadata.task_tool_names.is_empty() {
        true
    } else {
        metadata.task_tool_names.iter().any(|n| n == tool_name)
    }
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

/// Arg prefix before the first standalone `--` (clap end-of-options). Passthrough tokens after `--` are excluded.
pub(crate) fn argv_before_end_of_opts(args: &[String]) -> &[String] {
    match args.iter().position(|a| a == "--") {
        Some(i) => &args[..i],
        None => args,
    }
}

fn argv_has_long_flag(prefix: &[String], long: &str) -> bool {
    let flag = format!("--{long}");
    let flag_eq_prefix = format!("--{long}=");
    prefix
        .iter()
        .any(|a| a.as_str() == flag || a.starts_with(&flag_eq_prefix))
}

fn command_has_arg_id_or_long(cmd: &Command, id: &str, long: &str) -> bool {
    cmd.get_arguments().any(|a| {
        a.get_id() == id
            || a.get_id() == CLAP_MCP_STDIO_FLAG_ID_LEGACY && long == MCP_FLAG_LONG
            || a.get_long().is_some_and(|l| l == long)
    })
}

pub(crate) fn matches_stdio_flag(matches: &clap::ArgMatches, _flags: &ClapMcpBuiltinFlags) -> bool {
    matches.get_flag(CLAP_MCP_STDIO_FLAG_ID)
}

#[cfg(feature = "http")]
pub(crate) fn matches_http_flag(matches: &clap::ArgMatches, flags: &ClapMcpBuiltinFlags) -> bool {
    matches.contains_id(CLAP_MCP_HTTP_FLAG_ID)
        || (flags.http_long == MCP_HTTP_FLAG_LONG && matches.contains_id(MCP_HTTP_FLAG_LONG))
}

/// Arg IDs omitted from MCP tool arguments (built-in / clap-mcp global flags).
pub(crate) fn is_builtin_arg(id: &str) -> bool {
    is_clap_mcp_builtin_arg_id(id)
}

pub(crate) fn is_clap_mcp_builtin_arg_id(id: &str) -> bool {
    matches!(
        id,
        "help"
            | "version"
            | CLAP_MCP_STDIO_FLAG_ID
            | CLAP_MCP_EXPORT_SKILLS_FLAG_ID
            | EXPORT_SKILLS_FLAG_LONG
    ) || {
        #[cfg(feature = "http")]
        {
            id == CLAP_MCP_HTTP_FLAG_ID || id == MCP_HTTP_FLAG_LONG
        }
        #[cfg(not(feature = "http"))]
        {
            let _ = id;
            false
        }
    }
}

fn is_omitted_schema_clap_arg(arg: &clap::Arg) -> bool {
    let id = arg.get_id().as_str();
    is_clap_mcp_builtin_arg_id(id)
}

/// Builds MCP tools from a clap schema with execution config and metadata.
///
/// One tool per command (root + every subcommand). Tools include `meta.clapMcp` with
/// `reinvocationSafe`, `parallelSafe`, and optional `taskAugmented` hints.
pub fn tools_from_schema_with_metadata(
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
        .map(|cmd| {
            command_to_tool_with_config(cmd, config, metadata, metadata.output_schema.as_ref())
        })
        .collect()
}

fn command_to_tool_with_config(
    cmd: &ClapCommand,
    config: &ClapMcpConfig,
    metadata: &ClapMcpSchemaMetadata,
    output_schema: Option<&serde_json::Value>,
) -> Tool {
    let args: Vec<&ClapArg> = cmd
        .args
        .iter()
        .filter(|a| !is_builtin_arg(a.id.as_str()))
        .collect();

    let mut properties: BTreeMap<String, serde_json::Map<String, serde_json::Value>> =
        BTreeMap::new();
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

    let mut input_schema = serde_json::Map::new();
    input_schema.insert("type".into(), serde_json::json!("object"));
    input_schema.insert(
        "properties".into(),
        serde_json::Value::Object(
            properties
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::Object(v)))
                .collect(),
        ),
    );
    if !required.is_empty() {
        input_schema.insert(
            "required".into(),
            serde_json::Value::Array(
                required
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    let description = cmd
        .long_about
        .as_deref()
        .or(cmd.about.as_deref())
        .map(String::from);
    let title = cmd.about.as_ref().map(String::from);

    let meta = {
        let mut clap_mcp = serde_json::Map::new();
        clap_mcp.insert(
            "reinvocationSafe".into(),
            serde_json::Value::Bool(config.reinvocation_safe),
        );
        clap_mcp.insert(
            "parallelSafe".into(),
            serde_json::Value::Bool(config.parallel_safe),
        );
        clap_mcp.insert(
            "shareRuntime".into(),
            serde_json::Value::Bool(config.share_runtime),
        );
        if tool_task_eligible(&cmd.name, metadata) {
            clap_mcp.insert("taskAugmented".into(), serde_json::Value::Bool(true));
        }
        let mut m = Meta::default();
        m.0.insert("clapMcp".into(), serde_json::Value::Object(clap_mcp));
        Some(m)
    };

    let execution = if tool_task_eligible(&cmd.name, metadata) {
        Some(ToolExecution::from_raw(Some(TaskSupport::Optional)))
    } else {
        None
    };

    let mut tool = Tool::new_with_raw(
        cmd.name.clone(),
        description.map(|d| d.into()),
        Arc::new(input_schema),
    );
    if let Some(title) = title {
        tool = tool.with_title(title);
    }
    if let Some(meta) = meta {
        tool = tool.with_meta(meta);
    }
    if let Some(execution) = execution {
        tool = tool.with_execution(execution);
    }
    if let Some(output_schema) = output_schema.cloned().and_then(|v| v.as_object().cloned()) {
        tool = tool.with_raw_output_schema(Arc::new(output_schema));
    }
    tool
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
pub fn command_with_mcp_flag(cmd: Command) -> Command {
    command_with_mcp_flag_with_flags(cmd, &ClapMcpBuiltinFlags::default())
}

/// Like [`command_with_mcp_flag`] but uses `flags.stdio_long` for the user-facing long name.
pub fn command_with_mcp_flag_with_flags(mut cmd: Command, flags: &ClapMcpBuiltinFlags) -> Command {
    if command_has_arg_id_or_long(&cmd, CLAP_MCP_STDIO_FLAG_ID, flags.stdio_long) {
        return cmd;
    }

    cmd = cmd.arg(
        Arg::new(CLAP_MCP_STDIO_FLAG_ID)
            .long(flags.stdio_long)
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
pub fn command_with_export_skills_flag(cmd: Command) -> Command {
    command_with_export_skills_flag_with_flags(cmd, &ClapMcpBuiltinFlags::default())
}

/// Like [`command_with_export_skills_flag`] but uses `flags.export_skills_long`.
pub fn command_with_export_skills_flag_with_flags(
    mut cmd: Command,
    flags: &ClapMcpBuiltinFlags,
) -> Command {
    if command_has_arg_id_or_long(
        &cmd,
        CLAP_MCP_EXPORT_SKILLS_FLAG_ID,
        flags.export_skills_long,
    ) {
        return cmd;
    }

    cmd = cmd.arg(
        Arg::new(CLAP_MCP_EXPORT_SKILLS_FLAG_ID)
            .long(flags.export_skills_long)
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
    command_with_mcp_and_export_skills_flags_with_flags(cmd, &ClapMcpBuiltinFlags::default())
}

/// Like [`command_with_mcp_and_export_skills_flags`] with custom builtin long names.
pub fn command_with_mcp_and_export_skills_flags_with_flags(
    mut cmd: Command,
    flags: &ClapMcpBuiltinFlags,
) -> Command {
    cmd = command_with_mcp_flag_with_flags(cmd, flags);
    #[cfg(feature = "http")]
    {
        cmd = command_with_mcp_http_flag_with_flags(cmd, flags);
    }
    command_with_export_skills_flag_with_flags(cmd, flags)
}

#[cfg(feature = "http")]
pub fn command_with_mcp_http_flag(cmd: Command) -> Command {
    command_with_mcp_http_flag_with_flags(cmd, &ClapMcpBuiltinFlags::default())
}

#[cfg(feature = "http")]
pub fn command_with_mcp_http_flag_with_flags(
    mut cmd: Command,
    flags: &ClapMcpBuiltinFlags,
) -> Command {
    if command_has_arg_id_or_long(&cmd, CLAP_MCP_HTTP_FLAG_ID, flags.http_long) {
        return cmd;
    }

    cmd = cmd.arg(
        Arg::new(CLAP_MCP_HTTP_FLAG_ID)
            .long(flags.http_long)
            .value_name("ADDR")
            .help("Run an MCP server over Streamable HTTP at ADDR (e.g. 127.0.0.1:8080)")
            .global(true),
    );
    cmd
}

#[cfg(feature = "http")]
pub(crate) fn mcp_http_listen_from_env() -> Option<String> {
    if let Ok(listen) = std::env::var(MCP_HTTP_LISTEN_ENV)
        && !listen.is_empty()
    {
        return Some(listen);
    }
    match (
        std::env::var(MCP_HTTP_BIND_ENV).ok(),
        std::env::var(MCP_HTTP_PORT_ENV).ok(),
    ) {
        (Some(bind), Some(port)) if !bind.is_empty() && !port.is_empty() => {
            Some(format!("{bind}:{port}"))
        }
        _ => None,
    }
}

#[cfg(feature = "http")]
pub(crate) fn argv_mcp_http_listen_from_args(
    args: &[String],
    flags: &ClapMcpBuiltinFlags,
) -> Option<String> {
    let prefix = argv_before_end_of_opts(args);
    let http_flag = format!("--{}", flags.http_long);
    let http_prefix = format!("--{}=", flags.http_long);
    for (i, arg) in prefix.iter().enumerate() {
        if arg == &http_flag {
            if let Some(val) = prefix.get(i + 1).filter(|s| !s.starts_with('-')) {
                return Some(val.clone());
            }
            return mcp_http_listen_from_env();
        }
        if let Some(addr) = arg.strip_prefix(&http_prefix) {
            if addr.is_empty() {
                return mcp_http_listen_from_env();
            }
            return Some(addr.to_string());
        }
    }
    mcp_http_listen_from_env()
}

#[cfg(feature = "http")]
fn parse_mcp_http_listen(raw: &str) -> Result<std::net::SocketAddr, ClapMcpError> {
    raw.parse().map_err(|_| {
        ClapMcpError::InvalidConfig(format!(
            "invalid MCP HTTP listen address `{raw}` (expected host:port)"
        ))
    })
}

#[cfg(feature = "http")]
fn mcp_http_listen_error_message(flags: &ClapMcpBuiltinFlags) -> String {
    format!(
        "`--{}` requires HOST:PORT, or set {MCP_HTTP_LISTEN_ENV}, or {MCP_HTTP_BIND_ENV} + {MCP_HTTP_PORT_ENV}",
        flags.http_long
    )
}

#[cfg(feature = "http")]
fn resolve_mcp_http_listen_from_args(
    args: &[String],
    flags: &ClapMcpBuiltinFlags,
) -> Result<Option<std::net::SocketAddr>, ClapMcpError> {
    let prefix = argv_before_end_of_opts(args);
    let http_flag = format!("--{}", flags.http_long);
    let http_prefix = format!("--{}=", flags.http_long);
    let wants_http = prefix
        .iter()
        .any(|a| a == &http_flag || a.starts_with(&http_prefix));
    if !wants_http {
        return Ok(None);
    }
    match argv_mcp_http_listen_from_args(args, flags) {
        Some(raw) if !raw.is_empty() => parse_mcp_http_listen(&raw).map(Some),
        _ => Err(ClapMcpError::InvalidConfig(mcp_http_listen_error_message(
            flags,
        ))),
    }
}

/// Async MCP server for embedders: stdio or Streamable HTTP (`http` feature).
///
/// Runs on the **caller's tokio runtime**. Prefer [`ServeMcpBuilder`] from
/// `#[tokio::main]`; this function is the lower-level equivalent.
/// Use [`serve_mcp_blocking`] when `main` is synchronous.
///
/// When [`ClapMcpConfig::needs_multi_thread_runtime`] is true, the caller must use a
/// multi-thread runtime (e.g. `#[tokio::main(flavor = "multi_thread")]`).
///
/// # Example
///
/// ```rust,ignore
/// use clap_mcp::{ServeMcpBuilder, McpListen, ClapMcpServeOptions};
///
/// #[tokio::main(flavor = "multi_thread")]
/// async fn main() -> Result<(), clap_mcp::ClapMcpError> {
///     ServeMcpBuilder::for_cli::<Cli>(McpListen::Stdio)
///         .serve_options(ClapMcpServeOptions::default())
///         .serve()
///         .await
/// }
/// ```
pub async fn serve_mcp(
    listen: McpListen,
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> Result<(), ClapMcpError> {
    ServeMcpBuilder::new()
        .listen(listen)
        .schema_json(schema_json)
        .config(config)
        .metadata(metadata.clone())
        .serve_options(serve_options)
        .executable_path(executable_path)
        .in_process_handler(in_process_handler)
        .serve()
        .await
}

/// Blocking MCP server for embedders: stdio or Streamable HTTP (`http` feature).
///
/// Creates a tokio runtime internally. Prefer [`ServeMcpBuilder::serve_blocking`]
/// from sync `main`; this function is the lower-level equivalent.
/// For `#[tokio::main]`, prefer [`serve_mcp`] or [`ServeMcpBuilder::serve`].
///
/// # Example
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
pub fn serve_mcp_blocking(
    listen: McpListen,
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> Result<(), ClapMcpError> {
    ServeMcpBuilder::new()
        .listen(listen)
        .schema_json(schema_json)
        .config(config)
        .metadata(metadata.clone())
        .serve_options(serve_options)
        .executable_path(executable_path)
        .in_process_handler(in_process_handler)
        .serve_blocking()
}

#[cfg(feature = "http")]
fn serve_prepared_mcp_blocking(
    http_listen: Option<std::net::SocketAddr>,
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> Result<(), ClapMcpError> {
    let listen = match http_listen {
        Some(addr) => McpListen::Http(addr),
        None => McpListen::Stdio,
    };
    ServeMcpBuilder::new()
        .listen(listen)
        .schema_json(schema_json)
        .config(config)
        .metadata(metadata.clone())
        .serve_options(serve_options)
        .executable_path(executable_path)
        .in_process_handler(in_process_handler)
        .serve_blocking()
}

#[cfg(not(feature = "http"))]
fn serve_prepared_mcp_blocking(
    _http_listen: Option<std::net::SocketAddr>,
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> Result<(), ClapMcpError> {
    ServeMcpBuilder::new()
        .listen(McpListen::Stdio)
        .schema_json(schema_json)
        .config(config)
        .metadata(metadata.clone())
        .serve_options(serve_options)
        .executable_path(executable_path)
        .in_process_handler(in_process_handler)
        .serve_blocking()
}

#[cfg(feature = "http")]
pub(crate) fn argv_requests_mcp_http_without_subcommand_from_args(
    args: &[String],
    cmd: &Command,
    flags: &ClapMcpBuiltinFlags,
) -> bool {
    let prefix = argv_before_end_of_opts(args);
    let subcommand_names: std::collections::HashSet<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    let http_flag = format!("--{}", flags.http_long);
    let http_prefix = format!("--{}=", flags.http_long);
    let has_http = prefix
        .iter()
        .any(|a| a == &http_flag || a.starts_with(&http_prefix));
    let has_subcommand = prefix.iter().any(|a| subcommand_names.contains(a.as_str()));
    has_http && !has_subcommand
}

/// Returns true if argv contains the stdio MCP flag and no token before `--` is a subcommand name.
fn argv_requests_mcp_without_subcommand(cmd: &Command, flags: &ClapMcpBuiltinFlags) -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    argv_requests_mcp_without_subcommand_from_args(&args, cmd, flags)
}

/// Pure helper for argv_requests_mcp_without_subcommand; testable with arbitrary args.
pub(crate) fn argv_requests_mcp_without_subcommand_from_args(
    args: &[String],
    cmd: &Command,
    flags: &ClapMcpBuiltinFlags,
) -> bool {
    let prefix = argv_before_end_of_opts(args);
    let subcommand_names: std::collections::HashSet<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    let has_mcp = argv_has_long_flag(prefix, flags.stdio_long);
    let has_subcommand = prefix.iter().any(|a| subcommand_names.contains(a.as_str()));
    has_mcp && !has_subcommand
}

fn argv_export_skills_dir(flags: &ClapMcpBuiltinFlags) -> Option<Option<std::path::PathBuf>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    argv_export_skills_dir_from_args(&args, flags)
}

/// Pure helper for argv_export_skills_dir; testable with arbitrary args.
pub(crate) fn argv_export_skills_dir_from_args(
    args: &[String],
    flags: &ClapMcpBuiltinFlags,
) -> Option<Option<std::path::PathBuf>> {
    let prefix = argv_before_end_of_opts(args);
    let export_flag = format!("--{}", flags.export_skills_long);
    let export_prefix = format!("--{}=", flags.export_skills_long);
    for (i, arg) in prefix.iter().enumerate() {
        if arg == &export_flag {
            return Some(
                prefix
                    .get(i + 1)
                    .filter(|s| !s.starts_with('-'))
                    .map(std::path::PathBuf::from),
            );
        }
        if let Some(dir) = arg.strip_prefix(&export_prefix) {
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
        .filter(|a| !is_omitted_schema_clap_arg(a))
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
    let flags = config.builtin_flags;
    let cmd = command_with_mcp_and_export_skills_flags_with_flags(cmd, &flags);

    if let Some(maybe_dir) = argv_export_skills_dir(&flags) {
        let tools = tools_from_schema_with_metadata(&schema, &config, metadata);
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

    if config.allow_mcp_without_subcommand
        && (argv_requests_mcp_without_subcommand(&cmd, &flags) || {
            #[cfg(feature = "http")]
            {
                let args: Vec<String> = std::env::args().skip(1).collect();
                argv_requests_mcp_http_without_subcommand_from_args(&args, &cmd, &flags)
            }
            #[cfg(not(feature = "http"))]
            {
                false
            }
        })
    {
        let schema_json = match serde_json::to_string_pretty(&schema) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to serialize CLI schema: {}", e);
                std::process::exit(1);
            }
        };
        #[cfg(feature = "http")]
        let http_listen = {
            let args: Vec<String> = std::env::args().skip(1).collect();
            resolve_mcp_http_listen_from_args(&args, &flags).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(2);
            })
        };
        #[cfg(not(feature = "http"))]
        let http_listen: Option<std::net::SocketAddr> = None;

        if let Err(e) = serve_prepared_mcp_blocking(
            http_listen,
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
    let mcp_requested = matches_stdio_flag(&matches, &flags);
    #[cfg(feature = "http")]
    let http_listen = if matches_http_flag(&matches, &flags) {
        matches
            .get_one::<String>(CLAP_MCP_HTTP_FLAG_ID)
            .or_else(|| {
                if flags.http_long == MCP_HTTP_FLAG_LONG {
                    matches.get_one::<String>(MCP_HTTP_FLAG_LONG)
                } else {
                    None
                }
            })
            .map(|s| parse_mcp_http_listen(s))
            .transpose()
            .unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(2);
            })
    } else {
        None
    };
    #[cfg(not(feature = "http"))]
    let http_listen: Option<std::net::SocketAddr> = None;

    if mcp_requested && http_listen.is_some() {
        #[cfg(feature = "http")]
        eprintln!(
            "--{} and --{} are mutually exclusive",
            flags.stdio_long, flags.http_long
        );
        #[cfg(not(feature = "http"))]
        eprintln!("stdio and HTTP MCP flags are mutually exclusive");
        std::process::exit(2);
    }

    if mcp_requested || http_listen.is_some() {
        let schema_json = match serde_json::to_string_pretty(&schema) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to serialize CLI schema: {}", e);
                std::process::exit(1);
            }
        };
        if let Err(e) = serve_prepared_mcp_blocking(
            http_listen,
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
/// With the trait in scope, use `Args::parse_or_serve_mcp()`.
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
        parse_or_serve_mcp_with(ClapMcpRunOptions {
            config: T::clap_mcp_config(),
            serve: ClapMcpServeOptions::default(),
        })
    }
}

/// Run parsed CLI through a closure, or serve MCP if `--mcp` / `--mcp-http` is present.
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
    let args = A::parse_or_serve_mcp();
    f(args)
}

struct PreparedDeriveMcpServe {
    schema_json: String,
    in_process_handler: Option<InProcessToolHandler>,
    executable_path: Option<PathBuf>,
    metadata: ClapMcpSchemaMetadata,
}

fn capture_stdout_for_serve(serve_options: &ClapMcpServeOptions) -> bool {
    #[cfg(unix)]
    {
        serve_options.capture_stdout
    }
    #[cfg(not(unix))]
    {
        let _ = serve_options;
        false
    }
}

fn finish_prepared_derive_mcp_serve(
    config: &ClapMcpConfig,
    metadata: ClapMcpSchemaMetadata,
    schema_json: String,
    in_process_handler: Option<InProcessToolHandler>,
) -> PreparedDeriveMcpServe {
    let executable_path = if config.reinvocation_safe {
        None
    } else {
        std::env::current_exe().ok()
    };
    PreparedDeriveMcpServe {
        schema_json,
        in_process_handler,
        executable_path,
        metadata,
    }
}

/// Builds schema JSON, handler, and metadata for derive-based MCP serve (stateless tools).
pub(crate) fn prepare_derive_mcp_serve<T>(
    config: &ClapMcpConfig,
    serve_options: &ClapMcpServeOptions,
) -> PreparedDeriveMcpServe
where
    T: ClapMcpToolExecutor
        + ClapMcpSchemaMetadataProvider
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    let metadata = T::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&T::command(), &metadata);
    let schema_json = serde_json::to_string_pretty(&schema).expect("schema should serialize");
    let capture_stdout = capture_stdout_for_serve(serve_options);
    let in_process_handler = if config.reinvocation_safe {
        Some(make_in_process_handler::<T>(schema, capture_stdout))
    } else {
        None
    };
    finish_prepared_derive_mcp_serve(config, metadata, schema_json, in_process_handler)
}

/// Like [`prepare_derive_mcp_serve`], but captures shared session state in the in-process handler.
pub(crate) fn prepare_derive_mcp_serve_with_state<T>(
    config: &ClapMcpConfig,
    serve_options: &ClapMcpServeOptions,
    state: Arc<T::State>,
) -> PreparedDeriveMcpServe
where
    T: ClapMcpToolExecutorWithState
        + ClapMcpSchemaMetadataProvider
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    let metadata = T::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&T::command(), &metadata);
    let schema_json = serde_json::to_string_pretty(&schema).expect("schema should serialize");
    let capture_stdout = capture_stdout_for_serve(serve_options);
    let in_process_handler = if config.reinvocation_safe {
        Some(make_in_process_handler_with_state::<T>(
            schema,
            state,
            capture_stdout,
        ))
    } else {
        None
    };
    finish_prepared_derive_mcp_serve(config, metadata, schema_json, in_process_handler)
}

fn run_prepared_derive_mcp_serve(
    prepared: PreparedDeriveMcpServe,
    http_listen: Option<std::net::SocketAddr>,
    config: ClapMcpConfig,
    serve_options: ClapMcpServeOptions,
) -> Result<(), ClapMcpError> {
    serve_prepared_mcp_blocking(
        http_listen,
        prepared.schema_json,
        prepared.executable_path,
        config,
        prepared.in_process_handler,
        serve_options,
        &prepared.metadata,
    )
}

fn exit_on_mcp_serve_error(result: Result<(), ClapMcpError>) -> ! {
    if let Err(e) = result {
        eprintln!("MCP server error: {}", e);
        std::process::exit(1);
    }
    std::process::exit(0);
}

#[cfg(feature = "http")]
fn resolve_http_listen_from_env_or_exit(
    flags: &ClapMcpBuiltinFlags,
) -> Option<std::net::SocketAddr> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    resolve_mcp_http_listen_from_args(&args, flags).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    })
}

fn parse_or_serve_mcp_common<T>(
    options: ClapMcpRunOptions,
    prepare: impl FnOnce(&ClapMcpConfig, &ClapMcpServeOptions) -> PreparedDeriveMcpServe,
) -> T
where
    T: ClapMcpSchemaMetadataProvider + clap::Parser + clap::CommandFactory + clap::FromArgMatches,
{
    let ClapMcpRunOptions {
        config,
        serve: serve_options,
    } = options;
    let flags = config.builtin_flags;
    let mut cmd = T::command();
    cmd = command_with_mcp_and_export_skills_flags_with_flags(cmd, &flags);

    if let Some(maybe_dir) = argv_export_skills_dir(&flags) {
        let base_cmd = T::command();
        let metadata = T::clap_mcp_schema_metadata();
        let schema = schema_from_command_with_metadata(&base_cmd, &metadata);
        let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
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

    if config.allow_mcp_without_subcommand
        && (argv_requests_mcp_without_subcommand(&cmd, &flags) || {
            #[cfg(feature = "http")]
            {
                let args: Vec<String> = std::env::args().skip(1).collect();
                argv_requests_mcp_http_without_subcommand_from_args(&args, &cmd, &flags)
            }
            #[cfg(not(feature = "http"))]
            {
                false
            }
        })
    {
        #[cfg(feature = "http")]
        let http_listen = resolve_http_listen_from_env_or_exit(&flags);
        #[cfg(not(feature = "http"))]
        let http_listen: Option<std::net::SocketAddr> = None;
        let prepared = prepare(&config, &serve_options);
        exit_on_mcp_serve_error(run_prepared_derive_mcp_serve(
            prepared,
            http_listen,
            config,
            serve_options,
        ));
    }

    let matches = cmd.get_matches();
    let mcp_requested = matches_stdio_flag(&matches, &flags);
    #[cfg(feature = "http")]
    let http_listen = if matches_http_flag(&matches, &flags) {
        match matches
            .get_one::<String>(CLAP_MCP_HTTP_FLAG_ID)
            .or_else(|| {
                if flags.http_long == MCP_HTTP_FLAG_LONG {
                    matches.get_one::<String>(MCP_HTTP_FLAG_LONG)
                } else {
                    None
                }
            })
            .map(|s| parse_mcp_http_listen(s))
            .transpose()
        {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
    } else {
        None
    };
    #[cfg(not(feature = "http"))]
    let http_listen: Option<std::net::SocketAddr> = None;

    if mcp_requested && http_listen.is_some() {
        #[cfg(feature = "http")]
        eprintln!(
            "--{} and --{} are mutually exclusive",
            flags.stdio_long, flags.http_long
        );
        #[cfg(not(feature = "http"))]
        eprintln!("stdio and HTTP MCP flags are mutually exclusive");
        std::process::exit(2);
    }

    if mcp_requested || http_listen.is_some() {
        let prepared = prepare(&config, &serve_options);
        exit_on_mcp_serve_error(run_prepared_derive_mcp_serve(
            prepared,
            http_listen,
            config,
            serve_options,
        ));
    }

    T::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}

/// Derive-based entrypoint: parse CLI or start MCP server (stdio or HTTP) and exit.
///
/// Config comes from `T::clap_mcp_config()` (via `#[clap_mcp(...)]` on the derive).
/// Prefer [`ParseOrServeMcp::parse_or_serve_mcp`] when the trait is in scope.
pub fn parse_or_serve_mcp_with<T>(options: ClapMcpRunOptions) -> T
where
    T: ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutor
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    parse_or_serve_mcp_common::<T>(options, |config, serve_options| {
        prepare_derive_mcp_serve::<T>(config, serve_options)
    })
}

/// Stateful derive entrypoint: like [`parse_or_serve_mcp_with`] but captures `state` in the
/// in-process tool handler for the MCP server lifetime.
///
/// `state` is stored as [`Arc`] internally; your `run` function receives `&T::State` on each
/// tool call. Requires [`ClapMcpConfig::reinvocation_safe`](ClapMcpConfig::reinvocation_safe).
pub fn parse_or_serve_mcp_with_state<T>(options: ClapMcpRunOptions, state: Arc<T::State>) -> T
where
    T: ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutorWithState
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    parse_or_serve_mcp_common::<T>(options, |config, serve_options| {
        prepare_derive_mcp_serve_with_state::<T>(config, serve_options, Arc::clone(&state))
    })
}

/// Parse CLI or serve MCP with shared session state when `--mcp` is present.
///
/// Requires [`ClapMcpToolExecutorWithState`] on the derive target (see trait docs for setup).
pub trait ParseOrServeMcpWithState: ClapMcpToolExecutorWithState + Sized {
    /// Parse argv or start MCP with `state` captured for the server lifetime.
    fn parse_or_serve_mcp_with_state(state: Arc<Self::State>) -> Self;
}

impl<T> ParseOrServeMcpWithState for T
where
    T: ClapMcpConfigProvider
        + ClapMcpSchemaMetadataProvider
        + ClapMcpToolExecutorWithState
        + clap::Parser
        + clap::CommandFactory
        + clap::FromArgMatches
        + 'static,
{
    fn parse_or_serve_mcp_with_state(state: Arc<T::State>) -> Self {
        parse_or_serve_mcp_with_state::<T>(
            ClapMcpRunOptions {
                config: T::clap_mcp_config(),
                serve: ClapMcpServeOptions::default(),
            },
            state,
        )
    }
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
pub(crate) fn validate_required_args(
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

pub(crate) fn command_path(schema: &ClapSchema, command_name: &str) -> Option<Vec<String>> {
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
pub(crate) fn build_tool_argv(
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

    let mut positionals: Vec<&ClapArg> = args
        .iter()
        .filter(|a| a.long.is_none() && !a.num_args.as_deref().is_some_and(|n| n.contains("..")))
        .copied()
        .collect();
    positionals.sort_by_key(|a| a.index.unwrap_or(0));
    let trailing_positionals: Vec<&ClapArg> = args
        .iter()
        .filter(|a| a.long.is_none() && a.num_args.as_deref().is_some_and(|n| n.contains("..")))
        .copied()
        .collect();
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

    let mut trailing_values = Vec::new();
    for arg in &trailing_positionals {
        if let Some(v) = arguments.get(&arg.id)
            && let Some(strings) = value_to_strings(v)
        {
            trailing_values.extend(strings);
        }
    }
    if !trailing_values.is_empty() {
        out.push("--".to_string());
        out.extend(trailing_values);
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

fn parse_cli_from_tool_args<T>(
    schema: &ClapSchema,
    command_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
) -> Result<T, ClapMcpToolError>
where
    T: clap::CommandFactory + clap::FromArgMatches,
{
    validate_required_args(schema, command_name, &arguments).map_err(ClapMcpToolError::text)?;
    let argv = build_argv_for_clap(schema, command_name, arguments);
    let matches = T::command()
        .try_get_matches_from(&argv)
        .map_err(|e| ClapMcpToolError::text(e.to_string()))?;
    T::from_arg_matches(&matches).map_err(|e| ClapMcpToolError::text(e.to_string()))
}

fn execute_in_process_command<T>(
    schema: &ClapSchema,
    command_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
    capture_stdout: bool,
    execute: impl FnOnce(T) -> Result<ClapMcpToolOutput, ClapMcpToolError>,
) -> Result<ClapMcpToolOutput, ClapMcpToolError>
where
    T: clap::CommandFactory + clap::FromArgMatches,
{
    let cli = parse_cli_from_tool_args::<T>(schema, command_name, arguments)?;
    if capture_stdout {
        let (result, captured) = run_with_stdout_capture(|| execute(cli));
        merge_captured_stdout(result, captured)
    } else {
        execute(cli)
    }
}

fn execute_in_process_command_stateless<T>(
    schema: &ClapSchema,
    command_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
    capture_stdout: bool,
) -> Result<ClapMcpToolOutput, ClapMcpToolError>
where
    T: ClapMcpToolExecutor + clap::CommandFactory + clap::FromArgMatches,
{
    execute_in_process_command::<T>(schema, command_name, arguments, capture_stdout, |cli| {
        <T as ClapMcpToolExecutor>::execute_for_mcp(cli)
    })
}

fn execute_in_process_command_stateful<T>(
    schema: &ClapSchema,
    command_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
    state: &T::State,
    capture_stdout: bool,
) -> Result<ClapMcpToolOutput, ClapMcpToolError>
where
    T: ClapMcpToolExecutorWithState + clap::CommandFactory + clap::FromArgMatches,
{
    execute_in_process_command::<T>(schema, command_name, arguments, capture_stdout, |cli| {
        <T as ClapMcpToolExecutorWithState>::execute_for_mcp_with_state(cli, state)
    })
}

/// Builds an in-process tool handler for type `T` when using [`ServeMcpBuilder`],
/// [`serve_mcp`], or [`serve_mcp_blocking`] with `reinvocation_safe`.
/// [`ServeMcpBuilder::for_cli`] sets this automatically when appropriate.
pub fn in_process_tool_handler_for<T>(
    schema: ClapSchema,
    capture_stdout: bool,
) -> InProcessToolHandler
where
    T: ClapMcpToolExecutor + clap::CommandFactory + clap::FromArgMatches + 'static,
{
    make_in_process_handler::<T>(schema, capture_stdout)
}

pub(crate) fn make_in_process_handler<T>(
    schema: ClapSchema,
    capture_stdout: bool,
) -> InProcessToolHandler
where
    T: ClapMcpToolExecutor + clap::CommandFactory + clap::FromArgMatches + 'static,
{
    Arc::new(
        move |cmd: &str, args: serde_json::Map<String, serde_json::Value>| {
            execute_in_process_command_stateless::<T>(&schema, cmd, args, capture_stdout)
        },
    ) as InProcessToolHandler
}

pub(crate) fn make_in_process_handler_with_state<T>(
    schema: ClapSchema,
    state: Arc<T::State>,
    capture_stdout: bool,
) -> InProcessToolHandler
where
    T: ClapMcpToolExecutorWithState + clap::CommandFactory + clap::FromArgMatches + 'static,
{
    Arc::new(
        move |cmd: &str, args: serde_json::Map<String, serde_json::Value>| {
            execute_in_process_command_stateful::<T>(
                &schema,
                cmd,
                args,
                state.as_ref(),
                capture_stdout,
            )
        },
    ) as InProcessToolHandler
}

pub(crate) fn format_panic_payload(payload: &(dyn std::any::Any + Send)) -> String {
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
/// When `parallel_safe` is true and `share_runtime` is false, `run_async_tool` uses
/// `block_in_place` so the MCP server's multi-thread runtime can process overlapping calls
/// while dedicated-thread work runs.
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
        let catch_panics = config.catch_in_process_panics;
        let run_on_dedicated_thread = || {
            let task_id = crate::logging::current_mcp_task_id();
            std::thread::scope(|s| {
                let join_handle = s.spawn(move || {
                    let _task_id_guard = task_id.map(crate::logging::McpTaskIdGuard::new);
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    Ok(rt.block_on(f()))
                });
                match join_handle.join() {
                    Ok(inner) => inner,
                    Err(payload) if catch_panics => {
                        let msg = format_panic_payload(payload.as_ref());
                        Err(ClapMcpError::ToolThread(format!("Tool panicked: {msg}")))
                    }
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            })
        };
        if config.reinvocation_safe && config.parallel_safe {
            tokio::task::block_in_place(run_on_dedicated_thread)
        } else {
            run_on_dedicated_thread()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{
        ClapMcpServer, build_clap_mcp_server, build_execution_command,
        call_tool_result_from_output, call_tool_result_from_panic,
        call_tool_result_from_tool_error, command_launch_failure_result, get_prompt_result,
        list_prompts_result, list_resources_result, placeholder_tool_result, read_resource_result,
        schema_parse_failure_result, subprocess_stderr_log_params, validate_tool_argument_names,
    };
    use async_trait::async_trait;
    use clap::{Arg, ArgAction, Command, CommandFactory};
    use rmcp::ServerHandler;
    use rmcp::model::{
        Content, GetPromptRequestParams, PromptMessage, PromptMessageContent, PromptMessageRole,
        RawContent, ReadResourceRequestParams, ResourceContents, Tool,
    };
    use serde_json::json;
    use std::error::Error;
    use std::sync::{Arc, Mutex};

    fn content_text(content: &Content) -> &str {
        match &content.raw {
            RawContent::Text(text) => &text.text,
            _ => panic!("expected text content"),
        }
    }

    fn prompt_text(content: &PromptMessageContent) -> &str {
        match content {
            PromptMessageContent::Text { text, .. } => text,
            _ => panic!("expected text prompt content"),
        }
    }

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    #[cfg(unix)]
    use crate::server::call_tool_result_from_subprocess_output;

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
            &ClapMcpSchemaMetadata::default(),
            None,
        );

        assert_eq!(tool.name, "sample");
        assert_eq!(tool.description, None);

        let props = tool
            .input_schema
            .get("properties")
            .and_then(|value| value.as_object())
            .expect("tool should include input schema properties");
        let required = tool
            .input_schema
            .get("required")
            .and_then(|value| value.as_array())
            .expect("tool should include required keys");
        assert_eq!(
            required
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>(),
            vec!["input"]
        );
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

    fn passthrough_exec_command(allow_hyphen_values: bool) -> Command {
        let mut trailing = Arg::new("command").last(true).num_args(1..);
        if allow_hyphen_values {
            trailing = trailing.allow_hyphen_values(true);
        }
        Command::new("passthrough-args").subcommand(
            Command::new("exec")
                .arg(
                    Arg::new("dry_run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue),
                )
                .arg(trailing),
        )
    }

    #[test]
    fn test_build_tool_argv_trailing_vec_with_hyphen_tokens() {
        let schema = schema_from_command(&passthrough_exec_command(true));
        let arguments = serde_json::Map::from_iter([
            ("dry_run".to_string(), json!(false)),
            ("command".to_string(), json!(["-v", "--mcp", "hello"])),
        ]);
        let argv = build_tool_argv(&schema, "exec", arguments);
        assert_eq!(argv, vec!["--", "-v", "--mcp", "hello"]);
    }

    #[test]
    fn test_build_argv_round_trip_with_hyphen_trailing_vec() {
        let cmd = passthrough_exec_command(true);
        let schema = schema_from_command(&cmd);
        let arguments = serde_json::Map::from_iter([
            ("dry_run".to_string(), json!(true)),
            ("command".to_string(), json!(["-v", "hello"])),
        ]);
        let argv = build_argv_for_clap(&schema, "exec", arguments);
        let matches = cmd
            .try_get_matches_from(argv)
            .expect("trailing vec with -- separator should parse hyphen tokens");
        let sub = matches.subcommand().expect("exec subcommand");
        assert!(sub.1.get_flag("dry_run"));
        assert_eq!(
            sub.1
                .get_many::<String>("command")
                .into_iter()
                .flatten()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            vec!["-v", "hello"]
        );
    }

    #[test]
    fn test_build_argv_round_trip_with_trailing_vec() {
        let cmd = passthrough_exec_command(true);
        let schema = schema_from_command(&cmd);
        let arguments = serde_json::Map::from_iter([
            ("dry_run".to_string(), json!(true)),
            ("command".to_string(), json!(["echo", "hello"])),
        ]);
        let argv = build_argv_for_clap(&schema, "exec", arguments);
        assert_eq!(
            argv,
            vec![
                "cli".to_string(),
                "exec".to_string(),
                "--dry-run".to_string(),
                "--".to_string(),
                "echo".to_string(),
                "hello".to_string(),
            ]
        );
        let matches = cmd
            .try_get_matches_from(argv)
            .expect("trailing vec after flags should parse");
        let sub = matches.subcommand().expect("exec subcommand");
        assert_eq!(sub.0, "exec");
        assert!(sub.1.get_flag("dry_run"));
        assert_eq!(
            sub.1
                .get_many::<String>("command")
                .into_iter()
                .flatten()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            vec!["echo", "hello"]
        );
    }

    #[test]
    fn test_build_argv_hyphen_trailing_fails_without_end_of_opts() {
        let cmd = passthrough_exec_command(false);
        let err = cmd
            .try_get_matches_from(["cli", "exec", "-v", "hello"])
            .expect_err("trailing hyphen tokens without -- should fail clap parse");
        assert!(
            err.to_string().contains("unexpected argument")
                || err.to_string().contains("unknown argument")
                || err.to_string().contains("found argument"),
            "unexpected error: {err}"
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

        let cmd = command_with_mcp_and_export_skills_flags(Command::new("bare"));
        assert_eq!(
            cmd.get_arguments()
                .filter(|arg| arg.get_long() == Some(MCP_FLAG_LONG))
                .count(),
            1
        );
        assert_eq!(
            cmd.get_arguments()
                .filter(|arg| arg.get_long() == Some(EXPORT_SKILLS_FLAG_LONG))
                .count(),
            1
        );
    }

    #[test]
    fn test_argv_export_skills_dir_from_args() {
        let flags = ClapMcpBuiltinFlags::default();
        assert!(argv_export_skills_dir_from_args(&[], &flags).is_none());
        assert!(argv_export_skills_dir_from_args(&["--other".to_string()], &flags).is_none());
        assert_eq!(
            argv_export_skills_dir_from_args(&["--export-skills".to_string()], &flags),
            Some(None)
        );
        assert_eq!(
            argv_export_skills_dir_from_args(
                &["--export-skills".to_string(), "out".to_string()],
                &flags
            ),
            Some(Some(std::path::PathBuf::from("out")))
        );
        assert_eq!(
            argv_export_skills_dir_from_args(
                &["--export-skills".to_string(), "--mcp".to_string()],
                &flags
            ),
            Some(None)
        );
        assert_eq!(
            argv_export_skills_dir_from_args(&["--export-skills=out".to_string()], &flags),
            Some(Some(std::path::PathBuf::from("out")))
        );
        assert!(
            argv_export_skills_dir_from_args(
                &[
                    "run".to_string(),
                    "--".to_string(),
                    "--export-skills".to_string()
                ],
                &flags
            )
            .is_none(),
            "export-skills after -- must not trigger"
        );
    }

    #[test]
    fn test_argv_before_end_of_opts() {
        let args = vec!["run".to_string(), "--".to_string(), "--mcp".to_string()];
        assert_eq!(
            argv_before_end_of_opts(&args),
            &["run".to_string()] as &[String]
        );
    }

    #[test]
    fn test_argv_requests_mcp_without_subcommand_from_args() {
        let cmd = Command::new("app").subcommand(Command::new("run"));
        let flags = ClapMcpBuiltinFlags::default();
        assert!(argv_requests_mcp_without_subcommand_from_args(
            &["--mcp".to_string()],
            &cmd,
            &flags
        ));
        assert!(!argv_requests_mcp_without_subcommand_from_args(
            &["--mcp".to_string(), "run".to_string()],
            &cmd,
            &flags
        ));
        assert!(!argv_requests_mcp_without_subcommand_from_args(
            &["run".to_string()],
            &cmd,
            &flags
        ));
        assert!(!argv_requests_mcp_without_subcommand_from_args(
            &[],
            &cmd,
            &flags
        ));
        assert!(!argv_requests_mcp_without_subcommand_from_args(
            &["run".to_string(), "--".to_string(), "--mcp".to_string()],
            &cmd,
            &flags
        ));
        assert!(!argv_requests_mcp_without_subcommand_from_args(
            &["run".to_string(), "--mcp".to_string()],
            &cmd,
            &flags
        ));
        assert!(argv_requests_mcp_without_subcommand_from_args(
            &["--mcp".to_string(), "--".to_string(), "--mcp".to_string()],
            &cmd,
            &flags
        ));
    }

    #[test]
    fn test_argv_requests_mcp_custom_stdio_long() {
        let cmd = Command::new("app");
        let flags = ClapMcpBuiltinFlags::default().with_stdio_long("modelcontextprotocol");
        assert!(argv_requests_mcp_without_subcommand_from_args(
            &["--modelcontextprotocol".to_string()],
            &cmd,
            &flags
        ));
        assert!(!argv_requests_mcp_without_subcommand_from_args(
            &["--mcp".to_string()],
            &cmd,
            &flags
        ));
        assert!(!argv_requests_mcp_without_subcommand_from_args(
            &[
                "run".to_string(),
                "--".to_string(),
                "--modelcontextprotocol".to_string(),
            ],
            &cmd,
            &flags
        ));
    }

    #[test]
    fn test_is_builtin_arg() {
        assert!(is_builtin_arg("help"));
        assert!(is_builtin_arg("version"));
        assert!(is_builtin_arg(CLAP_MCP_STDIO_FLAG_ID));
        assert!(!is_builtin_arg(CLAP_MCP_STDIO_FLAG_ID_LEGACY));
        assert!(is_builtin_arg(EXPORT_SKILLS_FLAG_LONG));
        assert!(!is_builtin_arg("input"));
        assert!(!is_builtin_arg("path"));
        assert!(!is_builtin_arg("mcp"), "user mcp field must not be builtin");
    }

    #[test]
    fn test_tools_from_schema_with_metadata() {
        let schema = sample_helper_schema();
        let tools = tools_from_schema_with_metadata(
            &schema,
            &ClapMcpConfig::default(),
            &ClapMcpSchemaMetadata::default(),
        );
        assert!(!tools.is_empty());
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_mcp_http_listen_from_env_and_flag_alone() {
        let listen_key = MCP_HTTP_LISTEN_ENV;
        let bind_key = MCP_HTTP_BIND_ENV;
        let port_key = MCP_HTTP_PORT_ENV;

        let flags = ClapMcpBuiltinFlags::default();
        unsafe {
            std::env::set_var(listen_key, "127.0.0.1:9090");
        }
        assert_eq!(
            argv_mcp_http_listen_from_args(&["--mcp-http".to_string()], &flags),
            Some("127.0.0.1:9090".to_string())
        );
        unsafe {
            std::env::remove_var(listen_key);
        }

        unsafe {
            std::env::set_var(bind_key, "127.0.0.1");
            std::env::set_var(port_key, "9091");
        }
        assert_eq!(
            argv_mcp_http_listen_from_args(&["--mcp-http".to_string()], &flags),
            Some("127.0.0.1:9091".to_string())
        );
        unsafe {
            std::env::remove_var(bind_key);
            std::env::remove_var(port_key);
        }
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_http_flag_helpers_cover_command_and_argv_shapes() {
        use clap::Command;

        let cmd = command_with_mcp_http_flag(Command::new("app"));
        assert!(
            cmd.get_arguments()
                .any(|a| a.get_long() == Some(MCP_HTTP_FLAG_LONG))
        );

        let flags = ClapMcpBuiltinFlags::default();
        assert_eq!(
            argv_mcp_http_listen_from_args(
                &["--mcp-http".to_string(), "127.0.0.1:4242".to_string()],
                &flags
            ),
            Some("127.0.0.1:4242".to_string())
        );
        assert_eq!(
            argv_mcp_http_listen_from_args(&["--mcp-http=10.0.0.1:9".to_string()], &flags),
            Some("10.0.0.1:9".to_string())
        );

        let mut cmd = Command::new("app");
        cmd = cmd.arg(
            clap::Arg::new(CLAP_MCP_HTTP_FLAG_ID)
                .long(MCP_HTTP_FLAG_LONG)
                .global(true),
        );
        let unchanged = command_with_mcp_http_flag_with_flags(cmd, &flags);
        assert_eq!(unchanged.get_arguments().count(), 1);

        let matches = Command::new("app")
            .arg(
                clap::Arg::new(CLAP_MCP_HTTP_FLAG_ID)
                    .long(MCP_HTTP_FLAG_LONG)
                    .value_name("ADDR")
                    .global(true),
            )
            .get_matches_from(["app", "--mcp-http", "127.0.0.1:1"]);
        assert!(matches_http_flag(&matches, &flags));
        assert!(is_builtin_arg(CLAP_MCP_HTTP_FLAG_ID));
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_parse_mcp_http_listen_reports_invalid_addresses() {
        let err = parse_mcp_http_listen("not-an-address").expect_err("invalid listen");
        assert!(
            matches!(err, ClapMcpError::InvalidConfig(message) if message.contains("invalid MCP HTTP listen"))
        );
        assert!(
            mcp_http_listen_error_message(&ClapMcpBuiltinFlags::default())
                .contains(MCP_HTTP_LISTEN_ENV)
        );
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_resolve_mcp_http_listen_from_args_covers_success_and_errors() {
        let flags = ClapMcpBuiltinFlags::default();
        assert_eq!(
            resolve_mcp_http_listen_from_args(
                &["--mcp-http".to_string(), "127.0.0.1:4242".to_string()],
                &flags
            )
            .expect("listen should parse")
            .map(|addr| addr.to_string()),
            Some("127.0.0.1:4242".to_string())
        );
        assert!(
            resolve_mcp_http_listen_from_args(&["--help".to_string()], &flags)
                .expect("no http flag")
                .is_none()
        );
        let missing = resolve_mcp_http_listen_from_args(&["--mcp-http".to_string()], &flags)
            .expect_err("missing host:port should error");
        assert!(
            matches!(missing, ClapMcpError::InvalidConfig(message) if message.contains(MCP_HTTP_LISTEN_ENV))
        );
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_argv_requests_mcp_http_without_subcommand_from_args() {
        let cmd = Command::new("app").subcommand(Command::new("run"));
        let flags = ClapMcpBuiltinFlags::default();
        assert!(argv_requests_mcp_http_without_subcommand_from_args(
            &["--mcp-http".to_string(), "127.0.0.1:1".to_string()],
            &cmd,
            &flags
        ));
        assert!(!argv_requests_mcp_http_without_subcommand_from_args(
            &[
                "run".to_string(),
                "--mcp-http".to_string(),
                "127.0.0.1:1".to_string()
            ],
            &cmd,
            &flags
        ));
    }

    #[tokio::test]
    async fn test_serve_mcp_fails_fast_on_invalid_schema_json() {
        let err = serve_mcp(
            McpListen::Stdio,
            "not-json".to_string(),
            None,
            ClapMcpConfig::default(),
            None,
            ClapMcpServeOptions::default(),
            &ClapMcpSchemaMetadata::default(),
        )
        .await
        .expect_err("invalid schema should fail");
        assert!(matches!(err, ClapMcpError::SchemaJson(_)));
    }

    #[test]
    fn test_ambiguous_positional_scalars_build_swapped_argv() {
        use clap::{FromArgMatches, Parser, Subcommand};

        #[derive(Debug, Subcommand)]
        enum Cmd {
            Edit { task_id: String, state: String },
        }

        #[derive(Debug, Parser)]
        #[command(subcommand_required = true)]
        struct App {
            #[command(subcommand)]
            cmd: Cmd,
        }

        let schema = schema_from_command(&App::command());
        let args = serde_json::Map::from_iter([
            ("task_id".to_string(), json!("done")),
            ("state".to_string(), json!("TASK-0")),
        ]);
        let argv = build_argv_for_clap(&schema, "edit", args);
        assert_eq!(argv, vec!["cli", "edit", "TASK-0", "done"]);

        let matches = App::command().get_matches_from(argv);
        let parsed = App::from_arg_matches(&matches).expect("app should parse");
        match parsed.cmd {
            Cmd::Edit { task_id, state } => {
                assert_eq!(task_id, "TASK-0");
                assert_eq!(state, "done");
            }
        }
    }

    #[test]
    fn test_command_path_and_build_argv_for_clap() {
        let schema = nested_schema();
        assert_eq!(command_path(&schema, "sample"), Some(vec!["sample".into()]));
        assert_eq!(
            command_path(&schema, "child"),
            Some(vec!["sample".into(), "parent".into(), "child".into()])
        );
        assert_eq!(command_path(&schema, "nonexistent"), None);

        let args = serde_json::Map::from_iter([("value".to_string(), json!("v"))]);
        let argv = build_argv_for_clap(&schema, "child", args);
        assert_eq!(argv[0], "cli");
        assert_eq!(argv[1], "parent");
        assert_eq!(argv[2], "child");
        assert!(argv.contains(&"--value".to_string()));
        assert!(argv.contains(&"v".to_string()));

        let empty_argv = build_tool_argv(&schema, "nonexistent", serde_json::Map::new());
        assert!(empty_argv.is_empty());
    }

    #[cfg(not(feature = "output-schema"))]
    #[test]
    fn test_output_schema_for_type_without_schemars() {
        assert!(output_schema_for_type::<()>().is_none());
    }

    #[cfg(feature = "output-schema")]
    #[test]
    fn test_output_schema_for_type_with_schemars() {
        use schemars::JsonSchema;
        #[derive(JsonSchema)]
        struct Dummy {
            _x: i32,
        }
        let schema = output_schema_for_type::<Dummy>();
        assert!(schema.is_some());
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
            ReadResourceRequestParams::new(MCP_RESOURCE_URI_SCHEMA),
        )
        .await
        .expect("schema resource should resolve");
        let text = match &schema_read.contents[0] {
            ResourceContents::TextResourceContents { text, .. } => text,
            other => panic!("unexpected content: {other:?}"),
        };
        assert!(text.contains("\"name\":\"sample\""));

        let custom_read = read_resource_result(
            "{}",
            &custom,
            ReadResourceRequestParams::new("test://dynamic"),
        )
        .await
        .expect("custom resource should resolve");
        let text = match &custom_read.contents[0] {
            ResourceContents::TextResourceContents { text, .. } => text,
            other => panic!("unexpected content: {other:?}"),
        };
        assert_eq!(text, "dynamic body");

        let missing = read_resource_result(
            "{}",
            &custom,
            ReadResourceRequestParams::new("test://missing"),
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
            ReadResourceRequestParams::new("test://broken"),
        )
        .await
        .expect_err("provider failure should map to rpc error");
        assert_eq!(failing.message, "read failed");
    }

    #[tokio::test]
    async fn test_prompt_helpers_cover_logging_custom_and_error_paths() {
        let provider = Arc::new(TestPromptProvider {
            response: Ok(vec![PromptMessage::new_text(
                PromptMessageRole::User,
                "dynamic prompt",
            )]),
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
            GetPromptRequestParams::new(PROMPT_LOGGING_GUIDE),
        )
        .await
        .expect("logging guide should resolve");
        assert!(prompt_text(&logging_prompt.messages[0].content).contains("logger"));

        let mut topic_args = serde_json::Map::new();
        topic_args.insert("topic".into(), json!("coverage"));
        let dynamic_prompt = get_prompt_result(
            false,
            &prompts,
            GetPromptRequestParams::new("dynamic").with_arguments(topic_args),
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
            GetPromptRequestParams::new(PROMPT_LOGGING_GUIDE),
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
            GetPromptRequestParams::new("broken"),
        )
        .await
        .expect_err("provider failure should map to rpc error");
        assert_eq!(failing.message, "prompt failed");
    }

    #[test]
    fn test_call_tool_result_helpers_cover_text_structured_errors_and_panics() {
        let text = call_tool_result_from_output(ClapMcpToolOutput::Text("hello".to_string()));
        assert_ne!(text.is_error, Some(true));
        assert_eq!(content_text(&text.content[0]), "hello");

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
        assert!(content_text(&structured.content[0]).contains("\"sum\": 5"));

        let array_value = call_tool_result_from_output(ClapMcpToolOutput::Structured(json!(["a"])));
        assert_eq!(array_value.structured_content.as_ref(), Some(&json!(["a"])));

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
        assert!(content_text(&panic_result.content[0]).contains("Tool panicked: boom"));
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
            assert_ne!(success.is_error, Some(true));
            assert!(content_text(&success.content[0]).contains("stderr:\nnote"));

            let failure_output = std::process::Output {
                status: std::process::ExitStatus::from_raw(256),
                stdout: Vec::new(),
                stderr: b"boom\n".to_vec(),
            };
            let failure = call_tool_result_from_subprocess_output(&failure_output);
            assert_eq!(failure.is_error, Some(true));
            assert!(content_text(&failure.content[0]).contains("non-zero status"));
        }

        let launch_error = command_launch_failure_result(&std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        ));
        assert_eq!(launch_error.is_error, Some(true));
        assert!(content_text(&launch_error.content[0]).contains("Failed to run command"));

        let placeholder = placeholder_tool_result(
            "echo",
            &serde_json::Map::from_iter([("message".to_string(), json!("hi"))]),
        );
        assert!(content_text(&placeholder.content[0]).contains("Would invoke clap command 'echo'"));

        let parse_failure = schema_parse_failure_result();
        assert_eq!(parse_failure.is_error, Some(true));
        assert_eq!(
            content_text(&parse_failure.content[0]),
            "Failed to parse schema"
        );
    }

    #[test]
    fn test_validate_tool_argument_names_rejects_unknown_keys() {
        let tool = command_to_tool_with_config(
            &sample_helper_schema().root,
            &ClapMcpConfig::default(),
            &ClapMcpSchemaMetadata::default(),
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

        let structured = execute_in_process_command_stateless::<ExecCli>(
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

        let missing = execute_in_process_command_stateless::<ExecCli>(
            &schema,
            "echo",
            serde_json::Map::new(),
            false,
        )
        .expect_err("missing required arg should fail");
        assert!(
            missing
                .message
                .contains("Missing required argument(s): value")
        );
    }

    fn build_test_server(
        config: ClapMcpConfig,
        metadata: ClapMcpSchemaMetadata,
        serve_options: ClapMcpServeOptions,
        handler: Option<InProcessToolHandler>,
        executable_path: Option<std::path::PathBuf>,
    ) -> ClapMcpServer {
        let schema = nested_schema();
        let schema_json = serde_json::to_string(&schema).expect("schema json");
        let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
        build_clap_mcp_server(
            schema_json,
            tools,
            executable_path,
            handler,
            schema.root.name.clone(),
            &config,
            &serve_options,
            &metadata,
        )
        .expect("server should build")
    }

    #[test]
    fn test_build_clap_mcp_server_rejects_task_augmented_without_reinvocation_safe() {
        let config = ClapMcpConfig {
            reinvocation_safe: false,
            ..Default::default()
        };
        let metadata = ClapMcpSchemaMetadata {
            task_augmented_tools: true,
            ..Default::default()
        };
        let schema = nested_schema();
        let schema_json = serde_json::to_string(&schema).expect("schema json");
        let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
        assert!(matches!(
            build_clap_mcp_server(
                schema_json,
                tools,
                None,
                Some(Arc::new(|_, _| Ok(ClapMcpToolOutput::Text("ok".into())))),
                schema.root.name,
                &config,
                &ClapMcpServeOptions::default(),
                &metadata,
            ),
            Err(ClapMcpError::InvalidConfig(_))
        ));
    }

    #[test]
    fn test_build_clap_mcp_server_rejects_task_augmented_without_in_process_handler() {
        let config = ClapMcpConfig {
            reinvocation_safe: true,
            ..Default::default()
        };
        let metadata = ClapMcpSchemaMetadata {
            task_augmented_tools: true,
            ..Default::default()
        };
        let schema = nested_schema();
        let schema_json = serde_json::to_string(&schema).expect("schema json");
        let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
        assert!(matches!(
            build_clap_mcp_server(
                schema_json,
                tools,
                None,
                None,
                schema.root.name,
                &config,
                &ClapMcpServeOptions::default(),
                &metadata,
            ),
            Err(ClapMcpError::InvalidConfig(_))
        ));
    }

    #[test]
    fn test_build_clap_mcp_server_accepts_parallel_and_serial_configs() {
        let handler: InProcessToolHandler =
            Arc::new(|_, _| Ok(ClapMcpToolOutput::Text("ok".into())));
        for parallel_safe in [true, false] {
            let config = ClapMcpConfig {
                reinvocation_safe: true,
                parallel_safe,
                ..Default::default()
            };
            build_test_server(
                config,
                ClapMcpSchemaMetadata::default(),
                ClapMcpServeOptions::default(),
                Some(handler.clone()),
                None,
            );
        }
    }

    #[test]
    fn test_get_info_capability_matrix_and_instructions() {
        let handler: InProcessToolHandler =
            Arc::new(|_, _| Ok(ClapMcpToolOutput::Text("ok".into())));
        let config = ClapMcpConfig {
            reinvocation_safe: true,
            parallel_safe: true,
            ..Default::default()
        };

        let (tx, rx) = logging::log_channel(4);
        drop(tx);
        let with_logging = build_test_server(
            config.clone(),
            ClapMcpSchemaMetadata::default(),
            ClapMcpServeOptions {
                log_rx: Some(rx),
                ..Default::default()
            },
            Some(handler.clone()),
            None,
        );
        let info = with_logging.get_info();
        assert!(info.capabilities.logging.is_some());
        assert!(info.capabilities.tasks.is_none());
        assert_eq!(
            info.instructions.as_deref(),
            Some(LOG_INTERPRETATION_INSTRUCTIONS)
        );

        let with_tasks = build_test_server(
            config.clone(),
            ClapMcpSchemaMetadata {
                task_augmented_tools: true,
                ..Default::default()
            },
            ClapMcpServeOptions::default(),
            Some(handler.clone()),
            None,
        );
        let info = with_tasks.get_info();
        assert!(info.capabilities.logging.is_none());
        assert!(info.capabilities.tasks.is_some());
        assert!(info.instructions.is_none());

        let with_both = build_test_server(
            config,
            ClapMcpSchemaMetadata {
                task_augmented_tools: true,
                ..Default::default()
            },
            ClapMcpServeOptions {
                log_rx: Some(logging::log_channel(4).1),
                ..Default::default()
            },
            Some(handler),
            None,
        );
        let info = with_both.get_info();
        assert!(info.capabilities.logging.is_some());
        assert!(info.capabilities.tasks.is_some());
        assert_eq!(
            info.instructions.as_deref(),
            Some(LOG_INTERPRETATION_INSTRUCTIONS)
        );
    }

    #[test]
    fn test_build_execution_command_root_tool_skips_extra_segment() {
        let schema =
            schema_from_command(&Command::new("sample").arg(Arg::new("value").long("value")));
        let args = serde_json::Map::from_iter([("value".to_string(), json!("ok"))]);
        let command = build_execution_command(
            std::path::Path::new("/tmp/example"),
            &schema,
            "sample",
            "sample",
            &args,
        );
        let actual_args: Vec<_> = command.get_args().collect();
        assert_eq!(
            actual_args,
            vec![std::ffi::OsStr::new("--value"), std::ffi::OsStr::new("ok"),]
        );
    }

    #[test]
    fn test_validate_tool_argument_names_allows_extra_when_no_properties() {
        let tool = Tool::new(
            "freeform",
            "no schema properties",
            Arc::new(serde_json::Map::new()),
        );
        let args = serde_json::Map::from_iter([("anything".to_string(), json!(1))]);
        assert!(validate_tool_argument_names(&tool, "freeform", &args).is_ok());
    }
}
