use async_trait::async_trait;
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_client::{client_runtime, ClientHandler, McpClientOptions},
    schema::{
        CallToolRequestParams, ClientCapabilities, Implementation, InitializeRequestParams,
        ListResourcesResult, ReadResourceRequestParams, LATEST_PROTOCOL_VERSION,
    },
    McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
};

#[derive(Clone)]
struct ExampleClientHandler;

#[async_trait]
impl ClientHandler for ExampleClientHandler {}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Client details
    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "clap-mcp-client-example".into(),
            version: "0.1.0".into(),
            title: Some("clap-mcp client example".into()),
            description: Some("Tests clap-mcp derive example as an MCP stdio server".into()),
            icons: vec![],
            website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    // Launch the derive example as an MCP stdio server: cargo run --example derive -- --mcp
    let transport = StdioTransport::create_with_server_launch(
        "cargo",
        vec![
            "run".into(),
            "--example".into(),
            "derive".into(),
            "--".into(),
            "--mcp".into(),
        ],
        None,
        TransportOptions::default(),
    )?;

    let handler = ExampleClientHandler;

    let client = client_runtime::create_client(McpClientOptions {
        client_details,
        transport,
        handler: handler.to_mcp_client_handler(),
        task_store: None,
        server_task_store: None,
    });

    client.clone().start().await?;

    // List resources and print them
    let ListResourcesResult { resources, .. } =
        client.request_resource_list(None).await?;

    println!("Resources:");
    for res in &resources {
        println!("- {} ({})", res.name, res.uri);
    }

    // Read the clap schema resource
    let uri = "clap://schema".to_string();
    let read = client
        .request_resource_read(ReadResourceRequestParams { meta: None, uri })
        .await?;

    println!("\nclap://schema contents (truncated):");
    if let Some(first) = read.contents.first() {
        println!("{first:?}");
    }

    // List tools (one per command/subcommand)
    let tools_result = client.request_tool_list(None).await?;
    println!("\nTools:");
    for t in &tools_result.tools {
        println!("  {}: {}", t.name, t.description.as_deref().unwrap_or(""));
    }

    // Call the "greet" tool with a name
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
    println!("\nCall tool 'greet' with name=\"Rust\":");
    for block in &greet_result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }

    // Call the "add" tool
    let mut args = serde_json::Map::new();
    args.insert("a".into(), serde_json::json!(2));
    args.insert("b".into(), serde_json::json!(3));
    let call_result = client
        .request_tool_call(CallToolRequestParams {
            name: "add".into(),
            arguments: Some(args),
            meta: None,
            task: None,
        })
        .await?;
    println!("\nCall tool 'add' with a=2, b=3:");
    for block in &call_result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }

    // Call the "sub" tool with 10 and 5
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
    println!("\nCall tool 'sub' with a=10, b=5:");
    for block in &sub_result.content {
        if let Ok(t) = block.as_text_content() {
            println!("  {}", t.text);
        }
    }

    client.shut_down().await?;

    Ok(())
}

