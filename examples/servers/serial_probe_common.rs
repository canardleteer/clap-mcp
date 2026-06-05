//! Append-only execution probe for serialization integration tests.
//!
//! When `CLAP_MCP_SERIAL_PROBE` is set to a file path, each tool body records
//! `body_start` / `body_end` lines with monotonic `seq` for strict ordering checks.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static PROBE_SEQ: AtomicU64 = AtomicU64::new(0);
static PROBE_FILE: Mutex<Option<String>> = Mutex::new(None);

fn probe_path() -> Option<String> {
    let mut slot = PROBE_FILE.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = std::env::var("CLAP_MCP_SERIAL_PROBE").ok();
    }
    slot.clone()
}

fn append(event: &str, label: &str, call: &str, ms: u64) {
    let Some(path) = probe_path() else {
        return;
    };
    let seq = PROBE_SEQ.fetch_add(1, Ordering::SeqCst);
    let line = serde_json::json!({
        "event": event,
        "label": label,
        "call": call,
        "ms": ms,
        "seq": seq,
    });
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Records probe events around an async sleep (tool body).
pub async fn sleep_with_probe(label: &str, call: &str, ms: u64) -> String {
    append("body_start", label, call, ms);
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    append("body_end", label, call, ms);
    format!("slept {ms}ms as {label} ({call})")
}
