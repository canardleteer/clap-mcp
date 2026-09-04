//! Integration tests for server instructions, application identity,
//! and per-tool annotations.

use clap::Parser;
use clap_mcp::{
    ClapMcp, ClapMcpServeOptions, Implementation, McpListen, ServeMcpBuilder, ToolAnnotations,
    logging,
};
use rmcp::model::CallToolRequestParams;
use rmcp::{ClientHandler, ServiceExt};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, tool_title = "Root CLI Title", read_only)]
#[clap_mcp_output_from = "run_test_cli"]
#[command(name = "annotated-test-cli")]
enum AnnotatedTestCli {
    #[clap_mcp(read_only, idempotent, tool_title = "Fetch Data")]
    Fetch {
        #[arg(long)]
        id: String,
    },
    #[clap_mcp(destructive, open_world)]
    Delete {
        #[arg(long)]
        id: String,
    },
    #[clap_mcp(annotation(
        read_only = false,
        destructive = false,
        idempotent = true,
        tool_title = "Custom Tool"
    ))]
    Custom {
        #[arg(long)]
        name: String,
    },
    Plain {
        #[arg(long)]
        value: String,
    },
}

fn run_test_cli(cmd: AnnotatedTestCli) -> String {
    match cmd {
        AnnotatedTestCli::Fetch { id } => format!("fetched:{id}"),
        AnnotatedTestCli::Delete { id } => format!("deleted:{id}"),
        AnnotatedTestCli::Custom { name } => format!("custom:{name}"),
        AnnotatedTestCli::Plain { value } => format!("plain:{value}"),
    }
}

#[derive(Clone, Default)]
struct TestClientHandler;

impl ClientHandler for TestClientHandler {}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_server_metadata_and_annotations_over_stdio() {
    let (io1, io2) = tokio::io::duplex(8192);
    let (server_read, server_write) = tokio::io::split(io1);
    let (client_read, client_write) = tokio::io::split(io2);

    let custom_server_info = Implementation::new("test-app", "1.2.3")
        .with_title("Test App Title")
        .with_description("Test App Description");

    let custom_raw_tool = rmcp::model::Tool::new(
        "custom_raw",
        "Custom raw tool description",
        Arc::new(serde_json::Map::new()),
    );

