mod common;

use common::{launch_example, shutdown, tool_text};
use rmcp::model::CallToolRequestParams;

#[tokio::test(flavor = "current_thread")]
async fn stateful_counter_increments_across_tool_calls() {
    let client = launch_example("stateful_counter")
        .await
        .expect("stateful counter server should launch");

    let first = client
        .call_tool(CallToolRequestParams::new("increment"))
        .await
        .expect("first increment should succeed");
    assert_eq!(tool_text(&first), "count=1");

    let second = client
        .call_tool(CallToolRequestParams::new("increment"))
        .await
        .expect("second increment should succeed");
    assert_eq!(tool_text(&second), "count=2");

    let read = client
        .call_tool(CallToolRequestParams::new("read"))
        .await
        .expect("read should succeed");
    assert_eq!(tool_text(&read), "count=2");

    shutdown(client).await;
}
