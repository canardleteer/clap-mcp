//! Minimal MCP client for task-augmented `tools/call` against the task tool examples.
//!
//! ```text
//! cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_dedicated
//! cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_shared
//! ```

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use clap::Parser;
use rmcp::{
    ClientHandler, RoleClient, ServiceExt,
    model::{
        CallToolRequestParams, ClientRequest, CreateTaskResult, GetTaskInfoParams,
        GetTaskResultParams, Request, ServerResult, TaskStatus,
    },
    service::Peer,
    transport::{ConfigureCommandExt, TokioChildProcess},
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

#[derive(Clone, Default)]
struct NoOpHandler;

impl ClientHandler for NoOpHandler {}

async fn call_tool_task(
    peer: &Peer<RoleClient>,
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

async fn get_task_info(
    peer: &Peer<RoleClient>,
    task_id: &str,
) -> Result<rmcp::model::GetTaskResult, rmcp::ServiceError> {
    let resp = peer
        .send_request(ClientRequest::GetTaskInfoRequest(Request::new(
            GetTaskInfoParams {
                meta: None,
                task_id: task_id.to_string(),
            },
        )))
        .await?;
    match resp {
        ServerResult::GetTaskResult(result) => Ok(result),
        _ => Err(rmcp::ServiceError::UnexpectedResponse),
    }
}

async fn get_task_payload(
    peer: &Peer<RoleClient>,
    task_id: &str,
) -> Result<rmcp::model::GetTaskPayloadResult, rmcp::ServiceError> {
    let resp = peer
        .send_request(ClientRequest::GetTaskResultRequest(Request::new(
            GetTaskResultParams {
                meta: None,
                task_id: task_id.to_string(),
            },
        )))
        .await?;
    match resp {
        ServerResult::GetTaskPayloadResult(result) => Ok(result),
        ServerResult::CustomResult(value) => Ok(rmcp::model::GetTaskPayloadResult::new(value.0)),
        ServerResult::CallToolResult(result) => Ok(rmcp::model::GetTaskPayloadResult::new(
            serde_json::to_value(result).expect("task payload should serialize"),
        )),
        _ => Err(rmcp::ServiceError::UnexpectedResponse),
    }
}

async fn run_client(bin: &str) -> Result<(), rmcp::RmcpError> {
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

    let transport = TokioChildProcess::new(
        tokio::process::Command::new(example_binary_path(bin)).configure(|cmd| {
            cmd.arg("--mcp");
        }),
    )
    .map_err(rmcp::RmcpError::transport_creation::<TokioChildProcess>)?;

    let client = NoOpHandler.serve(transport).await?;

    let peer = client.peer();
    let create = call_tool_task(
        peer,
        CallToolRequestParams::new("sleep")
            .with_arguments(serde_json::Map::from_iter([(
                "ms".into(),
                serde_json::json!(25),
            )]))
            .with_task(rmcp::object!({})),
    )
    .await?;

    let task_id = create.task.task_id.clone();
    assert!(!task_id.is_empty(), "task id must be set");

    let mut poll_ms = create.task.poll_interval.unwrap_or(50).clamp(5, 500);

    loop {
        let st = get_task_info(peer, &task_id).await?;
        match st.task.status {
            TaskStatus::Completed => break,
            TaskStatus::Failed | TaskStatus::Cancelled => {
                panic!("task ended with status {:?}", st.task.status);
            }
            TaskStatus::Working | TaskStatus::InputRequired => {
                if let Some(p) = st.task.poll_interval {
                    poll_ms = p.clamp(5, 500);
                }
                tokio::time::sleep(Duration::from_millis(poll_ms)).await;
            }
        }
    }

    let payload = get_task_payload(peer, &task_id).await?;
    let tool_result: rmcp::model::CallToolResult =
        serde_json::from_value(payload.0).expect("task payload should be CallToolResult");
    let text = tool_result
        .content
        .iter()
        .filter_map(|block| match &block.raw {
            rmcp::model::RawContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("slept") || text.contains("25"),
        "unexpected task payload: {text:?}"
    );

    let _ = client.cancel().await;
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
