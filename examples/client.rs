//! MCP client example that tests clap-mcp servers.
//!
//! Subcommands launch different example servers:
//! - `subcommands` (default): Basic subcommands example with text and structured output
//! - `structured`: Structured output only
//! - `result-output`: Result<T, E> in #[clap_mcp_output], error responses
//! - `tracing-bridge`: With tracing integration (requires --features tracing)
//! - `log-bridge`: With log crate forwarding (requires --features log)
//! - `async-sleep`: Async tokio CLI with 3 sleep tasks, dedicated thread (requires --features tracing)
//! - `async-sleep-shared`: Same but shares the MCP server's runtime (requires --features tracing)
//! - `async-embedder-serve`: Imperative `ServeMcpBuilder` embedder path (requires --features tracing)

use clap::{Parser, Subcommand};
use rmcp::{
    ClientHandler, RoleClient, ServiceExt,
    model::{
        CallToolRequestParams, CancelledNotificationParam, LoggingMessageNotificationParam,
        ProgressNotificationParam, ResourceUpdatedNotificationParam,
    },
    service::NotificationContext,
    transport::{ConfigureCommandExt, TokioChildProcess},
};

#[derive(Clone)]
struct ExampleClientHandler {
    json: bool,
}

impl ClientHandler for ExampleClientHandler {
    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        if self.json {
            println!("{}", serde_json::to_string(&params).unwrap_or_default());
        } else {
            let logger = params.logger.as_deref().unwrap_or("unknown");
            let level = format!("{:?}", params.level).to_uppercase();
            println!("  [LOG {level} ({logger})] {}", params.data);
        }
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
    }

    async fn on_cancelled(
        &self,
        params: CancelledNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
    }

    async fn on_resource_list_changed(&self, _context: NotificationContext<RoleClient>) {
        if self.json {
            eprintln!("{{}}");
        }
    }

    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
    }

    async fn on_prompt_list_changed(&self, _context: NotificationContext<RoleClient>) {
        if self.json {
            eprintln!("{{}}");
        }
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        if self.json {
            eprintln!("{{}}");
        }
    }
}

#[derive(Parser)]
#[command(
    name = "client",
    about = "MCP client that tests clap-mcp example servers"
)]
struct Args {
    /// Print incoming notification JSON to stderr as it arrives
    #[arg(long, short)]
    json: bool,

    #[command(subcommand)]
    command: Cli,
}

#[derive(Subcommand)]
enum Cli {
    /// Test the subcommands example (default)
    Subcommands,
    /// Test the struct_subcommand example (struct root with optional subcommand)
    StructSubcommand,
    /// Test the optional_commands_and_args example (skip, requires)
    OptionalCommandsAndArgs,
    /// Test the result_output example (Result<T, E>, error responses)
    ResultOutput,
    /// Test the structured output example
    Structured,
    /// Test the tracing_bridge example (requires --features tracing)
    #[cfg(feature = "tracing")]
    TracingBridge,
    /// Test the async_sleep example (requires --features tracing)
    #[cfg(feature = "tracing")]
    AsyncSleep,
    /// Test the async_sleep_shared example (requires --features tracing)
    #[cfg(feature = "tracing")]
    AsyncSleepShared,
    /// Test the async_embedder_serve example (requires --features tracing)
    #[cfg(feature = "tracing")]
    AsyncEmbedderServe,
    /// Test the log_bridge example (requires --features log)
    #[cfg(feature = "log")]
    LogBridge,
}

fn server_args(example: &str) -> Vec<String> {
    let feature = match example {
        "tracing_bridge" | "async_sleep" | "async_sleep_shared" | "async_embedder_serve" => {
            Some("tracing")
        }
        "log_bridge" => Some("log"),
        _ => None,
    };
    let mut args = vec![
        "run".into(),
        "-p".into(),
        "clap-mcp-examples".into(),
        "--bin".into(),
        example.into(),
    ];
    if let Some(f) = feature {
        args.push("--features".into());
        args.push(f.into());
    }
    args.push("--".into());
    args.push("--mcp".into());
    args
}

