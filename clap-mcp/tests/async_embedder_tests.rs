//! Integration tests for imperative async embedder serve (`serve_mcp`).

mod common;

use common::{launch_example_with_args, shutdown, tool_text};
use rmcp::model::CallToolRequestParams;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_embedder_serve_sleep_demo_tool() {
    let client = launch_example_with_args("async_embedder_serve", &[], Some("tracing"))
        .await
        .expect("client should launch");

    let result = client
        .call_tool(CallToolRequestParams::new("sleep-demo"))
        .await
        .expect("sleep-demo should succeed");

    let text = tool_text(&result);
    assert!(
        result.structured_content.is_some() || text.contains("completed"),
        "expected structured sleep result, got: {text}"
    );

    shutdown(client).await;
}
