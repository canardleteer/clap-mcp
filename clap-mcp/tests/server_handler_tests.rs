#![cfg(feature = "http")]
// Logging types remain functional in rmcp 2.x but are deprecated by SEP-2577.
#![allow(deprecated)]
mod common;

use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpServeOptions, McpListen, ServeMcpBuilder, logging};
use common::{
    TasksClientHandler, call_tool_task, get_task_info, shutdown, task_call_params, tool_text,
};
use rmcp::model::{CallToolRequestParams, TaskPayload, TaskStatus};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe, task_augmented_tools)]
#[clap_mcp_output_from = "run_task_http_test"]
#[command(name = "task-http-test-cli")]
enum TaskHttpTestCli {
    #[clap_mcp(task)]
    Echo {
        #[arg(long)]
        message: String,
    },
    Plain {
        #[arg(long)]
        message: String,
    },
}

fn run_task_http_test(cmd: TaskHttpTestCli) -> String {
    match cmd {
        TaskHttpTestCli::Echo { message } | TaskHttpTestCli::Plain { message } => {
            format!("echo: {message}")
        }
    }
}

#[derive(Clone, Default)]
struct NoOpHandler;

impl ClientHandler for NoOpHandler {}

async fn launch_inprocess_http_server(builder: ServeMcpBuilder) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let _ = builder.serve().await;
    })
}

async fn connect_http_client<H: ClientHandler + Clone + Send + Sync + 'static>(
    connect: SocketAddr,
    handler: H,
) -> rmcp::service::RunningService<RoleClient, H> {
    let mut connected = false;
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(connect).await.is_ok() {
            connected = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        connected,
        "in-process HTTP server not listening on {connect}"
    );
    let uri = format!("http://{connect}/mcp");
    handler
        .serve(StreamableHttpClientTransport::from_uri(uri))
        .await
        .expect("connect")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inprocess_http_get_task_info_unknown_task_errors() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let server = launch_inprocess_http_server(ServeMcpBuilder::for_cli::<TaskHttpTestCli>(
        McpListen::Http(addr),
    ))
    .await;
    let client = connect_http_client(addr, TasksClientHandler).await;
    let peer = client.peer();

    let err = get_task_info(peer, "missing-task")
        .await
        .expect_err("unknown task should error");
    assert!(format!("{err:?}").contains("unknown task"));

    shutdown(client).await;
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inprocess_http_non_augmented_tool_returns_sync_for_tasks_client() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let server = launch_inprocess_http_server(ServeMcpBuilder::for_cli::<TaskHttpTestCli>(
        McpListen::Http(addr),
    ))
    .await;
    let client = connect_http_client(addr, TasksClientHandler).await;
    let peer = client.peer();

    let args =
        serde_json::Map::from_iter([("message".to_string(), serde_json::json!("sync-plain"))]);

    let result = peer
        .call_tool(CallToolRequestParams::new("plain").with_arguments(args.clone()))
        .await
        .expect("plain tool should complete synchronously");
    assert!(tool_text(&result).contains("sync-plain"));

    let err = call_tool_task(peer, task_call_params("plain", args))
        .await
        .expect_err("non-augmented tool must not return CreateTaskResult");
    assert!(matches!(err, rmcp::ServiceError::UnexpectedResponse));

    shutdown(client).await;
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inprocess_http_task_invalid_args_report_failed_status() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let server = launch_inprocess_http_server(ServeMcpBuilder::for_cli::<TaskHttpTestCli>(
        McpListen::Http(addr),
    ))
    .await;
    let client = connect_http_client(addr, TasksClientHandler).await;
    let peer = client.peer();

    let create = call_tool_task(
        peer,
        task_call_params(
            "echo",
            serde_json::Map::from_iter([("bogus".to_string(), serde_json::json!(1))]),
        ),
    )
    .await
    .expect("task create");

    let mut status = TaskStatus::Working;
    for _ in 0..50 {
        status = get_task_info(peer, &create.task.task_id)
            .await
            .expect("task info")
            .task
            .status();
        if status == TaskStatus::Failed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(status, TaskStatus::Failed);

    let st = get_task_info(peer, &create.task.task_id)
        .await
        .expect("task info");
    assert!(matches!(st.task.payload, TaskPayload::Failed { .. }));

    shutdown(client).await;
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inprocess_http_logging_channel_forwards_after_client_connect() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let (tx, rx) = logging::log_channel(8);
    let server = launch_inprocess_http_server(
        ServeMcpBuilder::for_cli::<TaskHttpTestCli>(McpListen::Http(addr)).serve_options(
            ClapMcpServeOptions {
                log_rx: Some(rx),
                ..Default::default()
            },
        ),
    )
    .await;

    tx.send(logging::log_params(
        rmcp::model::LoggingLevel::Info,
        Some("app".into()),
        "before client",
    ))
    .await
    .expect("queue log");

    let client = connect_http_client(addr, NoOpHandler).await;
    tx.send(logging::log_params(
        rmcp::model::LoggingLevel::Info,
        Some("app".into()),
        "after client",
    ))
    .await
    .expect("queue log after connect");

    let result =
        client
            .call_tool(CallToolRequestParams::new("echo").with_arguments(
                serde_json::Map::from_iter([("message".to_string(), serde_json::json!("logged"))]),
            ))
            .await
            .expect("call tool");
    assert!(tool_text(&result).contains("logged"));

    shutdown(client).await;
    server.abort();
}
