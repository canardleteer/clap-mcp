mod common;

use common::{launch_example, prompt_has_text, read_text, shutdown, tool_text};
use rmcp::model::{CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams};

#[tokio::test(flavor = "current_thread")]
async fn custom_resources_and_prompts_round_trip() {
    let client = launch_example("custom_resources_prompts")
        .await
        .expect("client should launch");

    let resources = client
        .list_resources(None)
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
        .read_resource(ReadResourceRequestParams::new("example://readme"))
        .await
        .expect("custom resource should be readable");
    assert!(read_text(&readme).contains("Custom resources & prompts example"));

    let schema = client
        .read_resource(ReadResourceRequestParams::new("clap://schema"))
        .await
        .expect("schema resource should be readable");
    assert!(read_text(&schema).contains("\"name\": \"custom-resources-prompts\""));

    let prompts = client
        .list_prompts(None)
        .await
        .expect("prompt list should work")
        .prompts;
    assert!(prompts.iter().any(|prompt| prompt.name == "example-prompt"));

    let prompt = client
        .get_prompt(GetPromptRequestParams::new("example-prompt"))
        .await
        .expect("custom prompt should resolve");
    assert!(prompt_has_text(&prompt.messages, "prefer the echo tool"));

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_resources_prompts_tools_and_arguments_return_errors() {
    let client = launch_example("custom_resources_prompts")
        .await
        .expect("client should launch");

    let resource_error = client
        .read_resource(ReadResourceRequestParams::new("example://missing"))
        .await
        .expect_err("unknown resource should error");
    assert!(format!("{resource_error:?}").contains("unknown resource uri"));

    let prompt_error = client
        .get_prompt(GetPromptRequestParams::new("missing-prompt"))
        .await
        .expect_err("unknown prompt should error");
    assert!(format!("{prompt_error:?}").contains("unknown prompt"));

    shutdown(client).await;

    let client = launch_example("subcommands")
        .await
        .expect("subcommands client should launch");

    let tool_error = client
        .call_tool(
            CallToolRequestParams::new("missing-tool").with_arguments(serde_json::Map::new()),
        )
        .await
        .expect_err("unknown tool should error");
    assert!(
        tool_error
            .to_string()
            .to_lowercase()
            .contains("unknown tool")
    );

    let invalid_arg_error =
        client
            .call_tool(CallToolRequestParams::new("greet").with_arguments(
                serde_json::Map::from_iter([("bogus".to_string(), serde_json::json!("value"))]),
            ))
            .await
            .expect_err("unknown argument should error");
    assert!(
        invalid_arg_error
            .to_string()
            .contains("unknown argument: bogus")
    );

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn in_process_outputs_and_required_args_are_preserved() {
    let client = launch_example("structured")
        .await
        .expect("structured client should launch");
    let structured = client
        .call_tool(
            CallToolRequestParams::new("add").with_arguments(serde_json::Map::from_iter([
                ("a".to_string(), serde_json::json!(2)),
                ("b".to_string(), serde_json::json!(3)),
            ])),
        )
        .await
        .expect("structured call should succeed");
    assert_ne!(structured.is_error, Some(true));
    assert_eq!(
        structured
            .structured_content
            .as_ref()
            .and_then(|content| content.get("sum"))
            .and_then(|value| value.as_i64()),
        Some(5)
    );
    assert!(tool_text(&structured).contains("\"sum\": 5"));
    shutdown(client).await;

    let client = launch_example("result_output")
        .await
        .expect("result-output client should launch");
    let structured_error =
        client
            .call_tool(CallToolRequestParams::new("check").with_arguments(
                serde_json::Map::from_iter([("x".to_string(), serde_json::json!(0))]),
            ))
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
    shutdown(client).await;

    let client = launch_example("optional_commands_and_args")
        .await
        .expect("optional args client should launch");
    let missing_arg = client
        .call_tool(CallToolRequestParams::new("read").with_arguments(serde_json::Map::new()))
        .await
        .expect("missing required arg should be returned as tool result");
    assert_eq!(missing_arg.is_error, Some(true));
    assert!(tool_text(&missing_arg).contains("Missing required argument(s): path"));
    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn subprocess_and_direct_server_paths_return_expected_text() {
    let client = launch_example("stderr_success")
        .await
        .expect("stderr success client should launch");
    let success = client
        .call_tool(
            CallToolRequestParams::new("succeed-with-stderr")
                .with_arguments(serde_json::Map::new()),
        )
        .await
        .expect("successful subprocess call should succeed");
    let success_text = tool_text(&success);
    assert!(success_text.contains("stdout ok"));
    assert!(success_text.contains("stderr:\nstderr note"));
    shutdown(client).await;

    let client = launch_example("subprocess_exit_handling")
        .await
        .expect("subprocess exit client should launch");
    let failure = client
        .call_tool(CallToolRequestParams::new("exit-fail").with_arguments(serde_json::Map::new()))
        .await
        .expect("non-zero exit should still yield a tool result");
    assert_eq!(failure.is_error, Some(true));
    assert!(tool_text(&failure).contains("exited with non-zero status"));
    shutdown(client).await;

    let client = launch_example("placeholder_server")
        .await
        .expect("placeholder server client should launch");
    let placeholder =
        client
            .call_tool(CallToolRequestParams::new("echo").with_arguments(
                serde_json::Map::from_iter([("message".to_string(), serde_json::json!("hi"))]),
            ))
            .await
            .expect("placeholder call should succeed");
    assert!(tool_text(&placeholder).contains("Would invoke clap command 'echo'"));
    shutdown(client).await;

    let client = launch_example("invalid_executable_server")
        .await
        .expect("invalid executable client should launch");
    let invalid =
        client
            .call_tool(CallToolRequestParams::new("echo").with_arguments(
                serde_json::Map::from_iter([("message".to_string(), serde_json::json!("hi"))]),
            ))
            .await
            .expect("invalid executable path should still yield a tool result");
    assert_eq!(invalid.is_error, Some(true));
    assert!(tool_text(&invalid).contains("Failed to run command"));
    shutdown(client).await;
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
        .call_tool(
            CallToolRequestParams::new("printed-only").with_arguments(serde_json::Map::new()),
        )
        .await
        .expect("printed-only call should succeed");
    assert_eq!(tool_text(&printed_only), "captured only");

    let printed_and_text = client
        .call_tool(
            CallToolRequestParams::new("printed-and-text").with_arguments(serde_json::Map::new()),
        )
        .await
        .expect("printed-and-text call should succeed");
    assert_eq!(
        tool_text(&printed_and_text),
        "returned text\ncaptured extra"
    );

    let structured = client
        .call_tool(CallToolRequestParams::new("structured").with_arguments(serde_json::Map::new()))
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

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn nested_subcommands_round_trip() {
    let client = launch_example("nested_subcommands")
        .await
        .expect("nested subcommands client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    assert!(tools.iter().any(|tool| tool.name == "child"));

    let child =
        client
            .call_tool(CallToolRequestParams::new("child").with_arguments(
                serde_json::Map::from_iter([("value".to_string(), serde_json::json!("ok"))]),
            ))
            .await
            .expect("nested child tool should succeed");
    assert!(tool_text(&child).contains("child=ok"));

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn logging_enabled_servers_expose_the_logging_guide_prompt() {
    let client = launch_example("tracing_bridge")
        .await
        .expect("tracing bridge client should launch");

    let prompts = client
        .list_prompts(None)
        .await
        .expect("prompt list should succeed")
        .prompts;
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt.name == clap_mcp::PROMPT_LOGGING_GUIDE)
    );

    let guide = client
        .get_prompt(GetPromptRequestParams::new(clap_mcp::PROMPT_LOGGING_GUIDE))
        .await
        .expect("logging guide prompt should resolve");
    assert!(prompt_has_text(&guide.messages, "logger"));

    shutdown(client).await;
}
