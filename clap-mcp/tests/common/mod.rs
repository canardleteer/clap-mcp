//! Shared rmcp client helpers for clap-mcp integration tests.

#![allow(dead_code)]

use rmcp::{
    ClientHandler, RoleClient, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, ClientCapabilities, ClientInfo, ClientRequest,
        ContentBlock, CreateTaskResult, GetTaskParams, GetTaskResult, Implementation,
        ReadResourceResult, Request, ResourceContents, ServerResult, TaskPayload, TaskStatus,
    },
    service::Peer,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn cargo_target_dir() -> std::path::PathBuf {
    std::env::var_os("CARGO_LLVM_COV_TARGET_DIR")
        .or_else(|| std::env::var_os("CARGO_TARGET_DIR"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"))
}

pub fn example_binary_path(bin: &str) -> std::path::PathBuf {
    let name = format!("{}{}", bin, std::env::consts::EXE_SUFFIX);
    cargo_target_dir().join("debug").join(name)
}

pub(crate) static BUILD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Default)]
pub struct NoOpHandler;

impl ClientHandler for NoOpHandler {}

/// Test client that declares the SEP-2663 tasks extension.
///
/// Task-eligible tools return `CreateTaskResult` for this client.
#[derive(Clone, Default)]
pub struct TasksClientHandler;

impl ClientHandler for TasksClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::new(
            ClientCapabilities::builder().enable_tasks().build(),
            Implementation::from_build_env(),
        )
    }
}

pub type ExampleClient = rmcp::service::RunningService<RoleClient, NoOpHandler>;
pub type TasksExampleClient = rmcp::service::RunningService<RoleClient, TasksClientHandler>;
pub type ExamplePeer = Peer<RoleClient>;

pub async fn launch_example_with_args(
    bin: &str,
    extra_args: &[&str],
    features: Option<&str>,
) -> Result<ExampleClient, rmcp::RmcpError> {
    launch_example_with_stdio_flag(bin, extra_args, features, "--mcp").await
}

pub async fn launch_example_with_stdio_flag(
    bin: &str,
    extra_args: &[&str],
    features: Option<&str>,
    stdio_flag: &str,
) -> Result<ExampleClient, rmcp::RmcpError> {
    {
        let _guard = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut cmd = std::process::Command::new("cargo");
        cmd.args(["build", "-p", "clap-mcp-examples", "--bin", bin]);
        if let Some(features) = features {
            cmd.args(["--features", features]);
        }
        let status = cmd
            .current_dir(workspace_root())
            .status()
            .expect("cargo build");
        assert!(status.success(), "example binary {bin} should build");
    }

    let transport = TokioChildProcess::new(
        tokio::process::Command::new(example_binary_path(bin)).configure(|cmd| {
            for arg in extra_args {
                cmd.arg(arg);
            }
            cmd.arg(stdio_flag);
        }),
    )
    .map_err(rmcp::RmcpError::transport_creation::<TokioChildProcess>)?;

    NoOpHandler.serve(transport).await.map_err(Into::into)
}

pub async fn launch_example(bin: &str) -> Result<ExampleClient, rmcp::RmcpError> {
    launch_example_with_args(bin, &[], None).await
}

