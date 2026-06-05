//! Integration tests for MCP task-augmented `tools/call` (both `share_runtime` configurations).

#![allow(clippy::await_holding_lock)]

mod common;

use common::{
    ExampleClient, ExamplePeer, assert_create_task_meta, call_tool_task, example_binary_path,
    get_task_payload, poll_until_completed, shutdown, task_call_params, tool_text,
};
use rmcp::model::CallToolResult;
use rmcp::{
    ClientHandler, RoleClient, ServiceExt,
    model::{
        CallToolRequestParams, LoggingLevel, LoggingMessageNotificationParam, Meta,
        SetLevelRequestParams,
    },
    service::NotificationContext,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PROBE_LONG_MS: u64 = 120;
const PROBE_SHORT_MS: u64 = 15;
const PROBE_ENV: &str = "CLAP_MCP_SERIAL_PROBE";

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

fn assert_probe_bodies_non_overlapping(events: &[ProbeEvent], label_a: &str, label_b: &str) {
    let (a0, a1) = probe_interval(events, label_a);
    let (b0, b1) = probe_interval(events, label_b);
    let overlaps = a0 < b1 && b0 < a1;
    assert!(
        !overlaps,
        "tool bodies must not overlap: {label_a} [{a0},{a1}] vs {label_b} [{b0},{b1}]; events={events:?}"
    );
}

fn assert_probe_bodies_overlap(events: &[ProbeEvent], label_a: &str, label_b: &str) {
    let (a0, a1) = probe_interval(events, label_a);
    let (b0, b1) = probe_interval(events, label_b);
    let overlaps = a0 < b1 && b0 < a1;
    assert!(
        overlaps,
        "tool bodies must overlap: {label_a} [{a0},{a1}] vs {label_b} [{b0},{b1}]; events={events:?}"
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
struct LogCapturingHandler {
    task_ids: Arc<Mutex<Vec<String>>>,
}

impl ClientHandler for LogCapturingHandler {
    async fn on_logging_message(
        &self,
        _params: LoggingMessageNotificationParam,
        context: NotificationContext<RoleClient>,
    ) {
        if let Some(meta) = context.extensions.get::<Meta>()
            && let Some(tid) = meta.0.get("taskId").and_then(|v| v.as_str())
        {
            self.task_ids
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(tid.to_string());
        }
    }
}

async fn launch_task_example<H>(
    bin: &str,
    handler: H,
    probe_path: Option<&Path>,
) -> Result<rmcp::service::RunningService<RoleClient, H>, rmcp::RmcpError>
where
    H: ClientHandler + Clone + Send + Sync + 'static,
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
            .current_dir(common::workspace_root())
            .status()
            .expect("cargo build");
        assert!(status.success(), "build {bin}");
    }

    let transport = TokioChildProcess::new(
        tokio::process::Command::new(example_binary_path(bin)).configure(|cmd| {
            if let Some(path) = probe_path {
                cmd.env(PROBE_ENV, path);
            }
            cmd.arg("--mcp");
        }),
    )
    .map_err(rmcp::RmcpError::transport_creation::<TokioChildProcess>)?;

    handler.serve(transport).await.map_err(Into::into)
}

async fn launch_with_noop(bin: &str) -> Result<ExampleClient, rmcp::RmcpError> {
    launch_task_example(bin, common::NoOpHandler, None).await
}

async fn launch_with_probe(bin: &str, probe_path: &Path) -> Result<ExampleClient, rmcp::RmcpError> {
    launch_task_example(bin, common::NoOpHandler, Some(probe_path)).await
}

async fn call_plain(
    peer: &ExamplePeer,
    ms: u64,
) -> Result<rmcp::model::CallToolResult, rmcp::ServiceError> {
    peer.call_tool(CallToolRequestParams::new("sleep").with_arguments(sleep_args(ms)))
        .await
}

async fn call_plain_probe(
    peer: &ExamplePeer,
    ms: u64,
    label: &str,
) -> Result<rmcp::model::CallToolResult, rmcp::ServiceError> {
    peer.call_tool(
        CallToolRequestParams::new("sleep").with_arguments(sleep_probe_args(ms, label, "plain")),
    )
    .await
}

async fn call_task_create_probe(
    peer: &ExamplePeer,
    ms: u64,
    label: &str,
) -> Result<rmcp::model::CreateTaskResult, rmcp::ServiceError> {
    call_tool_task(
        peer,
        task_call_params("sleep", sleep_probe_args(ms, label, "task")),
    )
    .await
}

async fn call_task_create(
    peer: &ExamplePeer,
    ms: u64,
) -> Result<rmcp::model::CreateTaskResult, rmcp::ServiceError> {
    call_tool_task(peer, task_call_params("sleep", sleep_args(ms))).await
}

async fn assert_concurrent_probe_serialization(
    peer: ExamplePeer,
    probe_path: &Path,
    long_is_task: bool,
    short_is_task: bool,
) {
    reset_probe_file(probe_path);

    let path = probe_path.to_path_buf();
    let client_long = peer.clone();
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

    let client_short = peer.clone();
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

async fn assert_concurrent_probe_overlap(
    peer: ExamplePeer,
    probe_path: &Path,
    long_is_task: bool,
    short_is_task: bool,
) {
    reset_probe_file(probe_path);

    let path = probe_path.to_path_buf();
    let client_long = peer.clone();
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

    let client_short = peer.clone();
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
    assert_probe_bodies_overlap(&events, "long", "short");
}

async fn task_payload_as_call_tool_result(peer: &ExamplePeer, task_id: &str) -> CallToolResult {
    let payload = get_task_payload(peer, task_id).await.expect("task payload");
    serde_json::from_value(payload.0).expect("task payload should be CallToolResult")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_augmented_tools_call_dedicated_runtime() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_tools_dedicated")
        .await
        .expect("client");
    let peer = client.peer();
    let create = call_task_create(peer, 20).await.expect("call");
    assert_create_task_meta(&create);
    poll_until_completed(peer, &create.task.task_id)
        .await
        .expect("poll");
    let _payload = get_task_payload(peer, &create.task.task_id)
        .await
        .expect("payload");
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_augmented_tools_call_shared_runtime() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_tools_shared").await.expect("client");
    let peer = client.peer();
    let create = call_task_create(peer, 20).await.expect("call");
    assert_create_task_meta(&create);
    poll_until_completed(peer, &create.task.task_id)
        .await
        .expect("poll");
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_tool_call_unchanged_dedicated() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_tools_dedicated")
        .await
        .expect("client");
    let t0 = Instant::now();
    let _ = call_plain(client.peer(), 25).await.expect("plain");
    assert!(t0.elapsed() >= Duration::from_millis(15));
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_tool_call_unchanged_shared() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_tools_shared").await.expect("client");
    let _ = call_plain(client.peer(), 25).await.expect("plain");
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_plus_task_concurrent_serializes_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tt_ded");
    let client = launch_with_probe("task_serial_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.peer().clone(), &probe, true, true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_plus_plain_concurrent_serializes_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pp_ded");
    let client = launch_with_probe("task_serial_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.peer().clone(), &probe, false, false).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_then_task_concurrent_serializes_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pt_ded");
    let client = launch_with_probe("task_serial_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.peer().clone(), &probe, false, true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_then_plain_concurrent_serializes_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tp_ded");
    let client = launch_with_probe("task_serial_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.peer().clone(), &probe, true, false).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_plus_task_concurrent_serializes_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tt_shr");
    let client = launch_with_probe("task_serial_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.peer().clone(), &probe, true, true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_then_task_concurrent_serializes_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pt_shr");
    let client = launch_with_probe("task_serial_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.peer().clone(), &probe, false, true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_then_plain_concurrent_serializes_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tp_shr");
    let client = launch_with_probe("task_serial_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_serialization(client.peer().clone(), &probe, true, false).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_augmented_logging_meta_task_id_dedicated() {
    let _serial = serial_test_guard();
    let task_ids = Arc::new(Mutex::new(Vec::new()));
    let handler = LogCapturingHandler {
        task_ids: task_ids.clone(),
    };
    let client = launch_task_example("task_tools_dedicated", handler, None)
        .await
        .expect("client");

    let _ = client
        .set_level(SetLevelRequestParams::new(LoggingLevel::Debug))
        .await;

    let peer = client.peer();
    let create = call_task_create(peer, 40).await.expect("task call");
    assert_create_task_meta(&create);
    poll_until_completed(peer, &create.task.task_id)
        .await
        .expect("poll");

    let deadline = Instant::now() + Duration::from_secs(2);
    let with_task_id = loop {
        let captured = task_ids.lock().unwrap_or_else(|e| e.into_inner());
        if !captured.is_empty() {
            break captured.clone();
        }
        drop(captured);
        if Instant::now() >= deadline {
            panic!("expected logging notification with meta.taskId");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert!(
        with_task_id.iter().all(|tid| tid == &create.task.task_id),
        "logging meta.taskId must match CreateTaskResult"
    );

    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_plus_task_concurrent_overlaps_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tt_par_ded");
    let client = launch_with_probe("task_parallel_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_overlap(client.peer().clone(), &probe, true, true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_plus_plain_concurrent_overlaps_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pp_par_ded");
    let client = launch_with_probe("task_parallel_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_overlap(client.peer().clone(), &probe, false, false).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_then_task_concurrent_overlaps_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pt_par_ded");
    let client = launch_with_probe("task_parallel_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_overlap(client.peer().clone(), &probe, false, true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_then_plain_concurrent_overlaps_dedicated_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tp_par_ded");
    let client = launch_with_probe("task_parallel_probe_dedicated", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_overlap(client.peer().clone(), &probe, true, false).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_plus_task_concurrent_overlaps_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tt_par_shr");
    let client = launch_with_probe("task_parallel_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_overlap(client.peer().clone(), &probe, true, true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_then_task_concurrent_overlaps_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("pt_par_shr");
    let client = launch_with_probe("task_parallel_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_overlap(client.peer().clone(), &probe, false, true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_then_plain_concurrent_overlaps_shared_probe() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("tp_par_shr");
    let client = launch_with_probe("task_parallel_probe_shared", &probe)
        .await
        .expect("client");
    assert_concurrent_probe_overlap(client.peer().clone(), &probe, true, false).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_task_logging_distinct_task_ids_parallel_probe() {
    let _serial = serial_test_guard();
    let task_ids = Arc::new(Mutex::new(Vec::new()));
    let handler = LogCapturingHandler {
        task_ids: task_ids.clone(),
    };
    let client = launch_task_example("task_parallel_probe_shared", handler, None)
        .await
        .expect("client");

    let _ = client
        .set_level(SetLevelRequestParams::new(LoggingLevel::Debug))
        .await;

    let peer = client.peer();
    let create_a = call_task_create_probe(peer, PROBE_LONG_MS, "a")
        .await
        .expect("task a");
    let create_b = call_task_create_probe(peer, PROBE_SHORT_MS, "b")
        .await
        .expect("task b");
    let peer_a = peer.clone();
    let peer_b = peer.clone();
    let poll_a_id = create_a.task.task_id.clone();
    let poll_b_id = create_b.task.task_id.clone();
    let poll_a = async move {
        poll_until_completed(&peer_a, &poll_a_id)
            .await
            .expect("poll a");
    };
    let poll_b = async move {
        poll_until_completed(&peer_b, &poll_b_id)
            .await
            .expect("poll b");
    };
    let ((), ()) = tokio::join!(poll_a, poll_b);

    let deadline = Instant::now() + Duration::from_secs(3);
    let captured = loop {
        let captured = task_ids.lock().unwrap_or_else(|e| e.into_inner());
        if captured.len() >= 2 {
            break captured.clone();
        }
        drop(captured);
        if Instant::now() >= deadline {
            panic!("expected logging notifications with meta.taskId for both tasks");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    };

    assert!(
        captured.iter().any(|tid| tid == &create_a.task.task_id),
        "expected logs for task a ({}), got {captured:?}",
        create_a.task.task_id
    );
    assert!(
        captured.iter().any(|tid| tid == &create_b.task.task_id),
        "expected logs for task b ({}), got {captured:?}",
        create_b.task.task_id
    );

    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_augmented_panic_caught_returns_error_payload() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_panic_catch").await.expect("client");
    let peer = client.peer();
    let create = call_tool_task(peer, task_call_params("panic-demo", serde_json::Map::new()))
        .await
        .expect("task create");
    assert_create_task_meta(&create);
    poll_until_completed(peer, &create.task.task_id)
        .await
        .expect("poll");
    let result = task_payload_as_call_tool_result(peer, &create.task.task_id).await;
    assert_eq!(result.is_error, Some(true));
    let text = tool_text(&result);
    assert!(
        text.contains("panicked") || text.contains("panic"),
        "got: {text}"
    );

    let create_ok = call_task_create(peer, 10).await.expect("non-panic task");
    poll_until_completed(peer, &create_ok.task.task_id)
        .await
        .expect("poll ok");
    let ok_result = task_payload_as_call_tool_result(peer, &create_ok.task.task_id).await;
    assert_ne!(ok_result.is_error, Some(true));

    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_augmented_panic_caught_parallel_safe() {
    let _serial = serial_test_guard();
    let client = launch_with_noop("task_panic_catch_parallel")
        .await
        .expect("client");
    let peer = client.peer();
    let create = call_tool_task(peer, task_call_params("panic-demo", serde_json::Map::new()))
        .await
        .expect("task create");
    poll_until_completed(peer, &create.task.task_id)
        .await
        .expect("poll");
    let result = task_payload_as_call_tool_result(peer, &create.task.task_id).await;
    assert_eq!(result.is_error, Some(true));
    shutdown(client).await;
}
