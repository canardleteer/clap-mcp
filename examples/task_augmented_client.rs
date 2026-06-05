//! Minimal MCP client for task-augmented `tools/call` against the task tool examples.
//!
//! ```text
//! cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_dedicated
//! cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_shared
//! ```

use std::convert::TryInto;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use clap::Parser;
use rust_mcp_sdk::{
    McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
    error::SdkResult,
    mcp_client::{ClientHandler, McpClientOptions, client_runtime},
    schema::{
        CallToolRequestParams, ClientCapabilities, CreateTaskResult, GetTaskParams,
        GetTaskPayloadParams, Implementation, InitializeRequestParams, LATEST_PROTOCOL_VERSION,
        LoggingMessageNotificationParams, ProgressNotificationParams, RpcError, TaskMetadata,
        TaskStatus,
        schema_utils::{ClientJsonrpcRequest, RequestFromClient, ResultFromServer},
    },
    task_store::InMemoryTaskStore,
};

#[derive(Parser)]
struct Args {
    /// Example server binary name: `task_tools_dedicated` or `task_tools_shared`
    #[arg(default_value = "task_tools_dedicated")]
    server_bin: String,
}

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("examples crate in workspace")
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

static BUILD_LOCK: Mutex<()> = Mutex::new(());

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

async fn run_client(bin: &str) -> SdkResult<()> {
    {
        let _guard = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        assert!(status.success(), "example {bin} should build");
    }

    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "task-augmented-client".into(),
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

    let transport = StdioTransport::create_with_server_launch(
        example_binary_path(bin).to_string_lossy().to_string(),
        vec!["--mcp".into()],
        None,
        TransportOptions::default(),
    )?;

    let client = client_runtime::create_client(McpClientOptions {
        client_details,
        transport,
        handler: NoOpHandler.to_mcp_client_handler(),
        task_store: None,
        server_task_store: Some(server_task_store),
        message_observer: None,
    });
    client.clone().start().await?;

    let response: ResultFromServer = client
        .request(
            RequestFromClient::CallToolRequest(CallToolRequestParams {
                name: "sleep".into(),
                arguments: Some(serde_json::Map::from_iter([(
                    "ms".into(),
                    serde_json::json!(25),
                )])),
                meta: None,
                task: Some(TaskMetadata::default()),
            }),
            None,
        )
        .await?;

    let create: CreateTaskResult = response.try_into().map_err(|e: RpcError| e)?;
    let task_id = create.task.task_id.clone();
    if let Some(meta) = create.meta.as_ref() {
        assert!(
            meta.get("taskId").is_some(),
            "CreateTaskResult meta should include taskId"
        );
    }

    let mut poll_ms = create.task.poll_interval.unwrap_or(50).clamp(5, 500) as u64;

    loop {
        let st = client
            .request_get_task(GetTaskParams {
                task_id: task_id.clone(),
            })
            .await?;
        match st.status {
            TaskStatus::Completed => break,
            TaskStatus::Failed | TaskStatus::Cancelled => {
                panic!("task ended with status {:?}", st.status);
            }
            TaskStatus::Working | TaskStatus::InputRequired => {
                if let Some(p) = st.poll_interval {
                    poll_ms = p.clamp(5, 500) as u64;
                }
                tokio::time::sleep(Duration::from_millis(poll_ms)).await;
            }
        }
    }

    let payload = client
        .request_get_task_payload(GetTaskPayloadParams { task_id })
        .await?;
    let text = payload
        .content
        .iter()
        .filter_map(|b| b.as_text_content().ok().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("slept") || text.contains("25"),
        "unexpected task payload: {text:?}"
    );

    client.shut_down().await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    if let Err(e) = run_client(&args.server_bin).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
    println!("ok: task-augmented tools/call completed");
}
