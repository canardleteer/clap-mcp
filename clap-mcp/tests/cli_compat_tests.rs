//! CLI compatibility: non-MCP argv behaves like plain clap; MCP requires `--mcp`.

mod common;

use common::{example_binary_path, workspace_root};
use std::process::Command;

fn build_example(bin: &str) {
    let status = Command::new("cargo")
        .args(["build", "-p", "clap-mcp-examples", "--bin", bin])
        .current_dir(workspace_root())
        .status()
        .expect("cargo build");
    assert!(status.success(), "build {bin}");
}

fn run_example(bin: &str, args: &[&str]) -> std::process::Output {
    build_example(bin);
    Command::new(example_binary_path(bin))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("run {bin} {args:?}: {e}"))
}

#[test]
fn required_subcommand_bare_argv_fails_like_clap() {
    let out = run_example("struct_subcommand_required", &[]);
    assert!(
        !out.status.success(),
        "bare invocation must fail (required subcommand); stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn required_subcommand_normal_invocation_succeeds() {
    let out = run_example("struct_subcommand_required", &["greet", "--name", "Rust"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Hello, Rust!"), "got: {stdout}");
}

#[test]
fn optional_subcommand_bare_argv_succeeds() {
    let out = run_example("struct_subcommand", &[]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("No subcommand"), "got: {stdout}");
}

#[test]
fn flat_enum_normal_invocation_unchanged() {
    let out = run_example("subcommands", &["greet", "--name", "MCP"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Hello, MCP!"), "got: {stdout}");
}

#[test]
fn passthrough_exec_after_double_dash_does_not_start_mcp() {
    let out = run_example("passthrough_args", &["exec", "--", "--mcp"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(r#"command=["--mcp"]"#),
        "trailing --mcp should be passthrough, got: {stdout}"
    );
}

#[test]
fn passthrough_exec_dry_run_with_trailing_command() {
    let out = run_example(
        "passthrough_args",
        &["exec", "--dry-run", "--", "echo", "hello"],
    );
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("dry_run=true"), "got: {stdout}");
    assert!(
        stdout.contains(r#"command=["echo", "hello"]"#),
        "got: {stdout}"
    );
}

#[test]
fn custom_mcp_flags_user_mcp_is_not_clap_mcp() {
    let out = run_example("custom_mcp_flags", &["--mcp"]);
    assert!(
        out.status.success(),
        "user --mcp should exit quickly; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("legacy_mcp=true"),
        "user flag path should run, got: {stdout}"
    );
}

#[test]
fn custom_mcp_flags_help_shows_renamed_stdio_flag() {
    let out = run_example("custom_mcp_flags", &["--help"]);
    assert!(out.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("--modelcontextprotocol"),
        "help should show renamed clap-mcp stdio flag"
    );
    assert!(
        combined.contains("--mcp"),
        "help should still show unrelated user --mcp flag"
    );
}

#[test]
fn preserve_cli_parse_invalid_argv_shows_usage() {
    let out = run_example("preserve_cli_parse", &["greet"]);
    assert!(
        !out.status.success(),
        "missing required --name should fail; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Usage"),
        "preserve-cli path should surface clap Usage formatting: {stderr}"
    );
}

#[test]
fn preserve_cli_parse_valid_invocation_succeeds() {
    let out = run_example("preserve_cli_parse", &["greet", "--name", "Ada"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Hello, Ada!"), "got: {stdout}");
}
