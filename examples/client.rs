//! MCP client example that tests clap-mcp servers.
//!
//! Subcommands launch different example servers:
//! - `derive` (default): Basic derive example with text and structured output
//! - `structured`: Structured output only
//! - `tracing-bridge`: With tracing integration (requires --features tracing)
//! - `log-bridge`: With log crate forwarding (requires --features log)
//! - `async-sleep`: Async tokio CLI with 3 sleep tasks, dedicated thread (requires --features tracing)
//! - `async-sleep-shared`: Same but shares the MCP server's runtime (requires --features tracing)

use async_trait::async_trait;
use clap::{Parser, Subcommand};
use rust_mcp_sdk::{
    McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
    error::SdkResult,
    mcp_client::{ClientHandler, McpClientOptions, client_runtime},
    schema::{
        CallToolRequestParams, CancelledNotificationParams, ClientCapabilities, Implementation,
        InitializeRequestParams, LATEST_PROTOCOL_VERSION, ListPromptsResult, ListResourcesResult,
        LoggingMessageNotificationParams, NotificationParams, ProgressNotificationParams,
        ResourceUpdatedNotificationParams, RpcError,
    },
};

#[derive(Clone)]
struct ExampleClientHandler {
    json: bool,
}

#[async_trait]
impl ClientHandler for ExampleClientHandler {
    async fn handle_logging_message_notification(
        &self,
        params: LoggingMessageNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if self.json {
            println!("{}", serde_json::to_string(&params).unwrap_or_default());
        } else {
            let logger = params.logger.as_deref().unwrap_or("unknown");
            let level = format!("{:?}", params.level).to_uppercase();
            println!("  [LOG {level} ({logger})] {}", params.data);
        }
        Ok(())
    }

    async fn handle_progress_notification(
        &self,
        params: ProgressNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
        Ok(())
    }

    async fn handle_cancelled_notification(
        &self,
        params: CancelledNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
        Ok(())
    }

    async fn handle_resource_list_changed_notification(
        &self,
        params: Option<NotificationParams>,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
        Ok(())
    }

    async fn handle_resource_updated_notification(
        &self,
        params: ResourceUpdatedNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
        Ok(())
    }

    async fn handle_prompt_list_changed_notification(
        &self,
        params: Option<NotificationParams>,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
        Ok(())
    }

    async fn handle_tool_list_changed_notification(
        &self,
        params: Option<NotificationParams>,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if self.json {
            eprintln!("{}", serde_json::to_string(&params).unwrap_or_default());
        }
        Ok(())
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
    /// Test the derive example (default)
    Derive,
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
    /// Test the log_bridge example (requires --features log)
    #[cfg(feature = "log")]
    LogBridge,
}

fn server_args(example: &str) -> Vec<String> {
    let feature = match example {
        "tracing_bridge" | "async_sleep" | "async_sleep_shared" => Some("tracing"),
        "log_bridge" => Some("log"),
        _ => None,
    };
    let mut args = vec!["run".into(), "--example".into(), example.into()];
    if let Some(f) = feature {
        args.push("--features".into());
        args.push(f.into());
    }
    args.push("--".into());
    args.push("--mcp".into());
    args
}

async fn run_client(example: &str, json: bool) -> SdkResult<()> {
    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "clap-mcp-client-example".into(),
            version: "0.1.0".into(),
            title: Some("clap-mcp client example".into()),
            description: Some(format!("Tests clap-mcp {} example", example)),
            icons: vec![],
            website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    let transport = StdioTransport::create_with_server_launch(
        "cargo",
        server_args(example),
        None,
        TransportOptions::default(),
    )?;

    let client = client_runtime::create_client(McpClientOptions {
        client_details,
        transport,
        handler: ExampleClientHandler { json }.to_mcp_client_handler(),
        task_store: None,
        server_task_store: None,
    });

    client.clone().start().await?;

    let ListResourcesResult { resources, .. } = client.request_resource_list(None).await?;
    println!("Resources:");
    for res in &resources {
        println!("- {} ({})", res.name, res.uri);
    }

    let tools_result = client.request_tool_list(None).await?;
    println!("\nTools:");
    for t in &tools_result.tools {
        println!("  {}: {}", t.name, t.description.as_deref().unwrap_or(""));
    }

    if example == "derive" {
        run_derive_tests(client.as_ref()).await?;
    } else if example == "structured" {
        run_structured_tests(client.as_ref()).await?;
    } else if example == "async_sleep" || example == "async_sleep_shared" {
        run_async_sleep_tests(client.as_ref()).await?;
    } else if example == "tracing_bridge" || example == "log_bridge" {
        run_logging_tests(client.as_ref()).await?;
    }

    client.shut_down().await?;
    Ok(())
}

async fn run_derive_tests(client: &impl McpClient) -> SdkResult<()> {
    let mut greet_args = serde_json::Map::new();
    greet_args.insert("name".into(), serde_json::json!("Rust"));
    let greet_result = client
        .request_tool_call(CallToolRequestParams {
            name: "greet".into(),
            arguments: Some(greet_args),
            meta: None,
            task: None,
        })
        .await?;
    println!("\nCall 'greet' with name=\"Rust\":");
    for block in &greet_result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }

