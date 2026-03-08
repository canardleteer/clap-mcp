use async_trait::async_trait;
use rust_mcp_sdk::{
    McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
    error::SdkResult,
    mcp_client::{ClientHandler, ClientRuntime, McpClientOptions, client_runtime},
    schema::{
        CallToolRequestParams, ClientCapabilities, GetPromptRequestParams, Implementation,
        InitializeRequestParams, LATEST_PROTOCOL_VERSION, LoggingMessageNotificationParams,
        ProgressNotificationParams, ReadResourceContent, ReadResourceRequestParams, RpcError,
    },
};
use std::path::Path;
use std::sync::{Arc, Mutex};

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cargo_target_dir() -> std::path::PathBuf {
    std::env::var_os("CARGO_LLVM_COV_TARGET_DIR")
        .or_else(|| std::env::var_os("CARGO_TARGET_DIR"))
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

async fn launch_example(bin: &str) -> SdkResult<Arc<ClientRuntime>> {
    {
        let _guard = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let status = std::process::Command::new("cargo")
            .args(["build", "-p", "clap-mcp-examples", "--bin", bin])
            .current_dir(workspace_root())
            .status()
            .expect("cargo build should run");
        assert!(status.success(), "example binary {bin} should build");
    }

    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "server-behavior-tests".into(),
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
        server_task_store: None,
    });
    client.clone().start().await?;
    Ok(client)
}

