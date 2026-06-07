//! Example-driven MCP contracts (`example_contract` filter).
//!
//! | Example binary | Contract |
//! | --- | --- |
//! | `nested_subcommands` | `child` in tools; `internal` not in tools |
//! | `struct_subcommand_globals` | `greet` + `verbose: true` → `verbose:` in output; global on leaf schema |
//! | `optional_commands_and_args` | `internal` not in tools; `read` schema requires `path` |
//! | `struct_subcommand_required` | CLI argv parity (`cli_compat_tests.rs`) |
//! | `arg_group_hints` | `search` has `meta.clapMcp.argGroups`; exec-only round-trip; both exec flags → parse error |
//! | `flat_struct_root` | exactly one tool; wide `inputSchema` includes root + flattened arg ids |
//! | `flatten_skip` | skipped connection args absent; `reindex`/`repair` not in tools; `show` round-trip |
//! | `flatten_subcommand_skip_flat` | single root tool; `visible` on schema; `hidden-a`/`hidden-b` absent |
//! | `flatten_subcommand_skip_nested` | `build`/`compile`/`link`/`clean` absent from tools |
//! | `preserve_cli_parse` | invalid argv exits non-zero with Usage in stderr (see also `cli_compat_tests.rs`) |

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

fn schema_property_keys(tool: &rmcp::model::Tool) -> HashSet<String> {
    tool.input_schema
        .get("properties")
        .and_then(|value| value.as_object())
        .map(|props| props.keys().cloned().collect())
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

const FLAT_STRUCT_ROOT_TOOL: &str = "flat-struct-root";

#[tokio::test(flavor = "current_thread")]
async fn example_contract_flat_struct_root_single_tool() {
    let client = launch_example("flat_struct_root")
        .await
        .expect("flat_struct_root client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    assert_eq!(
        tools.len(),
        1,
        "flat struct root should expose one MCP tool, got: {:?}",
        tool_names(&tools)
    );
    let tool = &tools[0];
    assert_eq!(tool.name, FLAT_STRUCT_ROOT_TOOL);

    let keys = schema_property_keys(tool);
    for key in ["verbose", "target", "email", "region"] {
        assert!(
            keys.contains(key),
            "flat struct root schema should include {key}, got: {keys:?}"
        );
    }

    let result = client
        .call_tool(
            CallToolRequestParams::new(FLAT_STRUCT_ROOT_TOOL).with_arguments(
                serde_json::Map::from_iter([
                    ("target".to_string(), serde_json::json!("prod")),
                    ("verbose".to_string(), serde_json::json!(true)),
                ]),
            ),
        )
        .await
        .expect("flat struct root tool call should succeed");
    assert!(tool_text(&result).contains("target=prod"));

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn example_contract_flatten_skip_hidden_args_and_commands() {
    let client = launch_example("flatten_skip")
        .await
        .expect("flatten_skip client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    let names = tool_names(&tools);
    assert!(names.contains(&"show"));
    assert!(names.contains(&"flush"));
    assert!(
        !names.contains(&"reindex") && !names.contains(&"repair"),
        "skipped variants must not be MCP tools: {names:?}"
    );

    let show = tools
        .iter()
        .find(|t| t.name == "show")
        .expect("show tool should exist");
    let show_keys = schema_property_keys(show);
    assert!(
        !show_keys.contains("host") && !show_keys.contains("port"),
        "skipped flattened connection args must not appear on show schema: {show_keys:?}"
    );

    let flush = tools
        .iter()
        .find(|t| t.name == "flush")
        .expect("flush tool should exist");
    assert!(
        schema_property_keys(flush).contains("custom-out"),
        "flush should expose custom clap arg id on schema"
    );
    let serialized = flush
        .meta
        .as_ref()
        .and_then(|m| m.get("clapMcp"))
        .and_then(|v| v.get("serialized"))
        .and_then(|v| v.as_bool());
    assert_eq!(
        serialized,
        Some(true),
        "flush should advertise serialized meta"
    );

    let show_result =
        client
            .call_tool(CallToolRequestParams::new("show").with_arguments(
                serde_json::Map::from_iter([("id".to_string(), serde_json::json!("abc"))]),
            ))
            .await
            .expect("show should succeed");
    assert!(tool_text(&show_result).contains("show:abc"));

    shutdown(client).await;
}

const FLATTEN_SUBCOMMAND_SKIP_FLAT_TOOL: &str = "flatten-subcommand-skip-flat";
const FLATTEN_SUBCOMMAND_SKIP_NESTED_TOOL: &str = "flatten-subcommand-skip-nested";

#[tokio::test(flavor = "current_thread")]
async fn example_contract_flatten_subcommand_skip_flat() {
    let client = launch_example("flatten_subcommand_skip_flat")
        .await
        .expect("flatten_subcommand_skip_flat client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    assert_eq!(
        tools.len(),
        1,
        "flat skip should expose one wide root tool, got: {:?}",
        tool_names(&tools)
    );
    let tool = &tools[0];
    assert_eq!(tool.name, FLATTEN_SUBCOMMAND_SKIP_FLAT_TOOL);
    assert!(
        schema_property_keys(tool).contains("visible"),
        "root visible flag should appear on MCP schema"
    );
    let names = tool_names(&tools);
    assert!(
        !names.contains(&"hidden-a") && !names.contains(&"hidden-b"),
        "flattened skipped subcommands must not become MCP tools: {names:?}"
    );

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn example_contract_flatten_subcommand_skip_nested() {
    let client = launch_example("flatten_subcommand_skip_nested")
        .await
        .expect("flatten_subcommand_skip_nested client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    let names = tool_names(&tools);
    for hidden in ["build", "compile", "link", "clean"] {
        assert!(
            !names.contains(&hidden),
            "nested flattened skip must hide {hidden}, got: {names:?}"
        );
    }
    assert!(
        tools.len() <= 1,
        "nested skip should not expose per-subcommand tools, got: {names:?}"
    );
    if tools.len() == 1 {
        assert_eq!(tools[0].name, FLATTEN_SUBCOMMAND_SKIP_NESTED_TOOL);
    }

    shutdown(client).await;
}