    let mut add_args = serde_json::Map::new();
    add_args.insert("a".into(), serde_json::json!(2));
    add_args.insert("b".into(), serde_json::json!(3));
    let add_result = client
        .request_tool_call(CallToolRequestParams {
            name: "add".into(),
            arguments: Some(add_args),
            meta: None,
            task: None,
        })
        .await?;
    println!("\nCall 'add' with a=2, b=3:");
    for block in &add_result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }

    let mut sub_args = serde_json::Map::new();
    sub_args.insert("a".into(), serde_json::json!(10));
    sub_args.insert("b".into(), serde_json::json!(5));
    let sub_result = client
        .request_tool_call(CallToolRequestParams {
            name: "sub".into(),
            arguments: Some(sub_args),
            meta: None,
            task: None,
        })
        .await?;
    println!("\nCall 'sub' with a=10, b=5 (structured output):");
    for block in &sub_result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }
    if let Some(ref structured) = sub_result.structured_content {
        println!(
            "  structured_content: {}",
            serde_json::to_string_pretty(structured).unwrap()
        );
    }

    Ok(())
}

async fn run_async_sleep_tests(client: &impl McpClient) -> SdkResult<()> {
    let result = client
        .request_tool_call(CallToolRequestParams {
            name: "sleep-demo".into(),
            arguments: None,
            meta: None,
            task: None,
        })
        .await?;
    println!("\nCall 'sleep-demo':");
    for block in &result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }
    if let Some(ref structured) = result.structured_content {
        println!(
            "  structured_content: {}",
            serde_json::to_string_pretty(structured).unwrap()
        );
    }
    Ok(())
}

async fn run_structured_tests(client: &impl McpClient) -> SdkResult<()> {
    let mut args = serde_json::Map::new();
    args.insert("a".into(), serde_json::json!(7));
    args.insert("b".into(), serde_json::json!(3));
    let result = client
        .request_tool_call(CallToolRequestParams {
            name: "add".into(),
            arguments: Some(args),
            meta: None,
            task: None,
        })
        .await?;
    println!("\nCall 'add' with a=7, b=3:");
    for block in &result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }
    if let Some(ref structured) = result.structured_content {
        println!(
            "  structured_content: {}",
            serde_json::to_string_pretty(structured).unwrap()
        );
    }
    Ok(())
}

async fn run_logging_tests(client: &impl McpClient) -> SdkResult<()> {
    let ListPromptsResult { prompts, .. } = client.request_prompt_list(None).await?;
    println!("\nPrompts:");
    for p in &prompts {
        println!("  {}: {}", p.name, p.description.as_deref().unwrap_or(""));
    }

    let mut args = serde_json::Map::new();
    args.insert("s".into(), serde_json::json!("hello"));
    let result = client
        .request_tool_call(CallToolRequestParams {
            name: "echo".into(),
            arguments: Some(args),
            meta: None,
            task: None,
        })
        .await?;
    println!("\nCall 'echo' with s=\"hello\":");
    for block in &result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    let args = Args::parse();
    let example = match args.command {
        Cli::Derive => "derive",
        Cli::Structured => "structured",
        #[cfg(feature = "tracing")]
        Cli::TracingBridge => "tracing_bridge",
        #[cfg(feature = "tracing")]
        Cli::AsyncSleep => "async_sleep",
        #[cfg(feature = "tracing")]
        Cli::AsyncSleepShared => "async_sleep_shared",
        #[cfg(feature = "log")]
        Cli::LogBridge => "log_bridge",
    };
    run_client(example, args.json).await
}
