//! Example-driven MCP contracts (`example_contract` filter).
//!
//! | Example binary | Contract |
//! | --- | --- |
//! | `nested_subcommands` | `child` in tools; `parent` / `internal` not in tools |
//! | `struct_subcommand_globals` | `greet` + `verbose: true` → `verbose:` in output; `api_token` skipped from schema |
//! | `optional_commands_and_args` | `internal` not in tools; `read` schema requires `path` |
//! | `struct_subcommand_required` | CLI argv parity (`cli_compat_tests.rs`) |
//! | `arg_group_hints` | `search` has `meta.clapMcp.argGroups`; exec-only round-trip; both exec flags → parse error |
//! | `flat_struct_root` | exactly one tool; wide `inputSchema`; `config_dir` default hidden; `env` default overridden |
//! | `flatten_skip` | skipped connection args absent; `reindex`/`repair` not in tools; `show` round-trip |
//! | `flatten_subcommand_skip_flat` | single root tool; `visible` on schema; `hidden-a`/`hidden-b` absent |
//! | `flatten_subcommand_skip_nested` | `build`/`compile`/`link`/`clean` absent from tools |
//! | `preserve_cli_parse` | invalid argv exits non-zero with Usage in stderr (see also `cli_compat_tests.rs`) |
//! | `server_metadata` | initialize `server_info` + instructions; tool titles / annotation hints |
//! | `vec_and_flags` | `port` integer; `size` string; `ratio` number via `input_type` |
//! | `structured` | `add` has object `outputSchema`; `echo` has none |
//! | `stderr_success` | `SubprocessStderr::Notify` advertises logging capability |

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
        !names.contains(&"parent"),
        "leaves_only must omit intermediate parent tools: {names:?}"
    );
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
    assert!(
        greet_tool
            .input_schema
            .get("properties")
            .and_then(|v| v.get("api_token"))
            .is_none(),
        "skip_global api_token must not appear on greet inputSchema"
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
    for key in ["verbose", "target", "email", "region", "config_dir", "env"] {
        assert!(
            keys.contains(key),
            "flat struct root schema should include {key}, got: {keys:?}"
        );
    }

    let props = tool
        .input_schema
        .get("properties")
        .and_then(|v| v.as_object())
        .expect("properties");
    assert!(
        props
            .get("config_dir")
            .and_then(|p| p.get("default"))
            .is_none(),
        "hide_default must omit config_dir default: {:?}",
        props.get("config_dir")
    );
    assert_eq!(
        props.get("env").and_then(|p| p.get("default")),
        Some(&serde_json::json!("staging")),
        "override_default must advertise staging for env"
    );

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

#[tokio::test(flavor = "current_thread")]
async fn example_contract_server_metadata_identity_and_annotations() {
    let client = launch_example("server_metadata")
        .await
        .expect("server_metadata client should launch");

    let peer_info = client.peer_info().expect("peer_info from initialize");
    let server_info = peer_info.server_info.as_ref().expect("server_info");
    assert_eq!(server_info.name, "server-metadata-example");
    assert_eq!(
        server_info.title.as_deref(),
        Some("Server Metadata Example")
    );
    assert_eq!(
        peer_info.instructions.as_deref(),
        Some("Prefer the status tool before calling reset.")
    );
    assert!(
        peer_info.capabilities.logging.is_none(),
        "instructions alone must not enable logging"
    );

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    let status = tools
        .iter()
        .find(|t| t.name == "status")
        .expect("status tool");
    assert_eq!(status.title.as_deref(), Some("Service Status"));
    let status_ann = status.annotations.as_ref().expect("status annotations");
    assert_eq!(status_ann.read_only_hint, Some(true));
    assert_eq!(status_ann.idempotent_hint, Some(true));

    let reset = tools
        .iter()
        .find(|t| t.name == "reset")
        .expect("reset tool");
    assert_eq!(reset.title.as_deref(), Some("Reset Service State"));
    let reset_ann = reset.annotations.as_ref().expect("reset annotations");
    assert_eq!(reset_ann.destructive_hint, Some(true));
    assert_eq!(reset_ann.open_world_hint, Some(true));

    let status_result = client
        .call_tool(CallToolRequestParams::new("status"))
        .await
        .expect("status should succeed");
    assert!(tool_text(&status_result).contains("healthy"));

    shutdown(client).await;
}

fn property_type(tool: &rmcp::model::Tool, key: &str) -> Option<String> {
    tool.input_schema
        .get("properties")
        .and_then(|v| v.get(key))
        .and_then(|p| p.get("type"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

#[tokio::test(flavor = "current_thread")]
async fn example_contract_vec_and_flags_numeric_input_types() {
    let client = launch_example("vec_and_flags")
        .await
        .expect("vec_and_flags client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    let run = tools
        .iter()
        .find(|t| t.name == "run")
        .expect("run tool should exist");

    assert_eq!(property_type(run, "port").as_deref(), Some("integer"));
    assert_eq!(property_type(run, "size").as_deref(), Some("string"));
    assert_eq!(property_type(run, "ratio").as_deref(), Some("number"));
    assert_eq!(property_type(run, "verbose").as_deref(), Some("integer"));
    assert_eq!(property_type(run, "dry_run").as_deref(), Some("boolean"));

    let result = client
        .call_tool(
            CallToolRequestParams::new("run").with_arguments(serde_json::Map::from_iter([
                ("files".to_string(), serde_json::json!(["a", "b"])),
                ("port".to_string(), serde_json::json!(8080)),
                ("size".to_string(), serde_json::json!("8MiB")),
                ("ratio".to_string(), serde_json::json!(1.5)),
            ])),
        )
        .await
        .expect("run should succeed");
    let text = tool_text(&result);
    assert!(text.contains("port=Some(8080)"), "{text}");
    assert!(text.contains("size=Some(8)"), "{text}");
    assert!(text.contains("ratio=Some(\"1.5\")"), "{text}");

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn example_contract_structured_per_tool_output_schema() {
    let client = launch_example("structured")
        .await
        .expect("structured client should launch");

    let tools = client
        .list_tools(None)
        .await
        .expect("tool list should work")
        .tools;
    let add = tools.iter().find(|t| t.name == "add").expect("add tool");
    assert!(
        add.output_schema.is_some(),
        "add should advertise outputSchema"
    );
    assert_eq!(
        add.output_schema
            .as_ref()
            .and_then(|s| s.get("type"))
            .and_then(|t| t.as_str()),
        Some("object")
    );

    let echo = tools.iter().find(|t| t.name == "echo").expect("echo tool");
    assert!(
        echo.output_schema.is_none(),
        "echo must not inherit add outputSchema"
    );

    let result = client
        .call_tool(
            CallToolRequestParams::new("add").with_arguments(serde_json::Map::from_iter([
                ("a".to_string(), serde_json::json!(2)),
                ("b".to_string(), serde_json::json!(3)),
            ])),
        )
        .await
        .expect("add should succeed");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|v| v.get("sum")),
        Some(&serde_json::json!(5))
    );

    shutdown(client).await;
}

#[tokio::test(flavor = "current_thread")]
async fn example_contract_stderr_success_notify_logging() {
    let client = launch_example("stderr_success")
        .await
        .expect("stderr_success client should launch");

    let peer_info = client.peer_info().expect("peer_info from initialize");
    assert!(
        peer_info.capabilities.logging.is_some(),
        "SubprocessStderr::Notify must advertise logging"
    );

    let result = client
        .call_tool(CallToolRequestParams::new("succeed-with-stderr"))
        .await
        .expect("succeed-with-stderr should succeed");
    let text = tool_text(&result);
    assert!(
        text.contains("stdout ok") && text.contains("stderr note"),
        "success text should include stdout and captured stderr: {text}"
    );

    shutdown(client).await;
}
