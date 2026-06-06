#![cfg(feature = "elicitation")]
mod common;

use common::{BUILD_LOCK, example_binary_path, shutdown, tool_text, workspace_root};
use rmcp::model::{
    CallToolRequestParams, CreateElicitationRequestParams, CreateElicitationResult,
    ElicitationAction,
};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{ClientHandler, ErrorData, RoleClient, ServiceExt, service::RequestContext};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Default)]
struct AcceptElicitationClient {
    last_message: Arc<Mutex<Option<String>>>,
}

impl ClientHandler for AcceptElicitationClient {
    async fn create_elicitation(
        &self,
        request: CreateElicitationRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, ErrorData> {
        let message = match request {
            CreateElicitationRequestParams::FormElicitationParams { message, .. } => message,
            CreateElicitationRequestParams::UrlElicitationParams { message, .. } => message,
        };
        *self.last_message.lock().await = Some(message);
        Ok(CreateElicitationResult {
            meta: None,
            action: ElicitationAction::Accept,
            content: Some(serde_json::json!({"value": "yes"})),
        })
    }
}

#[derive(Clone, Default)]
struct DeclineElicitationClient;

impl ClientHandler for DeclineElicitationClient {
    async fn create_elicitation(
        &self,
        _request: CreateElicitationRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, ErrorData> {
        Ok(CreateElicitationResult {
            meta: None,
            action: ElicitationAction::Decline,
            content: None,
        })
    }
}

#[derive(Clone, Default)]
struct CancelElicitationClient;

impl ClientHandler for CancelElicitationClient {
    async fn create_elicitation(
        &self,
        _request: CreateElicitationRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, ErrorData> {
        Ok(CreateElicitationResult {
            meta: None,
            action: ElicitationAction::Cancel,
            content: None,
        })
    }
}

#[derive(Clone, Default)]
struct AcceptWithoutValueClient;

impl ClientHandler for AcceptWithoutValueClient {
    async fn create_elicitation(
        &self,
        _request: CreateElicitationRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, ErrorData> {
        Ok(CreateElicitationResult {
            meta: None,
            action: ElicitationAction::Accept,
            content: Some(serde_json::json!({"other": "field"})),
        })
    }
}

async fn launch_elicitation_client<H: ClientHandler + Clone + 'static>(
    handler: H,
) -> rmcp::service::RunningService<RoleClient, H> {
    {
        let _guard = BUILD_LOCK.lock().unwrap();
        let status = std::process::Command::new("cargo")
            .args([
                "build",
                "-p",
                "clap-mcp-examples",
                "--bin",
                "elicitation_confirm",
                "--features",
                "elicitation",
            ])
            .current_dir(workspace_root())
            .status()
            .expect("build");
        assert!(status.success());
    }

    let transport = TokioChildProcess::new(
        tokio::process::Command::new(example_binary_path("elicitation_confirm")).configure(|cmd| {
            cmd.arg("--mcp");
        }),
    )
    .map_err(rmcp::RmcpError::transport_creation::<TokioChildProcess>)
    .expect("transport");

    handler.serve(transport).await.expect("serve")
}

#[tokio::test(flavor = "current_thread")]
async fn confirm_echo_elicitation_round_trip() {
    let client_handler = AcceptElicitationClient::default();
    let msg_store = client_handler.last_message.clone();

    let client = launch_elicitation_client(client_handler).await;
    let result = client
        .call_tool(CallToolRequestParams::new("confirm-echo"))
        .await
        .expect("tool");
    assert!(tool_text(&result).contains("confirmed:"));
    assert!(
        msg_store
            .lock()
            .await
            .as_deref()
            .unwrap()
            .contains("confirm-echo")
    );

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn confirm_echo_elicitation_decline_returns_declined_message() {
    let client = launch_elicitation_client(DeclineElicitationClient).await;
    let result = client
        .call_tool(CallToolRequestParams::new("confirm-echo"))
        .await
        .expect("tool");
    assert!(tool_text(&result).contains("elicitation declined"));
    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn confirm_echo_elicitation_cancel_returns_declined_message() {
    let client = launch_elicitation_client(CancelElicitationClient).await;
    let result = client
        .call_tool(CallToolRequestParams::new("confirm-echo"))
        .await
        .expect("tool");
    assert!(tool_text(&result).contains("elicitation declined"));
    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn confirm_echo_elicitation_accept_without_value_uses_default() {
    let client = launch_elicitation_client(AcceptWithoutValueClient).await;
    let result = client
        .call_tool(CallToolRequestParams::new("confirm-echo"))
        .await
        .expect("tool");
    assert!(tool_text(&result).contains("confirmed: accepted"));
    shutdown(client).await;
}
