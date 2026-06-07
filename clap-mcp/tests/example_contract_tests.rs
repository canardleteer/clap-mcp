//! Example-driven MCP contracts (`example_contract` filter).
//!
//! | Example binary | Contract |
//! | --- | --- |
//! | `nested_subcommands` | `child` in tools; `internal` not in tools |
//! | `struct_subcommand_globals` | `greet` + `verbose: true` → `verbose:` in output; global on leaf schema |
//! | `optional_commands_and_args` | `internal` not in tools; `read` schema requires `path` |
//! | `struct_subcommand_required` | CLI argv parity (`cli_compat_tests.rs`) |
//! | `arg_group_hints` | `search` has `meta.clapMcp.argGroups`; exec-only round-trip; both exec flags → parse error |

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

    let greet_tool = tools
        .iter()
        .find(|t| t.name == "greet")
        .expect("greet tool should exist");
    assert!(
        greet_tool
            .input_schema
            .get("properties")
            .and_then(|v| v.get("verbose"))
            .is_some(),
        "root global verbose should appear on greet tool inputSchema"
    );

    let greet = client
        .call_tool(
            CallToolRequestParams::new("greet").with_arguments(serde_json::Map::from_iter([
                ("name".to_string(), serde_json::json!("Ada")),
                ("verbose".to_string(), serde_json::json!(true)),
            ])),
        )
        .await
        .expect("greet should succeed");
    assert!(
        tool_text(&greet).contains("verbose:"),
        "struct_subcommand_globals greet with verbose should round-trip over MCP"
    );
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

#[tokio::test(flavor = "current_thread")]
async fn example_contract_arg_group_hints_meta_and_parse() {
    let client = launch_example("arg_group_hints")
        .await
        .expect("arg_group_hints client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    let search = tools
        .iter()
        .find(|t| t.name == "search")
        .expect("search tool should exist");

    let arg_groups = search
        .meta
        .as_ref()
        .and_then(|meta| meta.get("clapMcp"))
        .and_then(|value| value.get("argGroups"))
        .and_then(|value| value.as_array())
        .expect("search should expose argGroups meta");
    assert_eq!(arg_groups.len(), 1);
    assert_eq!(
        arg_groups[0].get("id").and_then(|v| v.as_str()),
        Some("execs")
    );
    let member_ids: Vec<&str> = arg_groups[0]
        .get("args")
        .and_then(|v| v.as_array())
        .expect("args array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(member_ids.contains(&"exec"));
    assert!(member_ids.contains(&"exec_batch"));

    let description = search
        .description
        .as_ref()
        .map(|d| d.to_string())
        .expect("search description");
    assert!(
        description.contains("Arg groups (parse-time)"),
        "description should include parse-time hint: {description}"
    );

    let ok = client
        .call_tool(
            CallToolRequestParams::new("search").with_arguments(serde_json::Map::from_iter([
                ("pattern".to_string(), serde_json::json!("*.rs")),
                ("exec".to_string(), serde_json::json!("echo hi")),
            ])),
        )
        .await
        .expect("search with exec only should succeed");
    assert!(tool_text(&ok).contains("pattern=*.rs"));
    assert!(tool_text(&ok).contains("exec=echo hi"));

    let both = client
        .call_tool(
            CallToolRequestParams::new("search").with_arguments(serde_json::Map::from_iter([
                ("pattern".to_string(), serde_json::json!("*.rs")),
                ("exec".to_string(), serde_json::json!("echo hi")),
                ("exec_batch".to_string(), serde_json::json!("echo batch")),
            ])),
        )
        .await
        .expect("search with both exec flags should return a tool result");
    assert_eq!(
        both.is_error,
        Some(true),
        "conflicting ArgGroup members should fail at parse time"
    );
    let err_text = tool_text(&both);
    assert!(
        err_text.contains("exec") || err_text.contains("cannot be used"),
        "parse failure should be reported: {err_text}"
    );

    shutdown(client).await;
}
