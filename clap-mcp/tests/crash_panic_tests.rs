//! Integration tests for crash and panic handling: subprocess non-zero exit and
//! in-process panic catching.

#![allow(clippy::await_holding_lock)]

mod common;

use common::{launch_example_with_args, shutdown, tool_text};
use rmcp::model::CallToolRequestParams;
use std::sync::Mutex;

static LAUNCH_LOCK: Mutex<()> = Mutex::new(());

async fn launch_and_call_tool(
    bin: &str,
    tool_name: &str,
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<rmcp::model::CallToolResult, rmcp::RmcpError> {
    let _guard = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let client = launch_example_with_args(bin, &["succeed"], None).await?;
    let params = if let Some(args) = arguments {
        CallToolRequestParams::new(tool_name.to_string()).with_arguments(args)
    } else {
        CallToolRequestParams::new(tool_name.to_string()).with_arguments(serde_json::Map::new())
    };
    let result = client
        .call_tool(params)
        .await
        .map_err(rmcp::RmcpError::from)?;
    shutdown(client).await;
    Ok(result)
}

#[tokio::test]
async fn test_subprocess_nonzero_exit_returns_error() {
    let result = launch_and_call_tool("subprocess_exit_handling", "exit-fail", None)
        .await
        .expect("launch and call should succeed");

    assert_eq!(result.is_error, Some(true));
    let text = tool_text(&result);
    assert!(text.contains("exited with non-zero status"), "got: {text}");
    assert!(
        text.contains("code: 1") || text.contains("code:1"),
        "got: {text}"
    );
}

#[tokio::test]
async fn test_in_process_panic_caught_returns_error() {
    let result = launch_and_call_tool("panic_catch_opt_in", "panic-demo", None)
        .await
        .expect("launch and call should succeed");

    assert_eq!(result.is_error, Some(true));
    let text = tool_text(&result);
    assert!(
        text.contains("panicked") || text.contains("panic"),
        "got: {text}"
    );
}
