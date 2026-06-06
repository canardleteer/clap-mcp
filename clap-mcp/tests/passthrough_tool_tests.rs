//! MCP integration tests for passthrough / trailing-arg tool patterns.

mod common;

use common::{launch_example, shutdown, tool_text};
use rmcp::model::CallToolRequestParams;

#[tokio::test(flavor = "current_thread")]
async fn exec_trailing_vec_in_process() {
    let client = launch_example("passthrough_args")
        .await
        .expect("passthrough_args client should launch");

    let mut args = serde_json::Map::new();
    args.insert("dry_run".into(), serde_json::json!(true));
    args.insert("command".into(), serde_json::json!(["-v", "hello"]));

    let result = client
        .call_tool(CallToolRequestParams::new("exec").with_arguments(args))
        .await
        .expect("exec with trailing hyphen tokens should succeed");

    let text = tool_text(&result);
    assert!(text.contains("dry_run=true"), "got: {text}");
    assert!(text.contains(r#"command=["-v", "hello"]"#), "got: {text}");

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn forward_long_vec_in_process() {
    let client = launch_example("passthrough_args")
        .await
        .expect("passthrough_args client should launch");

    let mut args = serde_json::Map::new();
    args.insert("args".into(), serde_json::json!(["foo", "bar"]));

    let result = client
        .call_tool(CallToolRequestParams::new("forward").with_arguments(args))
        .await
        .expect("forward should succeed");

    let text = tool_text(&result);
    assert!(text.contains(r#"args=["foo", "bar"]"#), "got: {text}");

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn skipped_arg_not_in_tool_list() {
    let client = launch_example("passthrough_args")
        .await
        .expect("passthrough_args client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("list_tools should succeed")
        .tools;

    let run_tool = tools
        .iter()
        .find(|t| t.name == "run")
        .expect("run tool should exist");

    let props = run_tool
        .input_schema
        .get("properties")
        .and_then(|value| value.as_object())
        .expect("run tool should have input schema properties");

    assert!(props.contains_key("input"), "run should expose input");
    assert!(
        !props.contains_key("internal"),
        "skipped internal arg must not appear in MCP schema"
    );

    let mut args = serde_json::Map::new();
    args.insert("input".into(), serde_json::json!("hello"));

    let result = client
        .call_tool(CallToolRequestParams::new("run").with_arguments(args))
        .await
        .expect("run without internal should succeed");

    let text = tool_text(&result);
    assert!(text.contains("input=hello"), "got: {text}");
    assert!(text.contains("internal=None"), "got: {text}");

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn exec_trailing_vec_subprocess() {
    let client = launch_example("passthrough_args_subprocess")
        .await
        .expect("passthrough_args_subprocess client should launch");

    let mut args = serde_json::Map::new();
    args.insert("command".into(), serde_json::json!(["echo", "subprocess"]));

    let result = client
        .call_tool(CallToolRequestParams::new("exec").with_arguments(args))
        .await
        .expect("subprocess exec should succeed");

    let text = tool_text(&result);
    assert!(
        text.contains(r#"command=["echo", "subprocess"]"#),
        "got: {text}"
    );

    shutdown(client).await;
}
