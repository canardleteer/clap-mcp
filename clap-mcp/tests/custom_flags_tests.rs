//! Tests for configurable clap-mcp builtin flag long names.

mod common;

use clap_mcp::{ClapMcpBuiltinFlags, EXPORT_SKILLS_FLAG_LONG, MCP_FLAG_LONG};
use common::{launch_example_with_stdio_flag, shutdown};

#[test]
fn default_flags_match_constants() {
    let flags = ClapMcpBuiltinFlags::default();
    assert_eq!(flags.stdio_long, MCP_FLAG_LONG);
    assert_eq!(flags.export_skills_long, EXPORT_SKILLS_FLAG_LONG);
    #[cfg(feature = "http")]
    assert_eq!(flags.http_long, clap_mcp::MCP_HTTP_FLAG_LONG);
}

#[tokio::test(flavor = "current_thread")]
async fn custom_stdio_long_starts_mcp_in_process() {
    let client =
        launch_example_with_stdio_flag("custom_mcp_flags", &[], None, "--modelcontextprotocol")
            .await
            .expect("custom_mcp_flags with --modelcontextprotocol should start MCP");

    let tools = client
        .list_tools(None)
        .await
        .expect("list_tools should succeed")
        .tools;
    assert!(
        !tools.is_empty(),
        "MCP server should expose at least one tool"
    );

    shutdown(client).await;
}