    let server_task = tokio::spawn(async move {
        ServeMcpBuilder::for_cli::<AnnotatedTestCli>(McpListen::Stdio)
            .stdio_io(server_read, server_write)
            .server_info(custom_server_info)
            .instructions("Application level instructions for testing.")
            .custom_tool(custom_raw_tool)
            .tool_annotation(
                "custom_raw",
                ToolAnnotations::from_raw(
                    Some("Annotated Custom Raw".into()),
                    Some(true),
                    Some(false),
                    Some(true),
                    Some(false),
                ),
            )
            .tool_annotation(
                "plain",
                ToolAnnotations::from_raw(
                    Some("Imperative Plain".into()),
                    Some(true),
                    Some(false),
                    None,
                    None,
                ),
            )
            .serve()
            .await
            .expect("server should run");
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = TestClientHandler
        .serve((client_read, client_write))
        .await
        .expect("client should connect");

    // 1. Verify Initialize Result / Server Info
    let peer_info = client.peer_info().expect("peer_info from initialize");
    let server_info = peer_info.server_info.as_ref().expect("server_info");
    assert_eq!(server_info.name, "test-app");
    assert_eq!(server_info.version, "1.2.3");
    assert_eq!(server_info.title.as_deref(), Some("Test App Title"));
    assert_eq!(
        server_info.description.as_deref(),
        Some("Test App Description")
    );

    // 2. Verify instructions appear in initialize
    assert_eq!(
        peer_info.instructions.as_deref(),
        Some("Application level instructions for testing.")
    );

    // 3. Setting instructions without logging MUST NOT enable logging capability
    assert!(
        peer_info.capabilities.logging.is_none(),
        "logging capability must not be enabled when only instructions are set"
    );

    // 4. Verify tools/list annotations
    let tools = client.list_tools(None).await.expect("list_tools").tools;

    // Check enum root tool annotations
    let root = tools
        .iter()
        .find(|t| t.name == "annotated-test-cli")
        .expect("root tool");
    assert_eq!(root.title.as_deref(), Some("Root CLI Title"));
    let root_ann = root.annotations.as_ref().expect("root annotations");
    assert_eq!(root_ann.title.as_deref(), Some("Root CLI Title"));
    assert_eq!(root_ann.read_only_hint, Some(true));

    // Check `fetch`
    let fetch = tools
        .iter()
        .find(|t| t.name == "fetch")
        .expect("fetch tool");
    assert_eq!(fetch.title.as_deref(), Some("Fetch Data"));
    let fetch_ann = fetch.annotations.as_ref().expect("fetch annotations");
    assert_eq!(fetch_ann.title.as_deref(), Some("Fetch Data"));
    assert_eq!(fetch_ann.read_only_hint, Some(true));
    assert_eq!(fetch_ann.idempotent_hint, Some(true));
    assert_eq!(fetch_ann.destructive_hint, None);
    assert_eq!(fetch_ann.open_world_hint, None);

    // Check `delete`
    let delete = tools
        .iter()
        .find(|t| t.name == "delete")
        .expect("delete tool");
    let delete_ann = delete.annotations.as_ref().expect("delete annotations");
    assert_eq!(delete_ann.destructive_hint, Some(true));
    assert_eq!(delete_ann.open_world_hint, Some(true));
    assert_eq!(delete_ann.read_only_hint, None);

    // Check `custom`
    let custom = tools
        .iter()
        .find(|t| t.name == "custom")
        .expect("custom tool");
    assert_eq!(custom.title.as_deref(), Some("Custom Tool"));
    let custom_ann = custom.annotations.as_ref().expect("custom annotations");
    assert_eq!(custom_ann.title.as_deref(), Some("Custom Tool"));
    assert_eq!(custom_ann.read_only_hint, Some(false));
    assert_eq!(custom_ann.destructive_hint, Some(false));
    assert_eq!(custom_ann.idempotent_hint, Some(true));

    // Check `plain` (imperative annotation via ServeMcpBuilder)
    let plain = tools
        .iter()
        .find(|t| t.name == "plain")
        .expect("plain tool");
    assert_eq!(plain.title.as_deref(), Some("Imperative Plain"));
    let plain_ann = plain.annotations.as_ref().expect("plain annotations");
    assert_eq!(plain_ann.title.as_deref(), Some("Imperative Plain"));
    assert_eq!(plain_ann.read_only_hint, Some(true));
    assert_eq!(plain_ann.destructive_hint, Some(false));

    // Check `custom_raw` (custom tool with imperative tool_annotation override)
    let custom_raw = tools
        .iter()
        .find(|t| t.name == "custom_raw")
        .expect("custom_raw tool");
    assert_eq!(custom_raw.title.as_deref(), Some("Annotated Custom Raw"));
    let custom_raw_ann = custom_raw
        .annotations
        .as_ref()
        .expect("custom_raw annotations");
    assert_eq!(
        custom_raw_ann.title.as_deref(),
        Some("Annotated Custom Raw")
    );
    assert_eq!(custom_raw_ann.read_only_hint, Some(true));
    assert_eq!(custom_raw_ann.destructive_hint, Some(false));
    assert_eq!(custom_raw_ann.idempotent_hint, Some(true));
    assert_eq!(custom_raw_ann.open_world_hint, Some(false));

    // 5. Verify annotated tools remain executable via clap-mcp handler
    let call_res =
        client
            .call_tool(CallToolRequestParams::new("fetch").with_arguments(
                serde_json::Map::from_iter([("id".to_string(), serde_json::json!("12345"))]),
            ))
            .await
            .expect("call fetch tool");
    let text = call_res
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or_default();
    assert_eq!(text, "fetched:12345");

    let delete_res =
        client
            .call_tool(CallToolRequestParams::new("delete").with_arguments(
                serde_json::Map::from_iter([("id".to_string(), serde_json::json!("abc"))]),
            ))
            .await
            .expect("call delete tool");
    let del_text = delete_res
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.as_str())
        .unwrap_or_default();
    assert_eq!(del_text, "deleted:abc");

    client.cancel().await.ok();
    server_task.abort();
    let _ = server_task.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_instructions_precede_logging_when_both_enabled() {
    let (io1, io2) = tokio::io::duplex(8192);
    let (server_read, server_write) = tokio::io::split(io1);
    let (client_read, client_write) = tokio::io::split(io2);

    let (log_tx, log_rx) = logging::log_channel(16);
    drop(log_tx);

    let server_task = tokio::spawn(async move {
        ServeMcpBuilder::for_cli::<AnnotatedTestCli>(McpListen::Stdio)
            .stdio_io(server_read, server_write)
            .serve_options(ClapMcpServeOptions::default().with_log_rx(log_rx))
            .instructions("Primary application instructions.")
            .serve()
            .await
            .expect("server should run");
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = TestClientHandler
        .serve((client_read, client_write))
        .await
        .expect("client should connect");

    let peer_info = client.peer_info().expect("peer_info from initialize");
    assert!(peer_info.capabilities.logging.is_some());
    let instructions = peer_info.instructions.as_deref().expect("instructions");
    assert!(
        instructions.starts_with("Primary application instructions.\n\n"),
        "application instructions must precede logging guidance: {instructions}"
    );
    assert!(
        instructions.contains("When this server emits log messages"),
        "logging guidance must be appended: {instructions}"
    );

    client.cancel().await.ok();
    server_task.abort();
    let _ = server_task.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_discover_handler_includes_identity_and_instructions() {
    let (io1, io2) = tokio::io::duplex(8192);
    let (server_read, server_write) = tokio::io::split(io1);
    let (client_read, client_write) = tokio::io::split(io2);

    let custom_server_info = Implementation::new("discover-app", "9.9.9")
        .with_title("Discover App")
        .with_description("Discover Description");

    let server_task = tokio::spawn(async move {
        ServeMcpBuilder::for_cli::<AnnotatedTestCli>(McpListen::Stdio)
            .stdio_io(server_read, server_write)
            .server_info(custom_server_info)
            .instructions("Discover Instructions")
            .serve()
            .await
            .expect("server should run");
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = TestClientHandler
        .serve((client_read, client_write))
        .await
        .expect("client should connect");

    let meta = rmcp::model::RequestMetaObject::with_client_context(
        rmcp::model::ProtocolVersion::LATEST,
        Implementation::new("test-client", "1.0.0"),
        rmcp::model::ClientCapabilities::default(),
    );
    let discover_result = client.discover(meta).await.expect("discover");

    let discover_info = discover_result.server_info().expect("discover server_info");
    assert_eq!(discover_info.name, "discover-app");
    assert_eq!(discover_info.version, "9.9.9");
    assert_eq!(discover_info.title.as_deref(), Some("Discover App"));
    assert_eq!(
        discover_info.description.as_deref(),
        Some("Discover Description")
    );
    assert_eq!(
        discover_result.instructions.as_deref(),
        Some("Discover Instructions")
    );

    client.cancel().await.ok();
    server_task.abort();
    let _ = server_task.await;
}
