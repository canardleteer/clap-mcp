use async_trait::async_trait;
use clap::{Arg, ArgAction, Command};
use rust_mcp_sdk::{
    mcp_server::{server_runtime, McpServerOptions, ServerHandler, ToMcpServerHandler},
    schema::{
        schema_utils, CallToolError, CallToolRequestParams, CallToolResult, ContentBlock,
        Implementation, InitializeResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
        ReadResourceContent, ReadResourceRequestParams, ReadResourceResult, Resource, RpcError,
        ServerCapabilities, ServerCapabilitiesResources, ServerCapabilitiesTools, TextResourceContents,
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

pub const MCP_FLAG_LONG: &str = "mcp";
pub const MCP_RESOURCE_URI_SCHEMA: &str = "clap://schema";

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
/// Use this to declare whether your CLI tool can be safely invoked multiple times
/// and whether it can run in parallel with other tool calls.
#[derive(Debug, Clone)]
pub struct ClapMcpConfig {
    /// If true, the CLI can be invoked multiple times without tearing down the process.
    /// When false (default), each tool call spawns a fresh subprocess.
    /// When true, reserves future in-process execution; for now still spawns but annotates tools.
    pub reinvocation_safe: bool,

    /// If true, tool calls may run concurrently. When false, calls are serialized.
    /// Default is true for backward compatibility (preserves current behavior).
    pub parallel_safe: bool,
}

impl Default for ClapMcpConfig {
    fn default() -> Self {
        Self {
            reinvocation_safe: false,
            parallel_safe: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClapSchema {
    pub root: ClapCommand,
}

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

/// Builds MCP tools from a clap schema: one tool per command (root + every subcommand).
/// Tool names match command names; descriptions use the same text as `--help`;
/// each tool's input schema lists the command's arguments (excluding help/version/mcp).
pub fn tools_from_schema(schema: &ClapSchema) -> Vec<Tool> {
    tools_from_schema_with_config(schema, &ClapMcpConfig::default())
}

/// Builds MCP tools from a clap schema with execution safety annotations.
///
/// Tools include `meta.clapMcp` with `reinvocationSafe` and `parallelSafe` hints.
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
/// If an arg with `--mcp` already exists, this is a no-op.
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
/// Note: this function intentionally ignores any `--mcp` flag added via `command_with_mcp_flag`,
/// so the schema reflects the CLI as defined by the application.
pub fn schema_from_command(cmd: &Command) -> ClapSchema {
    ClapSchema {
        root: command_to_schema(cmd),
    }
}

/// Imperative clap entrypoint.
///
/// - Ensures `--mcp` appears in `--help`
/// - If `--mcp` is present, starts an MCP stdio server and exits the process
/// - Otherwise, returns `ArgMatches` for normal app execution
pub fn get_matches_or_serve_mcp(cmd: Command) -> clap::ArgMatches {
    get_matches_or_serve_mcp_with_config(cmd, ClapMcpConfig::default())
}

/// Imperative clap entrypoint with execution safety configuration.
///
/// See [`get_matches_or_serve_mcp`] for behavior. Use `config` to declare
/// reinvocation and parallel execution safety.
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
        serve_schema_json_over_stdio_blocking(schema_json, None, config)
            .expect("MCP server must start");
        std::process::exit(0);
    }

    matches
}

/// High-level helper for `clap` derive-based CLIs.
///
/// - Ensures `--mcp` appears in `--help`
/// - If `--mcp` is present, starts an MCP stdio server and exits the process
/// - Otherwise, returns the parsed CLI type
pub fn parse_or_serve_mcp<T>() -> T
where
    T: clap::Parser + clap::CommandFactory + clap::FromArgMatches,
{
    parse_or_serve_mcp_with_config::<T>(ClapMcpConfig::default())
}

/// High-level helper for `clap` derive-based CLIs with execution safety configuration.
///
/// See [`parse_or_serve_mcp`] for behavior. Use `config` to declare reinvocation
/// and parallel execution safety.
pub fn parse_or_serve_mcp_with_config<T>(config: ClapMcpConfig) -> T
where
    T: clap::Parser + clap::CommandFactory + clap::FromArgMatches,
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
        serve_schema_json_over_stdio_blocking(schema_json, exe, config)
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

/// Builds argv for the executable from the schema and tool arguments.
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
/// If `executable_path` is `Some`, tool calls run that executable with the tool name
/// (as subcommand) and the given arguments (e.g. `exe add --a 2 --b 3`), and return its
/// stdout as the tool result. If `None`, tool calls return a placeholder message.
///
/// Use `config` to declare reinvocation and parallel execution safety. When
/// `parallel_safe` is false, tool calls are serialized.
pub async fn serve_schema_json_over_stdio(
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
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

    struct Handler {
        schema_json: String,
        tools: Vec<Tool>,
        executable_path: Option<PathBuf>,
        root_name: String,
        tool_execution_lock: Option<Arc<tokio::sync::Mutex<()>>>,
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

        async fn handle_call_tool_request(
            &self,
            params: CallToolRequestParams,
            _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
        ) -> std::result::Result<CallToolResult, CallToolError> {
            let known = self.tools.iter().any(|t| t.name == params.name);
            if !known {
                return Err(CallToolError::unknown_tool(params.name.clone()));
            }

            let _guard = if let Some(ref lock) = self.tool_execution_lock {
                Some(lock.lock().await)
            } else {
                None
            };

            if let Some(ref exe) = self.executable_path {
                let args_map = params.arguments.unwrap_or_default();
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
            let args_json = serde_json::Value::Object(params.arguments.unwrap_or_default());
            let text = format!(
                "Would invoke clap command '{name}' with arguments: {args_json:?}"
            );
            Ok(CallToolResult::from_content(vec![
                ContentBlock::text_content(text),
            ]))
        }
    }

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
            ..Default::default()
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        instructions: None,
        meta: None,
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
        root_name,
        tool_execution_lock,
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

/// Convenience wrapper for `serve_schema_json_over_stdio` that does not require an async main.
pub fn serve_schema_json_over_stdio_blocking(
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
) -> std::result::Result<(), ClapMcpError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime must build");
    rt.block_on(serve_schema_json_over_stdio(schema_json, executable_path, config))
}

