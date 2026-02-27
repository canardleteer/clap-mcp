use async_trait::async_trait;
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_client::{client_runtime, ClientHandler, McpClientOptions},
    schema::{
        ClientCapabilities, Implementation, InitializeRequestParams, ListResourcesResult,
        ReadResourceRequestParams, LATEST_PROTOCOL_VERSION,
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

    client.shut_down().await?;

    Ok(())
}

