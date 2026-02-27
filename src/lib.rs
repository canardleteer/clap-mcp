use async_trait::async_trait;
use clap::{Arg, ArgAction, Command};
use rust_mcp_sdk::{
    mcp_server::{server_runtime, McpServerOptions, ServerHandler, ToMcpServerHandler},
    schema::{
        schema_utils, CallToolError, CallToolRequestParams, CallToolResult, ContentBlock,
        Implementation, InitializeResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
        ReadResourceContent, ReadResourceRequestParams, ReadResourceResult, Resource, RpcError,
        ServerCapabilities, ServerCapabilitiesResources, ServerCapabilitiesTools, TextResourceContents,
        Tool, LATEST_PROTOCOL_VERSION,
    },
    McpServer, StdioTransport, TransportOptions,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

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
    let schema = schema_from_command(&cmd);
    let cmd = command_with_mcp_flag(cmd);

    let matches = cmd.get_matches();
    if matches.get_flag(MCP_FLAG_LONG) {
        let schema_json =
            serde_json::to_string_pretty(&schema).expect("schema JSON must serialize");
        serve_schema_json_over_stdio_blocking(schema_json).expect("MCP server must start");
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
    let mut cmd = T::command();
    cmd = command_with_mcp_flag(cmd);

    let matches = cmd.get_matches();
    let mcp_requested = matches.get_flag(MCP_FLAG_LONG);

    if mcp_requested {
        let base_cmd = T::command();
        let schema = schema_from_command(&base_cmd);
        let schema_json =
            serde_json::to_string_pretty(&schema).expect("schema JSON must serialize");
        serve_schema_json_over_stdio_blocking(schema_json).expect("MCP server must start");

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

/// Starts an MCP server over stdio exposing `clap://schema` with the provided JSON payload.
pub async fn serve_schema_json_over_stdio(
    schema_json: String,
) -> std::result::Result<(), ClapMcpError> {
    #[derive(Default)]
    struct Handler {
        schema_json: String,
        tools: Vec<Tool>,
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
            // Execution pipeline is out of scope for now; echo back the intended clap subcommand.
            let name = params.name;
            let args_json = serde_json::Value::Object(params.arguments.unwrap_or_default());
            let text = format!(
                "Would invoke clap subcommand '{name}' with arguments: {args_json:?}"
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

    // TODO: accept a precomputed Vec<Tool> once we expose a tool-extraction API
    let tools = Vec::new();

    let handler = Handler { schema_json, tools }.to_mcp_server_handler();
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
) -> std::result::Result<(), ClapMcpError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime must build");
    rt.block_on(serve_schema_json_over_stdio(schema_json))
}