pub fn tool_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn read_text(result: &ReadResourceResult) -> String {
    result
        .contents
        .iter()
        .filter_map(|content| match content {
            ResourceContents::TextResourceContents { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn read_blob(result: &ReadResourceResult) -> Option<(String, Option<String>)> {
    result.contents.iter().find_map(|content| match content {
        ResourceContents::BlobResourceContents {
            blob, mime_type, ..
        } => Some((blob.clone(), mime_type.clone())),
        _ => None,
    })
}

pub fn prompt_has_text(messages: &[rmcp::model::PromptMessage], needle: &str) -> bool {
    messages.iter().any(|message| match &message.content {
        ContentBlock::Text(text) => text.text.contains(needle),
        _ => false,
    })
}

pub async fn shutdown<H: ClientHandler>(client: rmcp::service::RunningService<RoleClient, H>) {
    let _ = client.cancel().await;
}

/// Params for a task-eligible `tools/call` (SEP-2663 is server-directed; no client task hint).
pub fn task_call_params(
    name: impl Into<String>,
    args: serde_json::Map<String, serde_json::Value>,
) -> CallToolRequestParams {
    CallToolRequestParams::new(name.into()).with_arguments(args)
}

pub async fn call_tool_task(
    peer: &ExamplePeer,
    params: CallToolRequestParams,
) -> Result<CreateTaskResult, rmcp::ServiceError> {
    let resp = peer
        .send_request(ClientRequest::CallToolRequest(Request::new(params)))
        .await?;
    match resp {
        ServerResult::CreateTaskResult(result) => Ok(result),
        _ => Err(rmcp::ServiceError::UnexpectedResponse),
    }
}

pub async fn get_task_info(
    peer: &ExamplePeer,
    task_id: &str,
) -> Result<GetTaskResult, rmcp::ServiceError> {
    let resp = peer
        .send_request(ClientRequest::GetTaskRequest(Request::new(
            GetTaskParams::new(task_id),
        )))
        .await?;
    match resp {
        ServerResult::GetTaskResult(result) => Ok(result),
        _ => Err(rmcp::ServiceError::UnexpectedResponse),
    }
}

/// Terminal `tasks/get` payload (`result` object) after the task completes.
pub async fn get_task_payload(
    peer: &ExamplePeer,
    task_id: &str,
) -> Result<serde_json::Value, rmcp::ServiceError> {
    let st = get_task_info(peer, task_id).await?;
    match st.task.payload {
        TaskPayload::Completed { result } => Ok(serde_json::Value::Object(result)),
        other => panic!(
            "expected completed task payload for {task_id}, got status {:?}",
            other.status()
        ),
    }
}

const TASK_POLL_TIMEOUT: Duration = Duration::from_secs(120);

pub async fn poll_until_completed(
    peer: &ExamplePeer,
    task_id: &str,
) -> Result<(), rmcp::ServiceError> {
    poll_until_completed_within(peer, task_id, TASK_POLL_TIMEOUT).await
}

pub async fn poll_until_completed_within(
    peer: &ExamplePeer,
    task_id: &str,
    timeout: Duration,
) -> Result<(), rmcp::ServiceError> {
    let deadline = Instant::now() + timeout;
    let mut poll_ms = 50u64;
    loop {
        let st = get_task_info(peer, task_id).await?;
        match st.task.status() {
            TaskStatus::Completed => return Ok(()),
            TaskStatus::Failed | TaskStatus::Cancelled => {
                panic!("unexpected task status {:?}", st.task.status());
            }
            TaskStatus::Working | TaskStatus::InputRequired => {
                if Instant::now() >= deadline {
                    panic!(
                        "task {task_id} did not complete within {timeout:?} (last status {:?})",
                        st.task.status()
                    );
                }
                if let Some(p) = st.task.task.poll_interval_ms {
                    poll_ms = p.clamp(5, 500);
                }
                tokio::time::sleep(Duration::from_millis(poll_ms)).await;
            }
            _ => panic!("unexpected task status {:?}", st.task.status()),
        }
    }
}

pub fn assert_create_task_meta(create: &CreateTaskResult) {
    assert!(!create.task.task_id.is_empty(), "task id must be set");
}

#[cfg(feature = "http")]
pub struct HttpExampleServer {
    pub child: tokio::process::Child,
}

#[cfg(feature = "http")]
impl HttpExampleServer {
    pub async fn shutdown(mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

#[cfg(feature = "http")]
pub async fn launch_http_example_with_addr(
    bin: &str,
    listen: std::net::SocketAddr,
) -> Result<(ExampleClient, HttpExampleServer), rmcp::RmcpError> {
    use rmcp::transport::StreamableHttpClientTransport;

    {
        let _guard = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut cmd = std::process::Command::new("cargo");
        cmd.args([
            "build",
            "-p",
            "clap-mcp-examples",
            "--bin",
            bin,
            "--features",
            "http",
        ]);
        let status = cmd
            .current_dir(workspace_root())
            .status()
            .expect("cargo build");
        assert!(status.success(), "example binary {bin} should build");
    }

    let listen_str = listen.to_string();
    let child = tokio::process::Command::new(example_binary_path(bin))
        .arg("--mcp-http")
        .arg(&listen_str)
        .spawn()
        .expect("spawn http server");

    let mut connected = false;
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(listen).await.is_ok() {
            connected = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        connected,
        "HTTP example server {bin} not listening on {listen}"
    );

    let uri = format!("http://{listen}/mcp");
    let transport = StreamableHttpClientTransport::from_uri(uri);
    let client = NoOpHandler.serve(transport).await?;
    Ok((client, HttpExampleServer { child }))
}
