//! Tests for [`ServeMcpBuilder::stdio_io`] custom transport.

use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpConfig, ClapMcpSchemaMetadata, McpListen, ServeMcpBuilder};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage, ServerResult};
use rmcp::transport::{IntoTransport, Transport};
use rmcp::{ClientHandler, ServiceExt};
use std::time::Duration;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from = "run_stdio_io_test"]
#[command(name = "stdio-io-test-cli")]
enum StdioIoTestCli {
    Ping,
}

fn run_stdio_io_test(cmd: StdioIoTestCli) -> String {
    match cmd {
        StdioIoTestCli::Ping => "pong".to_string(),
    }
}

#[derive(Clone, Default)]
struct NoOpHandler;

impl ClientHandler for NoOpHandler {}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdio_io_duplex_list_tools() {
    let (io1, io2) = tokio::io::duplex(8192);
    let (server_read, server_write) = tokio::io::split(io1);
    let (client_read, client_write) = tokio::io::split(io2);

    let server = tokio::spawn(async move {
        ServeMcpBuilder::for_cli::<StdioIoTestCli>(McpListen::Stdio)
            .stdio_io(server_read, server_write)
            .serve()
            .await
            .expect("stdio_io server should start");
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = NoOpHandler
        .serve((client_read, client_write))
        .await
        .expect("stdio_io client should connect");

    let tools = client.list_tools(None).await.expect("list tools").tools;
    assert!(tools.iter().any(|t| t.name == "ping"));

    client.cancel().await.ok();
    server.abort();
    let _ = server.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdio_initialize_falls_back_from_untested_protocol_version() {
    let (io1, io2) = tokio::io::duplex(8192);
    let (server_read, server_write) = tokio::io::split(io1);
    let (client_read, client_write) = tokio::io::split(io2);

    let server = tokio::spawn(async move {
        ServeMcpBuilder::for_cli::<StdioIoTestCli>(McpListen::Stdio)
            .stdio_io(server_read, server_write)
            .serve()
            .await
            .expect("stdio_io server should start");
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client =
        IntoTransport::<rmcp::RoleClient, _, _>::into_transport((client_read, client_write));
    let init: ClientJsonRpcMessage = serde_json::from_str(
        r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "proto-test", "version": "0.0.0" }
            }
        }"#,
    )
    .expect("init request JSON");
    client.send(init).await.expect("send initialize");
    let response = client.receive().await.expect("initialize response");
    let ServerJsonRpcMessage::Response(resp) = response else {
        panic!("expected JSON-RPC response, got {response:?}");
    };
    let ServerResult::InitializeResult(result) = resp.result else {
        panic!("expected InitializeResult, got {:?}", resp.result);
    };
    assert_eq!(
        result.protocol_version.as_str(),
        "2025-11-25",
        "stdio must not echo untested protocol versions"
    );

    server.abort();
    let _ = server.await;
}

#[test]
fn stdio_io_defaults_unchanged() {
    ServeMcpBuilder::new()
        .listen(McpListen::Stdio)
        .schema_json("{}")
        .config(ClapMcpConfig::default())
        .metadata(ClapMcpSchemaMetadata::default())
        .build()
        .expect("builder without stdio_io should build");
}

#[cfg(feature = "http")]
#[test]
fn stdio_io_rejects_with_http_listen() {
    let (read, write) = tokio::io::duplex(8192);
    let err = match ServeMcpBuilder::new()
        .listen(McpListen::Http("127.0.0.1:0".parse().expect("addr")))
        .stdio_io(read, write)
        .schema_json("{}")
        .config(ClapMcpConfig::default())
        .metadata(ClapMcpSchemaMetadata::default())
        .build()
    {
        Ok(_) => panic!("stdio_io with Http should fail at build"),
        Err(err) => err,
    };
    match err {
        clap_mcp::ClapMcpError::InvalidConfig(message) => {
            assert!(message.contains("stdio_io"));
            assert!(message.contains("Stdio"));
        }
        other => panic!("expected InvalidConfig, got {other:?}"),
    }
}
