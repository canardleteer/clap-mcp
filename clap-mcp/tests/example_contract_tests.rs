//! Example-driven MCP contracts (`example_contract` filter).
//!
//! | Example binary | Contract |
//! | --- | --- |
//! | `nested_subcommands` | `child` in tools; `internal` not in tools |
//! | `struct_subcommand_globals` | `greet` MCP round-trip (globals + `output_from` in `complex_cli_*`) |
//! | `optional_commands_and_args` | `internal` not in tools; `read` schema requires `path` |
//! | `struct_subcommand_required` | CLI argv parity (`cli_compat_tests.rs`) |

mod common;

use common::{launch_example, shutdown, tool_text};
use rmcp::model::CallToolRequestParams;
use std::collections::HashSet;

fn tool_names(tools: &[rmcp::model::Tool]) -> Vec<&str> {
    tools.iter().map(|t| t.name.as_ref()).collect()
}

fn required_schema_properties(tool: &rmcp::model::Tool) -> HashSet<String> {
    tool.input_schema
        .get("required")
        .and_then(|value| value.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(flavor = "current_thread")]
async fn example_contract_nested_subcommands_internal_skipped() {
    let client = launch_example("nested_subcommands")
        .await
        .expect("nested subcommands client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    let names = tool_names(&tools);
    assert!(names.contains(&"child"));
    assert!(
        !names.contains(&"internal"),
        "nested #[clap_mcp(skip)] must not appear in MCP tools: {names:?}"
    );

    let child =
        client
            .call_tool(CallToolRequestParams::new("child").with_arguments(
                serde_json::Map::from_iter([("value".to_string(), serde_json::json!("ok"))]),
            ))
            .await
            .expect("child tool should succeed");
    assert!(tool_text(&child).contains("child=ok"));

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn example_contract_struct_subcommand_globals_greet_round_trip() {
    let client = launch_example("struct_subcommand_globals")
        .await
        .expect("struct_subcommand_globals client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    assert!(
        tools.iter().any(|t| t.name == "greet"),
        "struct root with schema_only nested enum should expose leaf tools"
    );

    let greet =
        client
            .call_tool(CallToolRequestParams::new("greet").with_arguments(
                serde_json::Map::from_iter([("name".to_string(), serde_json::json!("Ada"))]),
            ))
            .await
            .expect("greet should succeed");
    assert!(
        tool_text(&greet).contains("Hello, Ada!"),
        "struct_subcommand_globals greet should round-trip over MCP"
    );

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn example_contract_optional_commands_internal_and_read_requires() {
    let client = launch_example("optional_commands_and_args")
        .await
        .expect("optional_commands_and_args client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    let names = tool_names(&tools);
    assert!(
        !names.contains(&"internal"),
        "skipped internal command must not be an MCP tool: {names:?}"
    );

    let read = tools
        .iter()
        .find(|t| t.name == "read")
        .expect("read tool should exist");
    assert!(
        required_schema_properties(read).contains("path"),
        "read tool should require path in MCP schema"
    );

    shutdown(client).await;
}
