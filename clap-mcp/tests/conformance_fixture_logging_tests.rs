//! Smoke test: conformance HTTP fixture forwards log notifications during tool calls.

#![cfg(all(feature = "http", feature = "tracing"))]

mod common;

use common::{shutdown, workspace_root};
use rmcp::model::{
    CallToolRequestParams, LoggingLevel, LoggingMessageNotificationParam, SetLevelRequestParams,
};
use rmcp::service::NotificationContext;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ClientHandler, ServiceExt};
use std::net::{Ipv4Addr, SocketAddr};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
struct LogCounter {
    count: Arc<Mutex<usize>>,
}

impl ClientHandler for LogCounter {
    async fn on_logging_message(
        &self,
        _params: LoggingMessageNotificationParam,
        _context: NotificationContext<rmcp::RoleClient>,
    ) {
        *self.count.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    }
}

fn spawn_conformance_server(port: u16) -> Child {
    {
        let _guard = common::BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "clap-mcp-examples",
                "--bin",
                "clap-mcp-conformance-http",
                "--features",
                "http,tracing",
            ])
            .current_dir(workspace_root())
            .status()
            .expect("build conformance fixture");
        assert!(status.success());
    }

    let exe = workspace_root().join("target/debug/clap-mcp-conformance-http");
    Command::new(exe)
        .arg("--mcp-http")
        .arg(format!("127.0.0.1:{port}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn conformance server")
}

async fn shutdown_child(mut child: Child) {
    let _ = child.kill();
    tokio::time::timeout(
        Duration::from_secs(10),
        tokio::task::spawn_blocking(move || child.wait()),
    )
    .await
    .expect("conformance server did not exit within 10s")
    .expect("conformance server wait join failed")
    .expect("conformance server wait failed");
}

async fn wait_for_http(port: u16) {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    for _ in 0..80 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("conformance server not ready on {port}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conformance_fixture_emits_logs_during_tool_call() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut child = spawn_conformance_server(port);
    wait_for_http(port).await;

    let counter = LogCounter::default();
    let uri = format!("http://127.0.0.1:{port}/mcp");
    let client = counter
        .clone()
        .serve(StreamableHttpClientTransport::from_uri(uri))
        .await
        .expect("connect");

    client
        .set_level(SetLevelRequestParams::new(LoggingLevel::Debug))
        .await
        .expect("set level");

    client
        .call_tool(CallToolRequestParams::new("test_tool_with_logging"))
        .await
        .expect("call tool");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let n = *counter.count.lock().unwrap_or_else(|e| e.into_inner());
        if n >= 3 {
            break;
        }
        if Instant::now() >= deadline {
            panic!("expected >= 3 log notifications, got {n}");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    shutdown(client).await;
    shutdown_child(child).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conformance_fixture_logs_after_prior_set_level_session() {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut child = spawn_conformance_server(port);
    wait_for_http(port).await;

    let uri = format!("http://127.0.0.1:{port}/mcp");

    // Mimic harness order: logging-set-level closes, then tools-call-with-logging connects.
    {
        let client = LogCounter::default()
            .serve(StreamableHttpClientTransport::from_uri(uri.clone()))
            .await
            .expect("connect set-level client");
        client
            .set_level(SetLevelRequestParams::new(LoggingLevel::Info))
            .await
            .expect("set level");
        shutdown(client).await;
    }

    let counter = LogCounter::default();
    let client = counter
        .clone()
        .serve(StreamableHttpClientTransport::from_uri(uri))
        .await
        .expect("connect logging tool client");

    client
        .set_level(SetLevelRequestParams::new(LoggingLevel::Debug))
        .await
        .expect("set debug level");

    client
        .call_tool(CallToolRequestParams::new("test_tool_with_logging"))
        .await
        .expect("call tool");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let n = *counter.count.lock().unwrap_or_else(|e| e.into_inner());
        if n >= 3 {
            break;
        }
        if Instant::now() >= deadline {
            panic!("expected >= 3 log notifications after prior session, got {n}");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    shutdown(client).await;
    shutdown_child(child).await;
}
