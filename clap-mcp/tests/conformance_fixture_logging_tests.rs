//! Smoke test: conformance HTTP fixture forwards log notifications during tool calls.

#![cfg(all(feature = "http", feature = "tracing"))]
#![allow(clippy::await_holding_lock)]
// Logging types remain functional in rmcp 2.x but are deprecated by SEP-2577.
#![allow(deprecated)]

mod common;

use common::{shutdown, workspace_root};
use rmcp::model::{
    CallToolRequestParams, LoggingLevel, LoggingMessageNotificationParam, SetLevelRequestParams,
};
use rmcp::service::NotificationContext;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ClientHandler, ServiceExt};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Both tests spawn subprocess HTTP servers; serialize to avoid macOS CI hangs when
/// they run in parallel within this integration test binary.
static CONFORMANCE_FIXTURE_LOCK: Mutex<()> = Mutex::new(());

fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    CONFORMANCE_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

const STEP_TIMEOUT: Duration = Duration::from_secs(30);

/// Streamable HTTP `cancel()` can stall on macOS CI; bound the wait and drop on timeout.
const CLIENT_CANCEL_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(target_os = "macos")]
const CLIENT_CANCEL_KILL_SERVER_AFTER: Duration = Duration::from_secs(5);

async fn step_timeout<T>(label: &str, fut: impl std::future::Future<Output = T>) -> T {
    match tokio::time::timeout(STEP_TIMEOUT, fut).await {
        Ok(value) => value,
        Err(_) => panic!("conformance fixture step timed out after {STEP_TIMEOUT:?}: {label}"),
    }
}

#[derive(Clone, Default)]
struct LogCounter {
    count: std::sync::Arc<Mutex<usize>>,
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

async fn spawn_conformance_server(port: u16) -> tokio::process::Child {
    {
        let _guard = common::BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let status = std::process::Command::new("cargo")
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
    tokio::process::Command::new(exe)
        .arg("--mcp-http")
        .arg(format!("127.0.0.1:{port}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn conformance server")
}

async fn shutdown_child(mut child: tokio::process::Child) {
    let _ = child.start_kill();
    step_timeout("conformance server exit", child.wait())
        .await
        .expect("conformance server wait failed");
}

async fn wait_for_http(port: u16) {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    step_timeout("HTTP listen ready", async {
        for _ in 0..80 {
            if tokio::net::TcpStream::connect(addr).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("conformance server not ready on {port}");
    })
    .await;
}

async fn shutdown_http_client<H: ClientHandler>(
    client: rmcp::service::RunningService<rmcp::RoleClient, H>,
) {
    let _ = tokio::time::timeout(CLIENT_CANCEL_TIMEOUT, shutdown(client)).await;
}

async fn teardown_fixture<H: ClientHandler>(
    client: rmcp::service::RunningService<rmcp::RoleClient, H>,
    child: tokio::process::Child,
) {
    #[cfg(target_os = "macos")]
    {
        let mut child = child;
        tokio::select! {
            _ = tokio::time::timeout(CLIENT_CANCEL_TIMEOUT, shutdown(client)) => {}
            _ = async {
                tokio::time::sleep(CLIENT_CANCEL_KILL_SERVER_AFTER).await;
                let _ = child.start_kill();
            } => {}
        }
        shutdown_child(child).await;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = tokio::time::timeout(CLIENT_CANCEL_TIMEOUT, shutdown(client)).await;
        shutdown_child(child).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conformance_fixture_emits_logs_during_tool_call() {
    let _guard = serial_guard();

    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let child = spawn_conformance_server(port).await;
    wait_for_http(port).await;

    let counter = LogCounter::default();
    let uri = format!("http://127.0.0.1:{port}/mcp");
    let client = step_timeout(
        "connect logging client",
        counter
            .clone()
            .serve(StreamableHttpClientTransport::from_uri(uri)),
    )
    .await
    .expect("connect");

    step_timeout(
        "set debug level",
        client.set_level(SetLevelRequestParams::new(LoggingLevel::Debug)),
    )
    .await
    .expect("set level");

    step_timeout(
        "call test_tool_with_logging",
        client.call_tool(CallToolRequestParams::new("test_tool_with_logging")),
    )
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

    teardown_fixture(client, child).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conformance_fixture_logs_after_prior_set_level_session() {
    let _guard = serial_guard();

    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let child = spawn_conformance_server(port).await;
    wait_for_http(port).await;

    let uri = format!("http://127.0.0.1:{port}/mcp");

    // Mimic harness order: logging-set-level closes, then tools-call-with-logging connects.
    {
        let client = step_timeout(
            "connect set-level client",
            LogCounter::default().serve(StreamableHttpClientTransport::from_uri(uri.clone())),
        )
        .await
        .expect("connect set-level client");
        step_timeout(
            "set info level",
            client.set_level(SetLevelRequestParams::new(LoggingLevel::Info)),
        )
        .await
        .expect("set level");
        shutdown_http_client(client).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let counter = LogCounter::default();
    let client = step_timeout(
        "connect logging tool client",
        counter
            .clone()
            .serve(StreamableHttpClientTransport::from_uri(uri)),
    )
    .await
    .expect("connect logging tool client");

    step_timeout(
        "set debug level",
        client.set_level(SetLevelRequestParams::new(LoggingLevel::Debug)),
    )
    .await
    .expect("set debug level");

    step_timeout(
        "call test_tool_with_logging",
        client.call_tool(CallToolRequestParams::new("test_tool_with_logging")),
    )
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

    teardown_fixture(client, child).await;
}
