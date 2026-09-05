//! Draft 2020-12 instance validation for clap-derived tool `inputSchema`.
//!
//! These matrices assert accept/reject behavior for boolean flag constraints,
//! not only that generated JSON contains certain fragments.

use clap::{Arg, ArgAction, ArgGroup, Command, Parser};
use clap_mcp::{
    ClapMcpConfig, ClapMcpSchemaMetadata, schema_from_command, tools_from_schema_with_metadata,
};
use serde_json::{Value, json};

fn tool_input_schema(cmd: &Command, tool_name: &str) -> Value {
    let schema = schema_from_command(cmd);
    let tools = tools_from_schema_with_metadata(
        &schema,
        &ClapMcpConfig::default(),
        &ClapMcpSchemaMetadata::default(),
    );
    let tool = tools
        .iter()
        .find(|t| t.name == tool_name)
        .unwrap_or_else(|| panic!("missing tool {tool_name}"));
    serde_json::Value::Object(tool.input_schema.as_ref().clone())
}

fn assert_valid(schema: &Value, instance: Value) {
    let compiled = jsonschema::draft202012::new(schema)
        .unwrap_or_else(|e| panic!("schema compile failed: {e}\nschema={schema}"));
    if let Err(err) = compiled.validate(&instance) {
        panic!("expected valid instance {instance}: {err}\nschema={schema}");
    }
}

fn assert_invalid(schema: &Value, instance: Value) {
    let compiled = jsonschema::draft202012::new(schema)
        .unwrap_or_else(|e| panic!("schema compile failed: {e}\nschema={schema}"));
    assert!(
        compiled.validate(&instance).is_err(),
        "expected invalid instance {instance}\nschema={schema}"
    );
}

#[test]
fn set_true_conflict_and_required_group_matrix() {
    let cmd = Command::new("app").subcommand(
        Command::new("set-avatar")
            .arg(Arg::new("image").long("image").conflicts_with("remove"))
            .arg(
                Arg::new("remove")
                    .long("remove")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("image"),
            )
            .group(
                ArgGroup::new("src")
                    .args(["image", "remove"])
                    .required(true),
            ),
    );
    let schema = tool_input_schema(&cmd, "set-avatar");

    assert_valid(&schema, json!({ "image": "/tmp/a.png" }));
    assert_valid(&schema, json!({ "remove": true }));
    assert_valid(&schema, json!({ "image": "/tmp/a.png", "remove": false }));

    assert_invalid(&schema, json!({}));
    assert_invalid(&schema, json!({ "remove": false }));
    assert_invalid(&schema, json!({ "image": "/tmp/a.png", "remove": true }));
    assert_invalid(&schema, json!({ "unknown": 1 }));
}

#[test]
fn set_true_mutual_conflict_matrix() {
    let cmd = Command::new("app").subcommand(
        Command::new("apply")
            .arg(
                Arg::new("dry_run")
                    .long("dry-run")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("force"),
            )
            .arg(
                Arg::new("force")
                    .long("force")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("dry_run"),
            ),
    );
    let schema = tool_input_schema(&cmd, "apply");

    assert_valid(&schema, json!({}));
    assert_valid(&schema, json!({ "dry_run": true }));
    assert_valid(&schema, json!({ "force": true }));
    assert_valid(&schema, json!({ "dry_run": false, "force": true }));
    assert_valid(&schema, json!({ "dry_run": true, "force": false }));

    assert_invalid(&schema, json!({ "dry_run": true, "force": true }));
}

#[test]
fn set_true_required_unless_matrix() {
    let cmd = Command::new("app").subcommand(
        Command::new("auth")
            .arg(
                Arg::new("token")
                    .long("token")
                    .required_unless_present("dry_run"),
            )
            .arg(
                Arg::new("dry_run")
                    .long("dry-run")
                    .action(ArgAction::SetTrue),
            ),
    );
    let schema = tool_input_schema(&cmd, "auth");

    assert_valid(&schema, json!({ "token": "secret" }));
    assert_valid(&schema, json!({ "dry_run": true }));
    assert_valid(&schema, json!({ "token": "secret", "dry_run": false }));

    assert_invalid(&schema, json!({}));
    assert_invalid(&schema, json!({ "dry_run": false }));
}

#[test]
fn set_true_requires_matrix() {
    let cmd = Command::new("app").subcommand(
        Command::new("push")
            .arg(
                Arg::new("force")
                    .long("force")
                    .action(ArgAction::SetTrue)
                    .requires("confirm"),
            )
            .arg(
                Arg::new("confirm")
                    .long("confirm")
                    .action(ArgAction::SetTrue),
            ),
    );
    let schema = tool_input_schema(&cmd, "push");

    assert_valid(&schema, json!({}));
    assert_valid(&schema, json!({ "force": false }));
    assert_valid(&schema, json!({ "force": true, "confirm": true }));
    assert_valid(&schema, json!({ "confirm": true }));

    assert_invalid(&schema, json!({ "force": true }));
    assert_invalid(&schema, json!({ "force": true, "confirm": false }));
}

#[test]
fn set_false_conflict_matrix() {
    let cmd = Command::new("app").subcommand(
        Command::new("feature")
            .arg(
                Arg::new("no_color")
                    .long("no-color")
                    .action(ArgAction::SetFalse)
                    .conflicts_with("force_color"),
            )
            .arg(
                Arg::new("force_color")
                    .long("force-color")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("no_color"),
            ),
    );
    let schema = tool_input_schema(&cmd, "feature");

    // SetFalse is active only at const false (flag passed).
    assert_valid(&schema, json!({}));
    assert_valid(&schema, json!({ "no_color": true, "force_color": true }));
    assert_valid(&schema, json!({ "no_color": false }));
    assert_valid(&schema, json!({ "force_color": true }));

    assert_invalid(&schema, json!({ "no_color": false, "force_color": true }));
}

#[test]
fn derive_bool_possible_values_do_not_break_boolean_instances() {
    #[derive(Parser, Debug)]
    #[command(name = "app")]
    struct Cli {
        #[command(subcommand)]
        cmd: Cmd,
    }
    #[derive(clap::Subcommand, Debug)]
    enum Cmd {
        Toggle {
            #[arg(long, action = ArgAction::SetTrue)]
            enabled: bool,
        },
    }
    let cmd = <Cli as clap::CommandFactory>::command();
    let schema = tool_input_schema(&cmd, "toggle");
    let enabled = schema
        .pointer("/properties/enabled")
        .expect("enabled property");
    assert_eq!(enabled.get("type"), Some(&json!("boolean")));
    assert!(enabled.get("enum").is_none());

    assert_valid(&schema, json!({ "enabled": true }));
    assert_valid(&schema, json!({ "enabled": false }));
    assert_invalid(&schema, json!({ "enabled": "true" }));
}
