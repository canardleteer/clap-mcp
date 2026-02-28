//! # clap-mcp
//!
//! Expose your [clap](https://docs.rs/clap) CLI as an MCP (Model Context Protocol) server over stdio.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use clap::Parser;
//!
//! #[derive(Parser, clap_mcp::ClapMcp)]
//! #[clap_mcp(reinvocation_safe, parallel_safe = false)]
//! enum Cli {
//!     #[clap_mcp_output = "format!(\"Hello, {}!\", name.as_deref().unwrap_or(\"world\"))"]
//!     Greet { #[arg(long)] name: Option<String> },
//! }
//!
//! fn main() {
//!     let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
//!     match cli {
//!         Cli::Greet { name } => println!("Hello, {}!", name.as_deref().unwrap_or("world")),
//!     }
//! }
//! ```
//!
//! Run with `--mcp` to start the MCP server instead of executing the CLI.

use async_trait::async_trait;
use clap::{Arg, ArgAction, Command};
use rust_mcp_sdk::{
    mcp_server::{server_runtime, McpServerOptions, ServerHandler, ToMcpServerHandler},
    schema::{
        schema_utils, CallToolError, CallToolRequestParams, CallToolResult, ContentBlock,
        GetPromptRequestParams, GetPromptResult, Implementation, InitializeResult,
        ListPromptsResult, ListResourcesResult, ListToolsResult, LoggingLevel,
        LoggingMessageNotificationParams, PaginatedRequestParams, Prompt, PromptMessage,
        ReadResourceContent, ReadResourceRequestParams, ReadResourceResult,
        Resource, Role, RpcError, ServerCapabilities, ServerCapabilitiesPrompts,
        ServerCapabilitiesResources, ServerCapabilitiesTools, TextResourceContents,
        Tool, ToolInputSchema, LATEST_PROTOCOL_VERSION,
    },
    McpServer, StdioTransport, TransportOptions,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

/// Derive macro for `ClapMcpConfigProvider` and `ClapMcpToolExecutor`.
///
/// Use with `#[derive(ClapMcp)]` on your clap enum. Supports attributes:
/// `#[clap_mcp(...)]`, `#[clap_mcp_output = "..."]`, `#[clap_mcp_output_type = "TypeName"]`.
pub use clap_mcp_macros::ClapMcp;

#[cfg(any(feature = "tracing", feature = "log"))]
pub mod logging;

/// Long flag that triggers MCP server mode. Add to your CLI via [`command_with_mcp_flag`].
pub const MCP_FLAG_LONG: &str = "mcp";

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
///
/// #[derive(Debug, Parser, clap_mcp::ClapMcp)]
/// #[clap_mcp(parallel_safe = false, reinvocation_safe)]
/// enum MyCli { Foo }
///
/// let config = MyCli::clap_mcp_config();
/// assert!(config.reinvocation_safe);
/// assert!(!config.parallel_safe);
/// ```
pub trait ClapMcpConfigProvider {
    fn clap_mcp_config() -> ClapMcpConfig;
}

/// Produces the output string for a parsed CLI value.
/// Used for in-process MCP tool execution when `reinvocation_safe` is true.
/// Implemented by the `#[derive(ClapMcp)]` macro via the blanket impl for `ClapMcpToolExecutor`.
pub trait ClapMcpRunnable {
    fn run(self) -> String;
}

/// Output produced by a CLI command for MCP tool results.
///
/// Use `Text` for plain string output; use `Structured` for serializable JSON
/// (when using `#[clap_mcp_output_type = "TypeName"]`).
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
/// Use `#[clap_mcp_output = "expr"]` for text output, or `#[clap_mcp_output_type = "TypeName"]`
/// with `#[clap_mcp_output = "expr"]` for structured JSON output.
pub trait ClapMcpToolExecutor {
    fn execute_for_mcp(self) -> ClapMcpToolOutput;
}

impl<T: ClapMcpToolExecutor> ClapMcpRunnable for T {
    fn run(self) -> String {
        self.execute_for_mcp().into_string()
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
}

/// Configuration for execution safety when exposing a CLI over MCP.
///
/// Use this to declare whether your CLI tool can be safely invoked multiple times,
/// whether it can run in parallel with other tool calls, and how async tools run.
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
#[derive(Debug, Clone, Default)]
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
    matches!(id, "help" | "version" | MCP_FLAG_LONG)
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
    schema
        .root
        .all_commands()
        .into_iter()
        .map(|cmd| command_to_tool_with_config(cmd, config))
        .collect()
}

fn command_to_tool_with_config(cmd: &ClapCommand, config: &ClapMcpConfig) -> Tool {
    let args: Vec<&ClapArg> = cmd
        .args
        .iter()
        .filter(|a| !is_builtin_arg(a.id.as_str()))
        .collect();

    let mut properties: HashMap<String, serde_json::Map<String, serde_json::Value>> =
        HashMap::new();
    for arg in &args {
        let mut prop = serde_json::Map::new();
        prop.insert(
            "type".to_string(),
            serde_json::Value::String("string".to_string()),
        );
        let desc = arg
            .long_help
            .as_deref()
            .or(arg.help.as_deref())
            .map(String::from);
        if let Some(d) = desc {
            prop.insert("description".to_string(), serde_json::Value::String(d));
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
        output_schema: None,
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
    ClapSchema {
        root: command_to_schema(cmd),
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
    let schema = schema_from_command(&cmd);
    let cmd = command_with_mcp_flag(cmd);

    let matches = cmd.get_matches();
    if matches.get_flag(MCP_FLAG_LONG) {
        let schema_json =
            serde_json::to_string_pretty(&schema).expect("schema JSON must serialize");
        serve_schema_json_over_stdio_blocking(
            schema_json,
            None,
            config,
            None,
            ClapMcpServeOptions::default(),
        )
        .expect("MCP server must start");
        std::process::exit(0);
    }

    matches
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
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
///
/// #[derive(Parser, clap_mcp::ClapMcp)]
/// enum Cli { Foo }
///
/// fn main() {
///     let cli = clap_mcp::parse_or_serve_mcp::<Cli>();
///     // If we get here, --mcp was not passed
/// }
/// ```
pub fn parse_or_serve_mcp<T>() -> T
where
    T: ClapMcpToolExecutor + clap::Parser + clap::CommandFactory + clap::FromArgMatches,
{
    parse_or_serve_mcp_with_config::<T>(ClapMcpConfig::default())
}

/// High-level helper for `clap` derive-based CLIs with config from `#[clap_mcp(...)]` attributes.
///
/// Use `#[derive(ClapMcp)]` and `#[clap_mcp(parallel_safe = false, reinvocation_safe)]` on your CLI type,
/// then call this instead of [`parse_or_serve_mcp`]. Config is taken from `T::clap_mcp_config()`.
///
/// # Example
///
/// ```rust,ignore
/// use clap::Parser;
///
/// #[derive(Parser, clap_mcp::ClapMcp)]
/// #[clap_mcp(reinvocation_safe, parallel_safe = false)]
/// enum Cli {
///     #[clap_mcp_output = "format!(\"done\")"]
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
    T: ClapMcpConfigProvider + ClapMcpToolExecutor + clap::Parser + clap::CommandFactory + clap::FromArgMatches,
{
    parse_or_serve_mcp_with_config::<T>(T::clap_mcp_config())
}

/// High-level helper for `clap` derive-based CLIs with execution safety configuration.
///
/// See [`parse_or_serve_mcp`] for behavior. Use `config` to declare reinvocation
/// and parallel execution safety. When `reinvocation_safe` is true, uses in-process
/// execution; requires `T: ClapMcpToolExecutor`.
pub fn parse_or_serve_mcp_with_config<T>(config: ClapMcpConfig) -> T
where
    T: ClapMcpToolExecutor + clap::Parser + clap::CommandFactory + clap::FromArgMatches,
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
    T: ClapMcpToolExecutor + clap::Parser + clap::CommandFactory + clap::FromArgMatches,
{
    let mut cmd = T::command();
    cmd = command_with_mcp_flag(cmd);

    let matches = cmd.get_matches();
    let mcp_requested = matches.get_flag(MCP_FLAG_LONG);

    if mcp_requested {
        let base_cmd = T::command();
        let schema = schema_from_command(&base_cmd);
        let schema_json =
            serde_json::to_string_pretty(&schema).expect("schema JSON must serialize");
        let exe = std::env::current_exe().ok();

        let in_process_handler = if config.reinvocation_safe {
            let schema = schema.clone();
            Some(Arc::new(move |cmd: &str, args: serde_json::Map<String, serde_json::Value>| {
                let argv = build_argv_for_clap(&schema, cmd, args);
                let matches = T::command().get_matches_from(&argv);
                let cli = T::from_arg_matches(&matches).map_err(|e| e.to_string())?;
                Ok(<T as ClapMcpToolExecutor>::execute_for_mcp(cli))
            }) as InProcessToolHandler)
        } else {
            None
        };

        serve_schema_json_over_stdio_blocking(
            schema_json,
            if config.reinvocation_safe { None } else { exe },
            config,
            in_process_handler,
            serve_options,
        )
        .expect("MCP server must start");

        std::process::exit(0);
    }

    T::from_arg_matches(&matches)
        .unwrap_or_else(|e| e.exit())
}

fn command_to_schema(cmd: &Command) -> ClapCommand {
    let mut args: Vec<ClapArg> = cmd
        .get_arguments()
        .filter(|a| a.get_long() != Some(MCP_FLAG_LONG))
        .map(arg_to_schema)
        .collect();

    // Stable ordering for consumers
    args.sort_by(|a, b| a.id.cmp(&b.id));

    let subcommands: Vec<ClapCommand> = cmd
        .get_subcommands()
        .map(command_to_schema)
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

/// Builds full argv for clap's `get_matches_from` (program name + subcommand + args).
fn build_argv_for_clap(
    schema: &ClapSchema,
    command_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let root_name = schema.root.name.clone();
    let args = build_tool_argv(schema, command_name, arguments);
    let mut argv = vec!["cli".to_string()]; // program name for parsing
    if command_name != root_name {
        argv.push(command_name.to_string());
    }
    argv.extend(args);
    argv
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

    let mut positionals: Vec<&ClapArg> = args.iter().filter(|a| a.long.is_none()).copied().collect();
    positionals.sort_by_key(|a| a.index.unwrap_or(0));
    let optionals: Vec<&ClapArg> = args.iter().filter(|a| a.long.is_some()).copied().collect();

    let mut out = Vec::new();

    for arg in positionals {
        if let Some(v) = arguments.get(&arg.id)
            && let Some(s) = value_to_string(v)
        {
            out.push(s);
        }
    }
    for arg in optionals {
        if let Some(long) = &arg.long
            && let Some(v) = arguments.get(&arg.id)
            && let Some(s) = value_to_string(v)
        {
            out.push(format!("--{long}"));
            out.push(s);
        }
    }

    out
}

/// Type for in-process tool execution handler.
///
/// Called with `(command_name, arguments)` and returns `Result<ClapMcpToolOutput, String>`.
/// Used when `reinvocation_safe` is true to avoid spawning subprocesses.
pub type InProcessToolHandler = Arc<
    dyn Fn(&str, serde_json::Map<String, serde_json::Value>) -> Result<ClapMcpToolOutput, String>
        + Send
        + Sync,
>;

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
/// # Example
///
/// ```rust,ignore
/// let schema_json = serde_json::to_string(&schema)?;
/// clap_mcp::serve_schema_json_over_stdio(
///     schema_json,
///     Some(std::env::current_exe()?),
///     clap_mcp::ClapMcpConfig::default(),
///     None,
///     clap_mcp::ClapMcpServeOptions::default(),
/// ).await?;
/// ```
pub async fn serve_schema_json_over_stdio(
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: ClapMcpServeOptions,
) -> std::result::Result<(), ClapMcpError> {
    let schema: ClapSchema = serde_json::from_str(&schema_json)?;
    let tools = tools_from_schema_with_config(&schema, &config);
    let root_name = schema.root.name.clone();

    let tool_execution_lock: Option<Arc<tokio::sync::Mutex<()>>> =
        if config.parallel_safe {
            None
        } else {
            Some(Arc::new(tokio::sync::Mutex::new(())))
        };

    let logging_enabled = serve_options.log_rx.is_some();
    let (runtime_tx, runtime_rx) = if logging_enabled {
        let (tx, rx) = tokio::sync::oneshot::channel::<Arc<dyn rust_mcp_sdk::McpServer>>();
        (Some(std::sync::Arc::new(std::sync::Mutex::new(Some(tx)))), Some(rx))
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
        Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Arc<dyn rust_mcp_sdk::McpServer>>>>>,
    >;

    struct Handler {
        schema_json: String,
        tools: Vec<Tool>,
        executable_path: Option<PathBuf>,
        in_process_handler: Option<InProcessToolHandler>,
        root_name: String,
        tool_execution_lock: Option<Arc<tokio::sync::Mutex<()>>>,
        runtime_tx: RuntimeTx,
    }

    #[async_trait]
    impl ServerHandler for Handler {
        async fn handle_list_resources_request(
            &self,
            _params: Option<PaginatedRequestParams>,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<ListResourcesResult, RpcError> {
            Ok(ListResourcesResult {
                resources: vec![Resource {
                    name: "clap-schema".into(),
                    uri: MCP_RESOURCE_URI_SCHEMA.into(),
                    title: Some("Clap CLI schema".into()),
                    description: Some("JSON schema extracted from clap Command definitions".into()),
                    mime_type: Some("application/json".into()),
                    annotations: None,
                    icons: vec![],
                    meta: None,
                    size: None,
                }],
                meta: None,
                next_cursor: None,
            })
        }

        async fn handle_read_resource_request(
            &self,
            params: ReadResourceRequestParams,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<ReadResourceResult, RpcError> {
            if params.uri != MCP_RESOURCE_URI_SCHEMA {
                return Err(RpcError::invalid_params()
                    .with_message(format!("unknown resource uri: {}", params.uri)));
            }

            Ok(ReadResourceResult {
                contents: vec![ReadResourceContent::TextResourceContents(TextResourceContents {
                    uri: params.uri,
                    mime_type: Some("application/json".into()),
                    text: self.schema_json.clone(),
                    meta: None,
                })],
                meta: None,
            })
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
            Ok(ListPromptsResult {
                prompts: vec![Prompt {
                    name: PROMPT_LOGGING_GUIDE.to_string(),
                    description: Some("How to interpret log messages from this clap-mcp server".to_string()),
                    arguments: vec![],
                    icons: vec![],
                    meta: None,
                    title: Some("clap-mcp Logging Guide".to_string()),
                }],
                meta: None,
                next_cursor: None,
            })
        }

        async fn handle_get_prompt_request(
            &self,
            params: GetPromptRequestParams,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<GetPromptResult, RpcError> {
            if params.name != PROMPT_LOGGING_GUIDE {
                return Err(RpcError::invalid_params()
                    .with_message(format!("unknown prompt: {}", params.name)));
            }
            Ok(GetPromptResult {
                description: Some("How to interpret log messages from this clap-mcp server".to_string()),
                messages: vec![PromptMessage {
                    content: ContentBlock::text_content(LOGGING_GUIDE_CONTENT.to_string()),
                    role: Role::User,
                }],
                meta: None,
            })
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
            if let Some(ref props) = tool.input_schema.properties {
                for key in args_map.keys() {
                    if !props.contains_key(key) {
                        return Err(CallToolError::invalid_arguments(
                            &params.name,
                            Some(format!("unknown argument: {key}")),
                        ));
                    }
                }
            }

            let _guard = if let Some(ref lock) = self.tool_execution_lock {
                Some(lock.lock().await)
            } else {
                None
            };

            if let Some(ref handler) = self.in_process_handler {
                match handler(&params.name, args_map) {
                    Ok(output) => {
                        let (content, structured_content) = match &output {
                            ClapMcpToolOutput::Text(s) => (
                                vec![ContentBlock::text_content(s.clone())],
                                None,
                            ),
                            ClapMcpToolOutput::Structured(v) => {
                                let json_text =
                                    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
                                let structured = v.as_object().cloned();
                                (
                                    vec![ContentBlock::text_content(json_text)],
                                    structured,
                                )
                            }
                        };
                        return Ok(CallToolResult {
                            content,
                            is_error: None,
                            meta: None,
                            structured_content,
                        });
                    }
                    Err(e) => {
                        return Ok(CallToolResult {
                            content: vec![ContentBlock::text_content(e)],
                            is_error: Some(true),
                            meta: None,
                            structured_content: None,
                        });
                    }
                }
            }

            if let Some(ref exe) = self.executable_path {
                let schema: ClapSchema = match serde_json::from_str(&self.schema_json) {
                    Ok(s) => s,
                    Err(_) => {
                        return Ok(CallToolResult {
                            content: vec![ContentBlock::text_content(
                                "Failed to parse schema".into(),
                            )],
                            is_error: Some(true),
                            meta: None,
                            structured_content: None,
                        });
                    }
                };
                let args = build_tool_argv(&schema, &params.name, args_map);
                let mut cmd = std::process::Command::new(exe);
                if params.name != self.root_name {
                    cmd.arg(params.name.as_str());
                }
                for arg in &args {
                    cmd.arg(arg);
                }
                match cmd.output() {
                    Ok(output) => {
                        let out = String::from_utf8_lossy(&output.stdout);
                        let err = String::from_utf8_lossy(&output.stderr);
                        if !err.is_empty() {
                            // When changing stderr logging behavior, update LOG_INTERPRETATION_INSTRUCTIONS and LOGGING_GUIDE_CONTENT.
                            let mut meta = serde_json::Map::new();
                            meta.insert("tool".to_string(), serde_json::Value::String(params.name.clone()));
                            let _ = runtime
                                .notify_log_message(LoggingMessageNotificationParams {
                                    data: serde_json::Value::String(err.trim().to_string()),
                                    level: LoggingLevel::Info,
                                    logger: Some("stderr".to_string()),
                                    meta: Some(meta),
                                })
                                .await;
                        }
                        let text = if err.is_empty() {
                            out.trim().to_string()
                        } else {
                            format!("{}\nstderr:\n{}", out.trim(), err.trim())
                        };
                        return Ok(CallToolResult::from_content(vec![
                            ContentBlock::text_content(text),
                        ]));
                    }
                    Err(e) => {
                        return Ok(CallToolResult {
                            content: vec![ContentBlock::text_content(format!(
                                "Failed to run command: {}",
                                e
                            ))],
                            is_error: Some(true),
                            meta: None,
                            structured_content: None,
                        });
                    }
                }
            }

            let name = params.name;
            let args_json = serde_json::Value::Object(args_map);
            let text = format!(
                "Would invoke clap command '{name}' with arguments: {args_json:?}"
            );
            Ok(CallToolResult::from_content(vec![
                ContentBlock::text_content(text),
            ]))
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
) -> std::result::Result<(), ClapMcpError> {
    let use_multi_thread = config.reinvocation_safe && config.share_runtime;
    let rt = if use_multi_thread {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime must build")
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime must build")
    };
    rt.block_on(serve_schema_json_over_stdio(
        schema_json,
        executable_path,
        config,
        in_process_handler,
        serve_options,
    ))
}

/// Runs an async future for MCP tool execution, respecting `share_runtime` in config.
///
/// Use this in `#[clap_mcp_output]` when your tool does async work (e.g. `tokio::sleep`,
/// `tokio::spawn`). The closure must return a `Future` that produces the tool output.
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
/// # Example
///
/// ```rust,ignore
/// #[derive(Parser, clap_mcp::ClapMcp)]
/// #[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime = false)]
/// enum Cli {
///     #[clap_mcp_output_type = "SleepResult"]
///     #[clap_mcp_output = "clap_mcp::run_async_tool(&Cli::clap_mcp_config(), || run_sleep_demo())"]
///     SleepDemo,
/// }
/// ```
///
/// # Panics
///
/// When `share_runtime` is true and `reinvocation_safe` is true, panics if not
/// running within a tokio runtime (e.g. `Handle::try_current()` fails).
pub fn run_async_tool<Fut, O>(config: &ClapMcpConfig, f: impl FnOnce() -> Fut + Send) -> O
where
    Fut: std::future::Future<Output = O> + Send,
    O: Send,
{
    if config.reinvocation_safe && config.share_runtime {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::try_current()
                .expect("share_runtime=true requires running within tokio runtime (use reinvocation_safe + share_runtime)")
                .block_on(f())
        })
    } else {
        std::thread::scope(|s| {
            s.spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime must build")
                    .block_on(f())
            })
            .join()
            .expect("async tool thread must not panic")
        })
    }
}

