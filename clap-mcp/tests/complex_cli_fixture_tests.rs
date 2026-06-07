//! Contract tests for the canonical complex CLI fixture (`complex_cli_*` filter).

mod complex_cli_fixture;

use clap::CommandFactory;
use clap_mcp::{
    ClapMcpConfig, ClapMcpSchemaMetadataProvider, ClapMcpToolExecutor, ClapMcpToolOutput,
    schema_from_command_with_metadata, tools_from_schema_with_metadata,
};
use complex_cli_fixture::{TestComplexCli, TestComplexSandbox, TestComplexTop};

#[test]
fn complex_cli_nested_skip_absent_from_tools() {
    let cmd = TestComplexCli::command();
    let metadata = TestComplexCli::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"leaf"));
    assert!(
        !names.contains(&"connect"),
        "nested #[clap_mcp(skip)] on child enum must propagate to root metadata"
    );
}

#[test]
fn complex_cli_struct_output_from_sees_globals() {
    let cli = TestComplexCli {
        verbose: true,
        command: TestComplexTop::Sandbox {
            command: TestComplexSandbox::Leaf {
                name: Some("test".into()),
            },
        },
    };
    let out = cli
        .execute_for_mcp()
        .expect("struct-root output_from should execute");
    match out {
        ClapMcpToolOutput::Text(s) => assert_eq!(s, "leaf:test:verbose=true"),
        other => panic!("expected text output, got {other:?}"),
    }
}

#[test]
fn complex_cli_skipped_multi_positional_compiles() {
    let cmd = TestComplexCli::command();
    let metadata = TestComplexCli::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(
        !names.contains(&"upload"),
        "skipped variant with multiple positionals must not appear as an MCP tool"
    );
}

#[test]
fn complex_cli_leaf_tool_schema_includes_root_global() {
    let cmd = TestComplexCli::command();
    let metadata = TestComplexCli::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let leaf = tools.iter().find(|t| t.name == "leaf").expect("leaf tool");
    assert!(
        leaf.input_schema
            .get("properties")
            .and_then(|v| v.get("verbose"))
            .is_some(),
        "root global verbose should appear on nested leaf tool inputSchema"
    );
}

#[test]
fn complex_cli_requires_in_schema() {
    let cmd = TestComplexCli::command();
    let metadata = TestComplexCli::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let commands = schema.root.all_commands();
    let leaf_cmd = commands
        .iter()
        .find(|c| c.name == "leaf")
        .expect("leaf command");
    let name_arg = leaf_cmd
        .args
        .iter()
        .find(|a| a.id == "name")
        .expect("name arg");
    assert!(name_arg.required, "name should be required in MCP schema");
}

#[test]
fn complex_cli_schema_only_intermediate() {
    // `TestComplexTop` and `TestComplexSandbox` use #[clap_mcp(schema_only)] without
    // per-enum output_from; compile-time coverage is the fixture module itself and
    // tests/ui/pass/schema_only_nested.rs.
    let metadata = TestComplexCli::clap_mcp_schema_metadata();
    assert!(
        !metadata.skip_commands.contains(&"leaf".to_string()),
        "exposed leaf under schema_only nesting should remain a tool"
    );
    assert!(
        metadata.skip_commands.contains(&"connect".to_string()),
        "nested skip must merge to root metadata"
    );
}
