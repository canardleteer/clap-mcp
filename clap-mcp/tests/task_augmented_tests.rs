//! Integration tests for MCP task-augmented `tools/call` (both `share_runtime` configurations).

#![allow(clippy::await_holding_lock)] // `serial_test_guard` intentionally serializes subprocess MCP tests

use std::collections::HashMap;
use std::convert::TryInto;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rust_mcp_sdk::{
    McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
    error::SdkResult,
    mcp_client::{ClientHandler, ClientRuntime, McpClientOptions, client_runtime},
    schema::{
        CallToolRequestParams, CallToolResult, ClientCapabilities, CreateTaskResult, GetTaskParams,
        GetTaskPayloadParams, Implementation, InitializeRequestParams, LATEST_PROTOCOL_VERSION,
        LoggingLevel, LoggingMessageNotificationParams, ProgressNotificationParams, RpcError,
        SetLevelRequestParams, TaskMetadata, TaskStatus,
        schema_utils::{ClientJsonrpcRequest, RequestFromClient, ResultFromServer},
    },
    task_store::InMemoryTaskStore,
};

/// Tool body duration for the "long" slot in probe serialization tests.
const PROBE_LONG_MS: u64 = 120;
/// Tool body duration for the "short" slot (issued concurrently with long).
const PROBE_SHORT_MS: u64 = 15;

const PROBE_ENV: &str = "CLAP_MCP_SERIAL_PROBE";

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace")
        .to_path_buf()
}

fn cargo_target_dir() -> std::path::PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"))
}

fn example_binary_path(bin: &str) -> std::path::PathBuf {
    let name = format!("{}{}", bin, std::env::consts::EXE_SUFFIX);
    cargo_target_dir().join("debug").join(name)
}

/// Serializes example builds (cargo build lock) and MCP subprocess tests (avoid parallel stdio races).
static TEST_SERIAL_LOCK: Mutex<()> = Mutex::new(());

fn serial_test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn sleep_args(ms: u64) -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([("ms".into(), serde_json::json!(ms))])
}

fn sleep_probe_args(
    ms: u64,
    label: &str,
    call: &str,
) -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([
        ("ms".into(), serde_json::json!(ms)),
        ("label".into(), serde_json::json!(label)),
        ("call".into(), serde_json::json!(call)),
    ])
}

fn fresh_probe_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "clap_mcp_serial_probe_{}_{}_{tag}.jsonl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

fn reset_probe_file(path: &Path) {
    let _ = fs::remove_file(path);
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ProbeEvent {
    event: String,
    label: String,
    call: String,
    #[serde(rename = "ms")]
    _ms: u64,
    seq: u64,
}

fn read_probe_events(path: &Path) -> Vec<ProbeEvent> {
    let raw = fs::read_to_string(path).unwrap_or_default();
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("invalid probe line {line:?}: {e}"))
        })
        .collect()
}

fn probe_interval(events: &[ProbeEvent], label: &str) -> (u64, u64) {
    let start = events
        .iter()
        .find(|e| e.label == label && e.event == "body_start")
        .unwrap_or_else(|| panic!("missing body_start for label {label}: {events:?}"))
        .seq;
    let end = events
        .iter()
        .find(|e| e.label == label && e.event == "body_end")
        .unwrap_or_else(|| panic!("missing body_end for label {label}: {events:?}"))
        .seq;
    assert!(start < end, "body_start must precede body_end for {label}");
    (start, end)
}

/// Strong serialization check: tool bodies must not overlap (mutex / unified queue).
fn assert_probe_bodies_non_overlapping(events: &[ProbeEvent], label_a: &str, label_b: &str) {
    let (a0, a1) = probe_interval(events, label_a);
    let (b0, b1) = probe_interval(events, label_b);
    let overlaps = a0 < b1 && b0 < a1;
    assert!(
        !overlaps,
        "tool bodies must not overlap (serialized server): {label_a} [{a0},{a1}] vs {label_b} [{b0},{b1}]; events={events:?}"
    );
}

