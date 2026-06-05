#![cfg(feature = "http")]
mod common;

use common::{launch_http_example_with_addr, shutdown, tool_text};
use rmcp::model::CallToolRequestParams;
use std::net::{Ipv4Addr, SocketAddr};

#[tokio::test(flavor = "current_thread")]
async fn http_subcommands_list_and_call_tool() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let (client, server) = launch_http_example_with_addr("subcommands_http", addr)
        .await
        .expect("http client should connect");

    let tools = client.list_tools(None).await.expect("list tools").tools;
    assert!(tools.iter().any(|t| t.name == "greet"));

    let result =
        client
            .call_tool(CallToolRequestParams::new("greet").with_arguments(
                serde_json::Map::from_iter([("name".to_string(), serde_json::json!("http"))]),
            ))
            .await
            .expect("call greet");
    assert!(tool_text(&result).contains("http"));

    shutdown(client).await;
    server.shutdown().await;
}