fn tool_text(result: &rust_mcp_sdk::schema::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text_content().ok().map(|text| text.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_text(result: &rust_mcp_sdk::schema::ReadResourceResult) -> String {
    result
        .contents
        .iter()
        .filter_map(|content| match content {
            ReadResourceContent::TextResourceContents(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test(flavor = "current_thread")]
async fn custom_resources_and_prompts_round_trip() {
    let client = launch_example("custom_resources_prompts")
        .await
        .expect("client should launch");

    let resources = client
        .request_resource_list(None)
        .await
        .expect("resource list should work")
        .resources;
    assert!(
        resources
            .iter()
            .any(|resource| resource.uri == "clap://schema")
    );
    assert!(
        resources
            .iter()
            .any(|resource| resource.uri == "example://readme")
    );

    let readme = client
        .request_resource_read(ReadResourceRequestParams {
            uri: "example://readme".into(),
            meta: None,
        })
        .await
        .expect("custom resource should be readable");
    assert!(read_text(&readme).contains("Custom resources & prompts example"));

    let schema = client
        .request_resource_read(ReadResourceRequestParams {
            uri: "clap://schema".into(),
            meta: None,
        })
        .await
        .expect("schema resource should be readable");
    assert!(read_text(&schema).contains("\"name\": \"custom-resources-prompts\""));

    let prompts = client
        .request_prompt_list(None)
        .await
        .expect("prompt list should work")
        .prompts;
    assert!(prompts.iter().any(|prompt| prompt.name == "example-prompt"));

    let prompt = client
        .request_prompt(GetPromptRequestParams {
            name: "example-prompt".into(),
            arguments: None,
            meta: None,
        })
        .await
        .expect("custom prompt should resolve");
    assert!(prompt.messages.iter().any(|message| {
        message
            .content
            .as_text_content()
            .ok()
            .is_some_and(|text| text.text.contains("prefer the echo tool"))
    }));

    client.shut_down().await.expect("shutdown should succeed");
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_resources_prompts_tools_and_arguments_return_errors() {
    let client = launch_example("custom_resources_prompts")
        .await
        .expect("client should launch");

    let resource_error = client
        .request_resource_read(ReadResourceRequestParams {
            uri: "example://missing".into(),
            meta: None,
        })
        .await
        .expect_err("unknown resource should error");
    assert!(format!("{resource_error:?}").contains("unknown resource uri"));

    let prompt_error = client
        .request_prompt(GetPromptRequestParams {
            name: "missing-prompt".into(),
            arguments: None,
            meta: None,
        })
        .await
        .expect_err("unknown prompt should error");
    assert!(format!("{prompt_error:?}").contains("unknown prompt"));

    client.shut_down().await.expect("shutdown should succeed");

    let client = launch_example("subcommands")
        .await
        .expect("subcommands client should launch");

    let tool_error = client
        .request_tool_call(CallToolRequestParams {
            name: "missing-tool".into(),
            arguments: Some(serde_json::Map::new()),
            meta: None,
            task: None,
        })
        .await
        .expect("unknown tool should return a tool error payload");
    assert_eq!(tool_error.is_error, Some(true));
    assert!(tool_text(&tool_error).contains("Unknown tool: missing-tool"));

    let invalid_arg_error = client
        .request_tool_call(CallToolRequestParams {
            name: "greet".into(),
            arguments: Some(serde_json::Map::from_iter([(
                "bogus".to_string(),
                serde_json::json!("value"),
            )])),
            meta: None,
            task: None,
        })
        .await
        .expect("unknown argument should return a tool error payload");
    assert_eq!(invalid_arg_error.is_error, Some(true));
    assert!(tool_text(&invalid_arg_error).contains("unknown argument: bogus"));

    client.shut_down().await.expect("shutdown should succeed");
}

#[tokio::test(flavor = "current_thread")]
async fn in_process_outputs_and_required_args_are_preserved() {
    let client = launch_example("structured")
        .await
        .expect("structured client should launch");
    let structured = client
        .request_tool_call(CallToolRequestParams {
            name: "add".into(),
            arguments: Some(serde_json::Map::from_iter([
                ("a".to_string(), serde_json::json!(2)),
                ("b".to_string(), serde_json::json!(3)),
            ])),
            meta: None,
            task: None,
        })
        .await
        .expect("structured call should succeed");
    assert_eq!(structured.is_error, None);
    assert_eq!(
        structured
            .structured_content
            .as_ref()
            .and_then(|content| content.get("sum"))
            .and_then(|value| value.as_i64()),
        Some(5)
    );
    assert!(tool_text(&structured).contains("\"sum\": 5"));
    client.shut_down().await.expect("shutdown should succeed");

    let client = launch_example("result_output")
        .await
        .expect("result-output client should launch");
    let structured_error = client
        .request_tool_call(CallToolRequestParams {
            name: "check".into(),
            arguments: Some(serde_json::Map::from_iter([(
                "x".to_string(),
                serde_json::json!(0),
            )])),
            meta: None,
            task: None,
        })
        .await
        .expect("structured error should still be a tool response");
    assert_eq!(structured_error.is_error, Some(true));
    assert_eq!(
        structured_error
            .structured_content
            .as_ref()
            .and_then(|content| content.get("code"))
            .and_then(|value| value.as_i64()),
        Some(-1)
    );
    client.shut_down().await.expect("shutdown should succeed");

    let client = launch_example("optional_commands_and_args")
        .await
        .expect("optional args client should launch");
    let missing_arg = client
        .request_tool_call(CallToolRequestParams {
            name: "read".into(),
            arguments: Some(serde_json::Map::new()),
            meta: None,
            task: None,
        })
        .await
        .expect("missing required arg should be returned as tool result");
    assert_eq!(missing_arg.is_error, Some(true));
    assert!(tool_text(&missing_arg).contains("Missing required argument(s): path"));
    client.shut_down().await.expect("shutdown should succeed");
}

#[tokio::test(flavor = "current_thread")]
async fn subprocess_and_direct_server_paths_return_expected_text() {
    let client = launch_example("stderr_success")
        .await
        .expect("stderr success client should launch");
    let success = client
        .request_tool_call(CallToolRequestParams {
            name: "succeed-with-stderr".into(),
            arguments: Some(serde_json::Map::new()),
            meta: None,
            task: None,
        })
        .await
        .expect("successful subprocess call should succeed");
    let success_text = tool_text(&success);
    assert!(success_text.contains("stdout ok"));
    assert!(success_text.contains("stderr:\nstderr note"));
    client.shut_down().await.expect("shutdown should succeed");

    let client = launch_example("subprocess_exit_handling")
        .await
        .expect("subprocess exit client should launch");
    let failure = client
        .request_tool_call(CallToolRequestParams {
            name: "exit-fail".into(),
            arguments: Some(serde_json::Map::new()),
            meta: None,
            task: None,
        })
        .await
        .expect("non-zero exit should still yield a tool result");
    assert_eq!(failure.is_error, Some(true));
    assert!(tool_text(&failure).contains("exited with non-zero status"));
    client.shut_down().await.expect("shutdown should succeed");

    let client = launch_example("placeholder_server")
        .await
        .expect("placeholder server client should launch");
    let placeholder = client
        .request_tool_call(CallToolRequestParams {
            name: "echo".into(),
            arguments: Some(serde_json::Map::from_iter([(
                "message".to_string(),
                serde_json::json!("hi"),
            )])),
            meta: None,
            task: None,
        })
        .await
        .expect("placeholder call should succeed");
    assert!(tool_text(&placeholder).contains("Would invoke clap command 'echo'"));
    client.shut_down().await.expect("shutdown should succeed");

    let client = launch_example("invalid_executable_server")
        .await
        .expect("invalid executable client should launch");
    let invalid = client
        .request_tool_call(CallToolRequestParams {
            name: "echo".into(),
            arguments: Some(serde_json::Map::from_iter([(
                "message".to_string(),
                serde_json::json!("hi"),
            )])),
            meta: None,
            task: None,
        })
        .await
        .expect("invalid executable path should still yield a tool result");
    assert_eq!(invalid.is_error, Some(true));
    assert!(tool_text(&invalid).contains("Failed to run command"));
    client.shut_down().await.expect("shutdown should succeed");
}

// Stdout capture and merge is Unix-only (run_with_stdout_capture is a no-op on Windows).
// The capture_stdout example also enables capture_stdout only on Unix, so skip this test on Windows.
#[tokio::test(flavor = "current_thread")]
#[cfg(unix)]
async fn capture_stdout_merges_only_text_outputs() {
    let client = launch_example("capture_stdout")
        .await
        .expect("capture stdout client should launch");

    let printed_only = client
        .request_tool_call(CallToolRequestParams {
            name: "printed-only".into(),
            arguments: Some(serde_json::Map::new()),
            meta: None,
            task: None,
        })
        .await
        .expect("printed-only call should succeed");
    assert_eq!(tool_text(&printed_only), "captured only");

    let printed_and_text = client
        .request_tool_call(CallToolRequestParams {
            name: "printed-and-text".into(),
            arguments: Some(serde_json::Map::new()),
            meta: None,
            task: None,
        })
        .await
        .expect("printed-and-text call should succeed");
    assert_eq!(
        tool_text(&printed_and_text),
        "returned text\ncaptured extra"
    );

    let structured = client
        .request_tool_call(CallToolRequestParams {
            name: "structured".into(),
            arguments: Some(serde_json::Map::new()),
            meta: None,
            task: None,
        })
        .await
        .expect("structured call should succeed");
    assert!(tool_text(&structured).contains("\"status\": \"ok\""));
    assert_eq!(
        structured
            .structured_content
            .as_ref()
            .and_then(|content| content.get("status"))
            .and_then(|value| value.as_str()),
        Some("ok")
    );

    client.shut_down().await.expect("shutdown should succeed");
}

#[tokio::test(flavor = "current_thread")]
async fn nested_subcommands_round_trip() {
    let client = launch_example("nested_subcommands")
        .await
        .expect("nested subcommands client should launch");

    let tools = client
        .request_tool_list(None)
        .await
        .expect("tool list should work")
        .tools;
    assert!(tools.iter().any(|tool| tool.name == "child"));

    let child = client
        .request_tool_call(CallToolRequestParams {
            name: "child".into(),
            arguments: Some(serde_json::Map::from_iter([(
                "value".to_string(),
                serde_json::json!("ok"),
            )])),
            meta: None,
            task: None,
        })
        .await
        .expect("nested child tool should succeed");
    assert!(tool_text(&child).contains("child=ok"));

    client.shut_down().await.expect("shutdown should succeed");
}

#[tokio::test(flavor = "current_thread")]
async fn logging_enabled_servers_expose_the_logging_guide_prompt() {
    let client = launch_example("tracing_bridge")
        .await
        .expect("tracing bridge client should launch");

    let prompts = client
        .request_prompt_list(None)
        .await
        .expect("prompt list should succeed")
        .prompts;
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt.name == clap_mcp::PROMPT_LOGGING_GUIDE)
    );

    let guide = client
        .request_prompt(GetPromptRequestParams {
            name: clap_mcp::PROMPT_LOGGING_GUIDE.into(),
            arguments: None,
            meta: None,
        })
        .await
        .expect("logging guide prompt should resolve");
    assert!(guide.messages.iter().any(|message| {
        message
            .content
            .as_text_content()
            .ok()
            .is_some_and(|text| text.text.contains("logger"))
    }));

    client.shut_down().await.expect("shutdown should succeed");
}
