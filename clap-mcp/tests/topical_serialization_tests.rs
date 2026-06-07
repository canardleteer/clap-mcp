//! Integration tests for topical serialization (`parallel_safe = true` + `#[clap_mcp(serialized)]`).

#![allow(clippy::await_holding_lock)]

mod common;

use common::{ExamplePeer, example_binary_path, shutdown};
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const PROBE_LONG_MS: u64 = 120;
const PROBE_SHORT_MS: u64 = 15;
const PROBE_ENV: &str = "CLAP_MCP_SERIAL_PROBE";

static TEST_SERIAL_LOCK: Mutex<()> = Mutex::new(());

fn serial_test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn fresh_probe_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "clap_mcp_topical_probe_{}_{}_{tag}.jsonl",
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
    #[allow(dead_code)]
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

async fn launch_with_probe(probe_path: &Path) -> Result<common::ExampleClient, rmcp::RmcpError> {
    {
        let status = std::process::Command::new("cargo")
            .args([
                "build",
                "-p",
                "clap-mcp-examples",
                "--bin",
                "topical_serial_probe",
                "--features",
                "tracing",
            ])
            .current_dir(common::workspace_root())
            .status()
            .expect("cargo build");
        assert!(status.success(), "build topical_serial_probe");
    }

    let transport = TokioChildProcess::new(
        tokio::process::Command::new(example_binary_path("topical_serial_probe")).configure(
            |cmd| {
                cmd.env(PROBE_ENV, probe_path);
                cmd.arg("--mcp");
            },
        ),
    )
    .map_err(rmcp::RmcpError::transport_creation::<TokioChildProcess>)?;

    common::NoOpHandler
        .serve(transport)
        .await
        .map_err(Into::into)
}

fn flush_args(output: &str, ms: u64, label: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([
        ("output".into(), serde_json::json!(output)),
        ("ms".into(), serde_json::json!(ms)),
        ("label".into(), serde_json::json!(label)),
    ])
}

fn search_args(ms: u64, label: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([
        ("ms".into(), serde_json::json!(ms)),
        ("label".into(), serde_json::json!(label)),
    ])
}

async fn call_flush(
    peer: &ExamplePeer,
    output: &str,
    ms: u64,
    label: &str,
) -> Result<rmcp::model::CallToolResult, rmcp::ServiceError> {
    peer.call_tool(
        CallToolRequestParams::new("flush").with_arguments(flush_args(output, ms, label)),
    )
    .await
}

async fn call_search(
    peer: &ExamplePeer,
    ms: u64,
    label: &str,
) -> Result<rmcp::model::CallToolResult, rmcp::ServiceError> {
    peer.call_tool(CallToolRequestParams::new("search").with_arguments(search_args(ms, label)))
        .await
}

async fn assert_concurrent_flush_probe(
    peer: ExamplePeer,
    probe_path: &Path,
    output_a: &str,
    output_b: &str,
    expect_overlap: bool,
) {
    reset_probe_file(probe_path);
    let path = probe_path.to_path_buf();
    let peer_long = peer.clone();
    let out_a = output_a.to_string();
    let long_fut = async move {
        call_flush(&peer_long, &out_a, PROBE_LONG_MS, "long")
            .await
            .expect("long flush")
    };
    let peer_short = peer.clone();
    let out_b = output_b.to_string();
    let short_fut = async move {
        tokio::time::sleep(Duration::from_millis(5)).await;
        call_flush(&peer_short, &out_b, PROBE_SHORT_MS, "short")
            .await
            .expect("short flush")
    };
    tokio::join!(long_fut, short_fut);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let events = read_probe_events(&path);
    if expect_overlap {
        assert_probe_bodies_overlap(&events, "long", "short");
    } else {
        assert_probe_bodies_non_overlapping(&events, "long", "short");
    }
}

async fn assert_concurrent_search_probe(peer: ExamplePeer, probe_path: &Path) {
    reset_probe_file(probe_path);
    let path = probe_path.to_path_buf();
    let peer_long = peer.clone();
    let long_fut = async move {
        call_search(&peer_long, PROBE_LONG_MS, "long")
            .await
            .expect("long search")
    };
    let peer_short = peer.clone();
    let short_fut = async move {
        tokio::time::sleep(Duration::from_millis(5)).await;
        call_search(&peer_short, PROBE_SHORT_MS, "short")
            .await
            .expect("short search")
    };
    tokio::join!(long_fut, short_fut);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let events = read_probe_events(&path);
    assert_probe_bodies_overlap(&events, "long", "short");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn topical_same_output_serializes() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("same_out");
    let client = launch_with_probe(&probe).await.expect("client");
    assert_concurrent_flush_probe(client.peer().clone(), &probe, "a", "a", false).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn topical_different_output_overlaps() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("diff_out");
    let client = launch_with_probe(&probe).await.expect("client");
    assert_concurrent_flush_probe(client.peer().clone(), &probe, "a", "b", true).await;
    shutdown(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn topical_unmarked_search_overlaps() {
    let _serial = serial_test_guard();
    let probe = fresh_probe_path("search");
    let client = launch_with_probe(&probe).await.expect("client");
    assert_concurrent_search_probe(client.peer().clone(), &probe).await;
    shutdown(client).await;
}
