//! Integration tests for crash and panic handling: subprocess non-zero exit and
//! in-process panic catching.

use rust_mcp_sdk::{
    McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
    error::SdkResult,
    mcp_client::{ClientHandler, McpClientOptions, client_runtime},
    schema::{
        CallToolRequestParams, ClientCapabilities, Implementation, InitializeRequestParams,
        LATEST_PROTOCOL_VERSION, LoggingMessageNotificationParams, ProgressNotificationParams,
        RpcError,
    },
};
use std::path::Path;
use std::sync::Mutex;

/// Workspace root (parent of clap-mcp). Required so that we can find the built example binaries.
fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Path to the built example binary (built in workspace target dir).
/// Uses platform executable suffix (e.g. `.exe` on Windows).
fn example_binary_path(bin: &str) -> std::path::PathBuf {
    let name = format!("{}{}", bin, std::env::consts::EXE_SUFFIX);
    workspace_root().join("target").join("debug").join(name)
}

/// Serializes tests that launch the server so they don't run in parallel (avoids port/cwd issues).
static LAUNCH_LOCK: Mutex<()> = Mutex::new(());

/// Ensure the example binary is built, then run it with --mcp and return the tool call result.
async fn launch_and_call_tool(
    bin: &str,
    tool_name: &str,
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
) -> SdkResult<rust_mcp_sdk::schema::CallToolResult> {
    let client = {
        let _guard = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Always build so we pick up latest library and example code.
        let status = std::process::Command::new("cargo")
            .args(["build", "-p", "clap-mcp-examples", "--bin", bin])
            .current_dir(workspace_root())
            .status()
            .expect("cargo build for example should run");
        assert!(
            status.success(),
            "cargo build -p clap-mcp-examples --bin {} must succeed",
            bin
        );

        let path = example_binary_path(bin);
        assert!(path.exists(), "example binary must exist at {:?}", path);

        let client_details = InitializeRequestParams {
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "crash-panic-test".into(),
                version: "0.1.0".into(),
                title: None,
                description: None,
                icons: vec![],
                website_url: None,
            },
            protocol_version: LATEST_PROTOCOL_VERSION.into(),
            meta: None,
        };

        let transport = StdioTransport::create_with_server_launch(
            path.to_string_lossy().to_string(),
            vec!["succeed".into(), "--mcp".into()],
            None,
            TransportOptions::default(),
        )?;

        client_runtime::create_client(McpClientOptions {
            client_details,
            transport,
            handler: NoOpHandler.to_mcp_client_handler(),
            task_store: None,
            server_task_store: None,
        })
    };

    client.clone().start().await?;

    let result = client
        .request_tool_call(CallToolRequestParams {
            name: tool_name.into(),
            arguments: arguments.or_else(|| Some(serde_json::Map::new())),
            meta: None,
            task: None,
        })
        .await?;

    client.shut_down().await?;
    Ok(result)
}

#[derive(Clone)]
struct NoOpHandler;

#[async_trait::async_trait]
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

#[tokio::test]
async fn test_subprocess_nonzero_exit_returns_error() {
    let result = launch_and_call_tool("subprocess_exit_handling", "exit-fail", None)
        .await
        .expect("launch and call should succeed");

    assert_eq!(
        result.is_error,
        Some(true),
        "subprocess exit non-zero should return is_error: true"
    );
    let text: String = result
        .content
        .iter()
        .filter_map(|b| b.as_text_content().ok().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("");
    assert!(
        text.contains("exited with non-zero status"),
        "content should mention non-zero exit; got: {:?}",
        text
    );
    assert!(
        text.contains("code: 1") || text.contains("code:1"),
        "content should include exit code; got: {:?}",
        text
    );
}

#[tokio::test]
async fn test_in_process_panic_caught_returns_error() {
    let result = launch_and_call_tool("panic_catch_opt_in", "panic-demo", None)
        .await
        .expect("launch and call should succeed");

    assert_eq!(
        result.is_error,
        Some(true),
        "caught panic should return is_error: true"
    );
    let text: String = result
        .content
        .iter()
        .filter_map(|b| b.as_text_content().ok().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("");
    assert!(
        text.contains("panicked") || text.contains("panic"),
        "content should mention panic; got: {:?}",
        text
    );
}