fn tool_text(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| match &block.raw {
            rmcp::model::RawContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn run_client(example: &str, json: bool) -> Result<(), rmcp::RmcpError> {
    let transport =
        TokioChildProcess::new(tokio::process::Command::new("cargo").configure(|cmd| {
            for arg in server_args(example) {
                cmd.arg(arg);
            }
        }))
        .map_err(rmcp::RmcpError::transport_creation::<TokioChildProcess>)?;

    let client = ExampleClientHandler { json }.serve(transport).await?;

    let resources = client.list_resources(None).await?.resources;
    println!("Resources:");
    for res in &resources {
        println!("- {} ({})", res.name, res.uri);
    }

    let tools_result = client.list_tools(None).await?;
    println!("\nTools:");
    for t in &tools_result.tools {
        println!("  {}: {}", t.name, t.description.as_deref().unwrap_or(""));
    }

    if example == "subcommands" || example == "struct_subcommand" {
        run_subcommands_tests(&client).await?;
    } else if example == "optional_commands_and_args" {
        run_optional_commands_tests(&client).await?;
    } else if example == "structured" {
        run_structured_tests(&client).await?;
    } else if example == "async_sleep"
        || example == "async_sleep_shared"
        || example == "async_embedder_serve"
    {
        run_async_sleep_tests(&client).await?;
    } else if example == "tracing_bridge" || example == "log_bridge" {
        run_logging_tests(&client).await?;
    }

    let _ = client.cancel().await;
    Ok(())
}

async fn run_subcommands_tests(
    client: &rmcp::service::RunningService<RoleClient, ExampleClientHandler>,
) -> Result<(), rmcp::ServiceError> {
    let mut greet_args = serde_json::Map::new();
    greet_args.insert("name".into(), serde_json::json!("Rust"));
    let greet_result = client
        .call_tool(CallToolRequestParams::new("greet").with_arguments(greet_args))
        .await?;
    println!("\nCall 'greet' with name=\"Rust\":");
    println!("  {}", tool_text(&greet_result));

    let mut add_args = serde_json::Map::new();
    add_args.insert("a".into(), serde_json::json!(2));
    add_args.insert("b".into(), serde_json::json!(3));
    let add_result = client
        .call_tool(CallToolRequestParams::new("add").with_arguments(add_args))
        .await?;
    println!("\nCall 'add' with a=2, b=3:");
    println!("  {}", tool_text(&add_result));

    let mut sub_args = serde_json::Map::new();
    sub_args.insert("a".into(), serde_json::json!(10));
    sub_args.insert("b".into(), serde_json::json!(5));
    let sub_result = client
        .call_tool(CallToolRequestParams::new("sub").with_arguments(sub_args))
        .await?;
    println!("\nCall 'sub' with a=10, b=5 (structured output):");
    println!("  {}", tool_text(&sub_result));
    if let Some(ref structured) = sub_result.structured_content {
        println!(
            "  structured_content: {}",
            serde_json::to_string_pretty(structured).unwrap()
        );
    }

    Ok(())
}

async fn run_optional_commands_tests(
    client: &rmcp::service::RunningService<RoleClient, ExampleClientHandler>,
) -> Result<(), rmcp::ServiceError> {
    let result = client
        .call_tool(CallToolRequestParams::new("public"))
        .await?;
    println!("\nCall 'public':");
    println!("  {}", tool_text(&result));

    let mut read_args = serde_json::Map::new();
    read_args.insert("path".into(), serde_json::json!("/tmp/example"));
    let result = client
        .call_tool(CallToolRequestParams::new("read").with_arguments(read_args))
        .await?;
    println!("\nCall 'read' with path=\"/tmp/example\":");
    println!("  {}", tool_text(&result));

    let mut process_args = serde_json::Map::new();
    process_args.insert("path".into(), serde_json::json!("/data"));
    process_args.insert("input".into(), serde_json::json!("hello"));
    let result = client
        .call_tool(CallToolRequestParams::new("process").with_arguments(process_args))
        .await?;
    println!("\nCall 'process' with path and input:");
    println!("  {}", tool_text(&result));

    let result = client
        .call_tool(CallToolRequestParams::new("read").with_arguments(serde_json::Map::new()))
        .await?;
    println!("\nCall 'read' without path (expect error):");
    println!("  {}", tool_text(&result));
    assert!(
        result.is_error == Some(true),
        "read without required path should return error"
    );

    Ok(())
}

async fn run_async_sleep_tests(
    client: &rmcp::service::RunningService<RoleClient, ExampleClientHandler>,
) -> Result<(), rmcp::ServiceError> {
    let result = client
        .call_tool(CallToolRequestParams::new("sleep-demo"))
        .await?;
    println!("\nCall 'sleep-demo':");
    println!("  {}", tool_text(&result));
    if let Some(ref structured) = result.structured_content {
        println!(
            "  structured_content: {}",
            serde_json::to_string_pretty(structured).unwrap()
        );
    }
    Ok(())
}

async fn run_structured_tests(
    client: &rmcp::service::RunningService<RoleClient, ExampleClientHandler>,
) -> Result<(), rmcp::ServiceError> {
    let mut args = serde_json::Map::new();
    args.insert("a".into(), serde_json::json!(7));
    args.insert("b".into(), serde_json::json!(3));
    let result = client
        .call_tool(CallToolRequestParams::new("add").with_arguments(args))
        .await?;
    println!("\nCall 'add' with a=7, b=3:");
    println!("  {}", tool_text(&result));
    if let Some(ref structured) = result.structured_content {
        println!(
            "  structured_content: {}",
            serde_json::to_string_pretty(structured).unwrap()
        );
    }
    Ok(())
}

async fn run_logging_tests(
    client: &rmcp::service::RunningService<RoleClient, ExampleClientHandler>,
) -> Result<(), rmcp::ServiceError> {
    let prompts = client.list_prompts(None).await?.prompts;
    println!("\nPrompts:");
    for p in &prompts {
        println!("  {}: {}", p.name, p.description.as_deref().unwrap_or(""));
    }

    let mut args = serde_json::Map::new();
    args.insert("s".into(), serde_json::json!("hello"));
    let result = client
        .call_tool(CallToolRequestParams::new("echo").with_arguments(args))
        .await?;
    println!("\nCall 'echo' with s=\"hello\":");
    println!("  {}", tool_text(&result));
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let example = match args.command {
        Cli::Subcommands => "subcommands",
        Cli::StructSubcommand => "struct_subcommand",
        Cli::OptionalCommandsAndArgs => "optional_commands_and_args",
        Cli::ResultOutput => "result_output",
        Cli::Structured => "structured",
        #[cfg(feature = "tracing")]
        Cli::TracingBridge => "tracing_bridge",
        #[cfg(feature = "tracing")]
        Cli::AsyncSleep => "async_sleep",
        #[cfg(feature = "tracing")]
        Cli::AsyncSleepShared => "async_sleep_shared",
        #[cfg(feature = "tracing")]
        Cli::AsyncEmbedderServe => "async_embedder_serve",
        #[cfg(feature = "log")]
        Cli::LogBridge => "log_bridge",
    };
    if let Err(e) = run_client(example, args.json).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