fn assert_probe_expected_calls(events: &[ProbeEvent], long_call: &str, short_call: &str) {
    let by_label: HashMap<_, _> = events
        .iter()
        .filter(|e| e.event == "body_start")
        .map(|e| (e.label.as_str(), e.call.as_str()))
        .collect();
    assert_eq!(by_label.get("long").copied(), Some(long_call));
    assert_eq!(by_label.get("short").copied(), Some(short_call));
}

#[derive(Clone)]
struct NoOpHandler;

#[async_trait]
impl ClientHandler for NoOpHandler {
    async fn handle_logging_message_notification(
        &self,
        _params: LoggingMessageNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_progress_notification(
        &self,
        _params: ProgressNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }
}

#[derive(Clone)]
struct LogCapturingHandler {
    logs: Arc<Mutex<Vec<LoggingMessageNotificationParams>>>,
}

#[async_trait]
impl ClientHandler for LogCapturingHandler {
    async fn handle_logging_message_notification(
        &self,
        params: LoggingMessageNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        self.logs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(params);
        Ok(())
    }

    async fn handle_progress_notification(
        &self,
        _params: ProgressNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }
}

async fn launch_task_example<H>(
    bin: &str,
    handler: H,
    probe_path: Option<&Path>,
) -> SdkResult<Arc<ClientRuntime>>
where
    H: ClientHandler + Send + Sync + 'static,
{
    {
        let status = std::process::Command::new("cargo")
            .args([
                "build",
                "-p",
                "clap-mcp-examples",
                "--bin",
                bin,
                "--features",
                "tracing",
            ])
            .current_dir(workspace_root())
            .status()
            .expect("cargo build");
        assert!(status.success(), "build {bin}");
    }

    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "task-augmented-tests".into(),
            version: "0.1.0".into(),
            title: None,
            description: None,
            icons: vec![],
            website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    let server_task_store: Arc<InMemoryTaskStore<ClientJsonrpcRequest, ResultFromServer>> =
        Arc::new(InMemoryTaskStore::new(None));

    let env = probe_path.map(|p| {
        let mut m = std::collections::HashMap::new();
        m.insert(PROBE_ENV.to_string(), p.to_string_lossy().into_owned());
        m
    });

    let transport = StdioTransport::create_with_server_launch(
        example_binary_path(bin).to_string_lossy().to_string(),
        vec!["--mcp".into()],
        env,
        TransportOptions::default(),
    )?;

    let client = client_runtime::create_client(McpClientOptions {
        client_details,
        transport,
        handler: handler.to_mcp_client_handler(),
        task_store: None,
        server_task_store: Some(server_task_store),
        message_observer: None,
    });
    client.clone().start().await?;
    Ok(client)
}

async fn launch_with_noop(bin: &str) -> SdkResult<Arc<ClientRuntime>> {
    launch_task_example(bin, NoOpHandler, None).await
}

async fn launch_with_probe(bin: &str, probe_path: &Path) -> SdkResult<Arc<ClientRuntime>> {
    launch_task_example(bin, NoOpHandler, Some(probe_path)).await
}

async fn poll_until_completed(client: &Arc<ClientRuntime>, task_id: &str) -> SdkResult<()> {
    let mut poll_ms = 50u64;
    loop {
        let st = client
            .request_get_task(GetTaskParams {
                task_id: task_id.to_string(),
            })
            .await?;
        match st.status {
            TaskStatus::Completed => return Ok(()),
            TaskStatus::Failed | TaskStatus::Cancelled => {
                panic!("unexpected task status {:?}", st.status);
            }
            TaskStatus::Working | TaskStatus::InputRequired => {
                if let Some(p) = st.poll_interval {
                    poll_ms = p.clamp(5, 500) as u64;
                }
                tokio::time::sleep(Duration::from_millis(poll_ms)).await;
            }
        }
    }
}

async fn call_plain(client: &Arc<ClientRuntime>, ms: u64) -> SdkResult<CallToolResult> {
    let r: ResultFromServer = client
        .request(
            RequestFromClient::CallToolRequest(CallToolRequestParams {
                name: "sleep".into(),
                arguments: Some(sleep_args(ms)),
                meta: None,
                task: None,
            }),
            None,
        )
        .await?;
    r.try_into().map_err(Into::into)
}

async fn call_plain_probe(
    client: &Arc<ClientRuntime>,
    ms: u64,
    label: &str,
) -> SdkResult<CallToolResult> {
    let r: ResultFromServer = client
        .request(
            RequestFromClient::CallToolRequest(CallToolRequestParams {
                name: "sleep".into(),
                arguments: Some(sleep_probe_args(ms, label, "plain")),
                meta: None,
                task: None,
            }),
            None,
        )
        .await?;
    r.try_into().map_err(Into::into)
}

async fn call_task_create_probe(
    client: &Arc<ClientRuntime>,
    ms: u64,
    label: &str,
) -> SdkResult<CreateTaskResult> {
    let r: ResultFromServer = client
        .request(
            RequestFromClient::CallToolRequest(CallToolRequestParams {
                name: "sleep".into(),
                arguments: Some(sleep_probe_args(ms, label, "task")),
                meta: None,
                task: Some(TaskMetadata::default()),
            }),
            None,
        )
        .await?;
    r.try_into().map_err(Into::into)
}

async fn call_task_create(client: &Arc<ClientRuntime>, ms: u64) -> SdkResult<CreateTaskResult> {
    let r: ResultFromServer = client
        .request(
            RequestFromClient::CallToolRequest(CallToolRequestParams {
                name: "sleep".into(),
                arguments: Some(sleep_args(ms)),
                meta: None,
                task: Some(TaskMetadata::default()),
            }),
            None,
        )
        .await?;
    r.try_into().map_err(Into::into)
}

fn assert_create_task_meta(create: &CreateTaskResult) {
    let meta = create.meta.as_ref().expect("CreateTaskResult.meta");
    let tid_json = meta.get("taskId").expect("taskId in meta");
    assert_eq!(tid_json.as_str().unwrap(), create.task.task_id);
}

/// Issues long + short tool calls concurrently, then asserts probe ordering proves
/// the two tool bodies never overlapped (strict serialization).
async fn assert_concurrent_probe_serialization(
    client: Arc<ClientRuntime>,
    probe_path: &Path,
    long_is_task: bool,
    short_is_task: bool,
) {
    reset_probe_file(probe_path);

    let path = probe_path.to_path_buf();
    let client_long = client.clone();
    let long_fut = async move {
        if long_is_task {
            let create = call_task_create_probe(&client_long, PROBE_LONG_MS, "long")
                .await
                .expect("long task create");
            poll_until_completed(&client_long, &create.task.task_id)
                .await
                .expect("long task poll");
        } else {
            call_plain_probe(&client_long, PROBE_LONG_MS, "long")
                .await
                .expect("long plain");
        }
    };

    let client_short = client.clone();
    let short_fut = async move {
        if short_is_task {
            let create = call_task_create_probe(&client_short, PROBE_SHORT_MS, "short")
                .await
                .expect("short task create");
            poll_until_completed(&client_short, &create.task.task_id)
                .await
                .expect("short task poll");
        } else {
            call_plain_probe(&client_short, PROBE_SHORT_MS, "short")
                .await
                .expect("short plain");
        }
    };

    let ((), ()) = tokio::join!(long_fut, short_fut);

    let events = read_probe_events(&path);
    assert!(
        events.len() >= 4,
        "expected probe body_start/end for long and short, got {events:?}"
    );
    let long_call = if long_is_task { "task" } else { "plain" };
    let short_call = if short_is_task { "task" } else { "plain" };
    assert_probe_expected_calls(&events, long_call, short_call);
    assert_probe_bodies_non_overlapping(&events, "long", "short");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_augmented_tools_call_dedicated_runtime() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_tools_dedicated")
        .await
        .expect("client");
    let create = call_task_create(&client, 20).await.expect("call");
    assert_create_task_meta(&create);
    poll_until_completed(&client, &create.task.task_id)
        .await
        .expect("poll");
    let _payload = client
        .request_get_task_payload(GetTaskPayloadParams {
            task_id: create.task.task_id,
        })
        .await
        .expect("payload");
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_augmented_tools_call_shared_runtime() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_tools_shared").await.expect("client");
    let create = call_task_create(&client, 20).await.expect("call");
    assert_create_task_meta(&create);
    poll_until_completed(&client, &create.task.task_id)
        .await
        .expect("poll");
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_tool_call_unchanged_dedicated() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_tools_dedicated")
        .await
        .expect("client");
    let t0 = Instant::now();
    let _ = call_plain(&client, 25).await.expect("plain");
    assert!(t0.elapsed() >= Duration::from_millis(15));
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_tool_call_unchanged_shared() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_tools_shared").await.expect("client");
    let _ = call_plain(&client, 25).await.expect("plain");
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_plus_task_concurrent_serializes_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tt_ded");
    let client = launch_with_probe("task_serial_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.clone(), &probe, true, true).await;
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_plus_plain_concurrent_serializes_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pp_ded");
    let client = launch_with_probe("task_serial_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.clone(), &probe, false, false).await;
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_then_task_concurrent_serializes_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pt_ded");
    let client = launch_with_probe("task_serial_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.clone(), &probe, false, true).await;
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_then_plain_concurrent_serializes_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tp_ded");
    let client = launch_with_probe("task_serial_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.clone(), &probe, true, false).await;
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_plus_task_concurrent_serializes_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tt_shr");
    let client = launch_with_probe("task_serial_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.clone(), &probe, true, true).await;
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_then_task_concurrent_serializes_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pt_shr");
    let client = launch_with_probe("task_serial_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.clone(), &probe, false, true).await;
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_then_plain_concurrent_serializes_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tp_shr");
    let client = launch_with_probe("task_serial_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.clone(), &probe, true, false).await;
    client.shut_down().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_augmented_logging_meta_task_id_dedicated() {
    let _serial = serial_test_guard();
    let logs = Arc::new(Mutex::new(Vec::new()));
    let handler = LogCapturingHandler { logs: logs.clone() };
    let client = launch_task_example("task_tools_dedicated", handler, None)
        .await
        .expect("client");

    let _ = client
        .request_set_logging_level(SetLevelRequestParams {
            level: LoggingLevel::Debug,
            meta: None,
        })
        .await;

    let create = call_task_create(&client, 40).await.expect("task call");
    assert_create_task_meta(&create);
    poll_until_completed(&client, &create.task.task_id)
        .await
        .expect("poll");

    let deadline = Instant::now() + Duration::from_secs(2);
    let with_task_id = loop {
        let captured = logs.lock().unwrap_or_else(|e| e.into_inner());
        let matching: Vec<_> = captured
            .iter()
            .filter_map(|p| {
                p.meta
                    .as_ref()
                    .and_then(|m| m.get("taskId"))
                    .and_then(|v| v.as_str())
                    .map(|tid| tid.to_string())
            })
            .collect();
        if !matching.is_empty() {
            break matching;
        }
        if Instant::now() >= deadline {
            panic!(
                "expected logging notification with meta.taskId; got {} messages",
                captured.len()
            );
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert!(
        with_task_id.iter().all(|tid| tid == &create.task.task_id),
        "logging meta.taskId must match CreateTaskResult"
    );

    client.shut_down().await.expect("shutdown");
}
