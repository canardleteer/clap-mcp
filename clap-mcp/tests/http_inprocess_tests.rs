#![cfg(feature = "http")]
// resources/subscribe|unsubscribe remain for legacy protocol peers (deprecated in rmcp 3).
#![allow(deprecated)]
mod common;

use clap::Parser;
use clap_mcp::{ClapMcp, McpListen, ServeMcpBuilder};
use common::{shutdown, tool_text};
use rmcp::model::{CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from = "run_http_test"]
#[command(name = "http-test-cli")]
enum HttpTestCli {
    Echo {
        #[arg(long)]
        message: String,
    },
}

fn run_http_test(cmd: HttpTestCli) -> String {
    match cmd {
        HttpTestCli::Echo { message } => format!("echo: {message}"),
    }
}

#[derive(Clone, Default)]
struct NoOpHandler;

impl ClientHandler for NoOpHandler {}

async fn wait_for_http(addr: SocketAddr) {
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("HTTP server not ready at {addr}");
}

async fn connect_http_client(
    addr: SocketAddr,
) -> rmcp::service::RunningService<RoleClient, NoOpHandler> {
    wait_for_http(addr).await;
    let uri = format!("http://{addr}/mcp");
    NoOpHandler
        .serve(StreamableHttpClientTransport::from_uri(uri))
        .await
        .expect("connect")
}

async fn exercise_inprocess_http(listen: SocketAddr, connect: SocketAddr) {
    let server = tokio::spawn(async move {
        let _ = ServeMcpBuilder::for_cli::<HttpTestCli>(McpListen::Http(listen))
            .serve()
            .await;
    });

    wait_for_http(connect).await;

    let uri = format!("http://{connect}/mcp");
    let transport = StreamableHttpClientTransport::from_uri(uri);
    let client = NoOpHandler.serve(transport).await.expect("connect");

    let tools = client.list_tools(None).await.expect("list tools").tools;
    assert!(tools.iter().any(|t| t.name == "echo"));

    let result = client
        .call_tool(
            CallToolRequestParams::new("echo").with_arguments(serde_json::Map::from_iter([(
                "message".to_string(),
                serde_json::json!("inprocess"),
            )])),
        )
        .await
        .expect("call echo");
    assert!(tool_text(&result).contains("inprocess"));

    shutdown(client).await;
    server.abort();
    let _ = server.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_inprocess_resources_prompts_and_placeholder_tool() {
    use clap_mcp::{
        ClapMcpConfig, ClapMcpSchemaMetadata, PROMPT_LOGGING_GUIDE, schema_from_command,
    };

    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let (tx, rx) = clap_mcp::logging::log_channel(4);
    drop(tx);
    let schema = schema_from_command(
        &clap::Command::new("placeholder-cli")
            .subcommand(clap::Command::new("echo").arg(clap::Arg::new("message").long("message"))),
    );
    let server = tokio::spawn(async move {
        let _ = ServeMcpBuilder::new()
            .listen(McpListen::Http(addr))
            .schema_json(serde_json::to_string(&schema).expect("schema json"))
            .config(ClapMcpConfig::default())
            .metadata(ClapMcpSchemaMetadata::default())
            .serve_options(clap_mcp::ClapMcpServeOptions {
                log_rx: Some(rx),
                ..Default::default()
            })
            .in_process_handler(None)
            .executable_path(None)
            .serve()
            .await;
    });

    wait_for_http(addr).await;
    let client = connect_http_client(addr).await;

    let resources = client
        .list_resources(None)
        .await
        .expect("resources")
        .resources;
    assert!(resources.iter().any(|r| r.uri == "clap://schema"));

    let schema_resource = client
        .read_resource(ReadResourceRequestParams::new("clap://schema"))
        .await
        .expect("read schema");
    assert!(common::read_text(&schema_resource).contains("placeholder-cli"));

    let prompts = client.list_prompts(None).await.expect("prompts").prompts;
    assert!(prompts.iter().any(|p| p.name == PROMPT_LOGGING_GUIDE));

    let guide = client
        .get_prompt(GetPromptRequestParams::new(PROMPT_LOGGING_GUIDE))
        .await
        .expect("logging guide");
    assert!(common::prompt_has_text(
        &guide.messages,
        "notifications/message",
    ));

    let placeholder =
        client
            .call_tool(CallToolRequestParams::new("echo").with_arguments(
                serde_json::Map::from_iter([("message".to_string(), serde_json::json!("hi"))]),
            ))
            .await
            .expect("placeholder tool");
    assert!(tool_text(&placeholder).contains("Would invoke clap command"));

    client
        .subscribe(rmcp::model::SubscribeRequestParams::new("clap://schema"))
        .await
        .expect("resources/subscribe should succeed");
    client
        .unsubscribe(rmcp::model::UnsubscribeRequestParams::new("clap://schema"))
        .await
        .expect("resources/unsubscribe should succeed");
    client
        .unsubscribe(rmcp::model::UnsubscribeRequestParams::new(
            "clap://never-subscribed",
        ))
        .await
        .expect("unsubscribe of unknown URI should still succeed");

    shutdown(client).await;
    server.abort();
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[clap_mcp_output_from = "run_subprocess_http_test"]
#[command(name = "subprocess-http-test-cli")]
enum SubprocessHttpTestCli {
    Emit {
        #[arg(long, default_value = "ok")]
        message: String,
    },
}

fn run_subprocess_http_test(cmd: SubprocessHttpTestCli) -> String {
    match cmd {
        SubprocessHttpTestCli::Emit { message } => {
            eprintln!("stderr:{message}");
            format!("stdout:{message}")
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_inprocess_subprocess_tool_executes_in_same_coverage_process() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let server = tokio::spawn(async move {
        let _ = ServeMcpBuilder::for_cli::<SubprocessHttpTestCli>(McpListen::Http(addr))
            .serve()
            .await;
    });

    wait_for_http(addr).await;
    let client = connect_http_client(addr).await;

    let result =
        client
            .call_tool(CallToolRequestParams::new("emit").with_arguments(
                serde_json::Map::from_iter([("message".to_string(), serde_json::json!("cov"))]),
            ))
            .await
            .expect("subprocess tool call should return");
    let text = tool_text(&result);
    assert!(
        text.contains("stdout:cov")
            || text.contains("stderr:cov")
            || text.contains("Would invoke")
            || result.is_error == Some(true),
        "unexpected subprocess tool result: {text}"
    );

    shutdown(client).await;
    server.abort();
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, catch_in_process_panics)]
#[clap_mcp_output_from = "run_panic_http_test"]
#[command(name = "panic-http-test-cli")]
enum PanicHttpTestCli {
    PanicDemo,
}

fn run_panic_http_test(cmd: PanicHttpTestCli) -> String {
    match cmd {
        PanicHttpTestCli::PanicDemo => panic!("coverage panic"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_inprocess_in_process_panic_is_caught() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let server = tokio::spawn(async move {
        let _ = ServeMcpBuilder::for_cli::<PanicHttpTestCli>(McpListen::Http(addr))
            .serve()
            .await;
    });

    let client = connect_http_client(addr).await;
    let result = client
        .call_tool(CallToolRequestParams::new("panic-demo"))
        .await
        .expect("panic tool");
    assert_eq!(result.is_error, Some(true));
    assert!(tool_text(&result).contains("panicked"));

    shutdown(client).await;
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_inprocess_serve_list_and_call_tool() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    exercise_inprocess_http(addr, addr).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_inprocess_unspecified_bind_accepts_localhost() {
    let listener =
        tokio::net::TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
            .await
            .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let connect = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port());
    exercise_inprocess_http(addr, connect).await;
}
