//! Tests for ClapMcpConfig and configuration possibilities.

use clap::{CommandFactory, Parser, Subcommand};
use clap_mcp::ClapMcp;
use clap_mcp::{
    ClapMcpConfig, ClapMcpConfigProvider, ClapMcpRunnable, ClapMcpSchemaMetadata,
    ClapMcpSchemaMetadataProvider, ClapMcpToolExecutor, ClapMcpToolOutput,
    LOG_INTERPRETATION_INSTRUCTIONS, LOGGING_GUIDE_CONTENT, PROMPT_LOGGING_GUIDE, ParseOrServeMcp,
    run_async_tool, schema_from_command, schema_from_command_with_metadata,
    tools_from_schema_with_config, tools_from_schema_with_config_and_metadata,
};
use serde::Serialize;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[command(name = "test-cli")]
enum TestCliDefaults {
    Foo,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe)]
#[command(name = "test-cli-both-true")]
enum TestCliBothTrue {
    Bar,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = true)]
#[command(name = "test-cli-parallel-only")]
enum TestCliParallelOnly {
    Baz,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(name = "test-cli-reinvoke-only")]
enum TestCliReinvokeOnly {
    #[clap_mcp_output = "format!(\"result: {}\", x)"]
    Qux { x: i32 },
}

#[derive(Debug, Serialize)]
struct SubResult {
    difference: i32,
    minuend: i32,
    subtrahend: i32,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime = false)]
#[command(name = "test-cli-structured")]
enum TestCliStructured {
    #[clap_mcp_output_json = "SubResult { difference: a - b, minuend: a, subtrahend: b }"]
    Sub { a: i32, b: i32 },
}

// --- #[clap_mcp_output_from = "run"] ---

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "test-cli-output-from")]
enum TestCliOutputFrom {
    TextOut { x: i32 },
    OptionOut { present: bool },
    ResultOk,
    ResultErr,
    StructuredOut { a: i32, b: i32 },
}

fn run(cmd: TestCliOutputFrom) -> Result<OutputFromResult, String> {
    match cmd {
        TestCliOutputFrom::TextOut { x } => Ok(OutputFromResult::Text(format!("x={}", x))),
        TestCliOutputFrom::OptionOut { present } => {
            if present {
                Ok(OutputFromResult::Text("some".to_string()))
            } else {
                Ok(OutputFromResult::Empty)
            }
        }
        TestCliOutputFrom::ResultOk => Ok(OutputFromResult::Text("ok".to_string())),
        TestCliOutputFrom::ResultErr => Err("fail".to_string()),
        TestCliOutputFrom::StructuredOut { a, b } => Ok(OutputFromResult::Structured(SubResult {
            difference: a - b,
            minuend: a,
            subtrahend: b,
        })),
    }
}

#[derive(Debug)]
enum OutputFromResult {
    Text(String),
    Empty,
    Structured(SubResult),
}

impl clap_mcp::IntoClapMcpResult for OutputFromResult {
    fn into_tool_result(
        self,
    ) -> std::result::Result<ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
        match self {
            OutputFromResult::Text(s) => Ok(ClapMcpToolOutput::Text(s)),
            OutputFromResult::Empty => Ok(ClapMcpToolOutput::Text(String::new())),
            OutputFromResult::Structured(s) => Ok(ClapMcpToolOutput::Structured(
                serde_json::to_value(&s).expect("serialize"),
            )),
        }
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime)]
#[command(name = "test-cli-share-runtime")]
enum TestCliShareRuntime {
    Foo,
}

// Struct root with required subcommand
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(name = "test-struct-cli")]
struct TestStructCli {
    #[command(subcommand)]
    command: TestStructCommands,
}

#[derive(Debug, Subcommand, ClapMcp)]
enum TestStructCommands {
    #[clap_mcp_output = "format!(\"sum: {}\", a + b)"]
    Add { a: i32, b: i32 },
}

// Struct root with optional subcommand
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(name = "test-struct-optional-cli", subcommand_required = false)]
struct TestStructOptionalCli {
    #[command(subcommand)]
    command: Option<TestStructOptionalCommands>,
}

#[derive(Debug, Subcommand, ClapMcp)]
enum TestStructOptionalCommands {
    #[clap_mcp_output_literal = "done"]
    Done,
}

// Root struct with #[clap_mcp(skip_root_when_subcommands)] — root excluded from MCP tool list via derive
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp(skip_root_when_subcommands)]
#[command(name = "test-root-skip-when-subcommands", subcommand_required = false)]
struct TestRootSkipWhenSubcommands {
    #[command(subcommand)]
    command: Option<TestStructOptionalCommands>,
}

#[test]
fn test_config_default() {
    let config = ClapMcpConfig::default();
    assert!(
        !config.reinvocation_safe,
        "reinvocation_safe should default to false"
    );
    assert!(
        !config.parallel_safe,
        "parallel_safe should default to false"
    );
    assert!(
        !config.share_runtime,
        "share_runtime should default to false"
    );
    assert!(
        config.allow_mcp_without_subcommand,
        "allow_mcp_without_subcommand should default to true"
    );
}

#[test]
fn test_clap_mcp_config_provider_defaults() {
    let config = TestCliDefaults::clap_mcp_config();
    assert!(!config.reinvocation_safe);
    assert!(!config.parallel_safe);
}

#[test]
fn test_clap_mcp_config_provider_both_true() {
    let config = TestCliBothTrue::clap_mcp_config();
    assert!(config.reinvocation_safe);
    assert!(config.parallel_safe);
}

#[test]
fn test_clap_mcp_config_provider_parallel_only() {
    let config = TestCliParallelOnly::clap_mcp_config();
    assert!(!config.reinvocation_safe);
    assert!(config.parallel_safe);
}

#[test]
fn test_clap_mcp_config_provider_reinvoke_only() {
    let config = TestCliReinvokeOnly::clap_mcp_config();
    assert!(config.reinvocation_safe);
    assert!(!config.parallel_safe);
}

#[test]
fn test_clap_mcp_config_provider_share_runtime() {
    let config = TestCliShareRuntime::clap_mcp_config();
    assert!(config.reinvocation_safe);
    assert!(config.share_runtime);
}

#[test]
fn test_clap_mcp_config_provider_share_runtime_defaults_when_omitted() {
    // TestCliReinvokeOnly has reinvocation_safe but no share_runtime attribute
    let config = TestCliReinvokeOnly::clap_mcp_config();
    assert!(config.reinvocation_safe);
    assert!(
        !config.share_runtime,
        "share_runtime should default to false when omitted"
    );
}

#[test]
fn test_tools_from_schema_with_config_meta() {
    let cmd = TestCliDefaults::command();
    let schema = schema_from_command(&cmd);

    let config_false_false = ClapMcpConfig {
        reinvocation_safe: false,
        parallel_safe: false,
        ..Default::default()
    };
    let metadata = ClapMcpSchemaMetadata::default();
    let tools = tools_from_schema_with_config_and_metadata(&schema, &config_false_false, &metadata);
    assert!(!tools.is_empty());
    for tool in &tools {
        let meta = tool.meta.as_ref().expect("tool should have meta");
        let clap_mcp = meta.get("clapMcp").expect("meta should have clapMcp");
        let obj = clap_mcp.as_object().expect("clapMcp should be object");
        assert_eq!(
            obj.get("reinvocationSafe").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            obj.get("parallelSafe").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    let config_true_true = ClapMcpConfig {
        reinvocation_safe: true,
        parallel_safe: true,
        ..Default::default()
    };
    let tools = tools_from_schema_with_config(&schema, &config_true_true);
    for tool in &tools {
        let meta = tool.meta.as_ref().expect("tool should have meta");
        let clap_mcp = meta.get("clapMcp").expect("meta should have clapMcp");
        let obj = clap_mcp.as_object().expect("clapMcp should be object");
        assert_eq!(
            obj.get("reinvocationSafe").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            obj.get("parallelSafe").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            obj.get("shareRuntime").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    let config_share_runtime = ClapMcpConfig {
        reinvocation_safe: true,
        parallel_safe: false,
        share_runtime: true,
        ..Default::default()
    };
    let tools = tools_from_schema_with_config(&schema, &config_share_runtime);
    for tool in &tools {
        let meta = tool.meta.as_ref().expect("tool should have meta");
        let clap_mcp = meta.get("clapMcp").expect("meta should have clapMcp");
        let obj = clap_mcp.as_object().expect("clapMcp should be object");
        assert_eq!(
            obj.get("shareRuntime").and_then(|v| v.as_bool()),
            Some(true)
        );
    }
}

#[test]
fn test_clap_mcp_runnable() {
    let result = TestCliReinvokeOnly::Qux { x: 42 }.run();
    assert_eq!(result, "result: 42");
}

#[test]
fn test_clap_mcp_runnable_default_debug() {
    let result = TestCliDefaults::Foo.run();
    // Unit variants without #[clap_mcp_output] default to kebab-case variant name
    assert_eq!(result, "foo");
}

#[test]
fn test_clap_mcp_tool_output_text() {
    let out = ClapMcpToolOutput::Text("hello".to_string());
    assert_eq!(out.as_text(), Some("hello"));
    assert!(out.as_structured().is_none());
    assert_eq!(out.into_string(), "hello");
}

#[test]
fn test_clap_mcp_tool_output_structured() {
    let v = serde_json::json!({"x": 1, "y": 2});
    let out = ClapMcpToolOutput::Structured(v.clone());
    assert!(out.as_text().is_none());
    assert_eq!(out.as_structured(), Some(&v));
    let s = out.into_string();
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
    assert_eq!(parsed.get("x").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(parsed.get("y").and_then(|v| v.as_i64()), Some(2));
}

#[test]
fn test_logging_constants() {
    assert_eq!(PROMPT_LOGGING_GUIDE, "clap-mcp-logging-guide");
    assert!(LOG_INTERPRETATION_INSTRUCTIONS.contains("stderr"));
    assert!(LOG_INTERPRETATION_INSTRUCTIONS.contains("app"));
    assert!(LOGGING_GUIDE_CONTENT.contains("stderr"));
    assert!(LOGGING_GUIDE_CONTENT.contains("app"));
}

#[test]
fn test_clap_mcp_tool_executor_structured() {
    let sub = TestCliStructured::Sub { a: 10, b: 3 };
    let out = sub.execute_for_mcp().expect("should succeed");
    let v = out.as_structured().expect("should be structured");
    assert_eq!(v.get("difference").and_then(|x| x.as_i64()), Some(7));
    assert_eq!(v.get("minuend").and_then(|x| x.as_i64()), Some(10));
    assert_eq!(v.get("subtrahend").and_then(|x| x.as_i64()), Some(3));
}

/// ParseOrServeMcp is implemented for types that derive ClapMcp with the right bounds.
#[test]
fn test_parse_or_serve_mcp_trait_implemented() {
    fn require_parse_or_serve_mcp<T: ParseOrServeMcp>() {}
    require_parse_or_serve_mcp::<TestCliOutputFrom>();
    require_parse_or_serve_mcp::<TestStructOptionalCli>();
    require_parse_or_serve_mcp::<TestStructCli>();
}

#[test]
fn test_clap_mcp_output_from_text() {
    let cli = TestCliOutputFrom::TextOut { x: 42 };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some("x=42"));
}

#[test]
fn test_clap_mcp_output_from_option_some() {
    let cli = TestCliOutputFrom::OptionOut { present: true };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some("some"));
}

#[test]
fn test_clap_mcp_output_from_option_none() {
    let cli = TestCliOutputFrom::OptionOut { present: false };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some(""));
}

#[test]
fn test_clap_mcp_output_from_result_ok() {
    let cli = TestCliOutputFrom::ResultOk;
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some("ok"));
}

#[test]
fn test_clap_mcp_output_from_result_err() {
    let cli = TestCliOutputFrom::ResultErr;
    let err = cli.execute_for_mcp().expect_err("should fail");
    assert!(err.message.contains("fail"));
}

#[test]
fn test_clap_mcp_output_from_structured() {
    let cli = TestCliOutputFrom::StructuredOut { a: 10, b: 3 };
    let out = cli.execute_for_mcp().expect("should succeed");
    let v = out.as_structured().expect("should be structured");
    assert_eq!(v.get("difference").and_then(|x| x.as_i64()), Some(7));
}

// --- run_async_tool and share_runtime edge cases ---

#[test]
fn test_run_async_tool_dedicated_thread_reinvocation_safe_false() {
    // When reinvocation_safe=false, always uses dedicated thread (share_runtime ignored)
    let config = ClapMcpConfig {
        reinvocation_safe: false,
        parallel_safe: false,
        share_runtime: true, // ignored
        ..Default::default()
    };
    let result = run_async_tool(&config, || async { 42 });
    assert_eq!(result, 42);
}

#[test]
fn test_run_async_tool_dedicated_thread_share_runtime_false() {
    // When share_runtime=false, uses dedicated thread even with reinvocation_safe=true
    let config = ClapMcpConfig {
        reinvocation_safe: true,
        parallel_safe: false,
        share_runtime: false,
        ..Default::default()
    };
    let result = run_async_tool(&config, || async { 99 });
    assert_eq!(result, 99);
}

#[test]
fn test_run_async_tool_dedicated_thread_share_runtime_true_but_reinvoke_false() {
    // share_runtime=true with reinvocation_safe=false: uses dedicated thread
    let config = ClapMcpConfig {
        reinvocation_safe: false,
        parallel_safe: true,
        share_runtime: true,
        ..Default::default()
    };
    let result = run_async_tool(&config, || async { "hello".to_string() });
    assert_eq!(result, "hello");
}

#[test]
fn test_run_async_tool_returns_complex_type() {
    let config = ClapMcpConfig::default();
    let result = run_async_tool(&config, || async { vec![1u8, 2, 3] });
    assert_eq!(result, vec![1, 2, 3]);
}

// Shared runtime path (reinvocation_safe=true + share_runtime=true) is exercised
// via integration: run the async_sleep example with share_runtime in #[clap_mcp].
// Unit-testing it would require block_on from within a tokio worker, which panics.

// --- struct root with #[command(subcommand)] ---

#[test]
fn test_struct_cli_config_provider() {
    let config = TestStructCli::clap_mcp_config();
    assert!(config.reinvocation_safe);
    assert!(!config.parallel_safe);
}

#[test]
fn test_struct_cli_executor_delegates() {
    let cli = TestStructCli {
        command: TestStructCommands::Add { a: 3, b: 7 },
    };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some("sum: 10"));
}

#[test]
fn test_struct_optional_cli_executor_some() {
    let cli = TestStructOptionalCli {
        command: Some(TestStructOptionalCommands::Done),
    };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some("done"));
}

#[test]
fn test_struct_optional_cli_executor_none() {
    let cli = TestStructOptionalCli { command: None };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some(""));
}

// --- #[clap_mcp(skip)] and #[clap_mcp(requires)] ---

// Root-level skip: struct with subcommand and a root field hidden from MCP
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(name = "test-root-skip")]
struct TestRootSkip {
    #[clap_mcp(skip)]
    #[arg(long)]
    out: Option<String>,
    #[command(subcommand)]
    command: TestRootSkipCommands,
}

#[derive(Debug, Subcommand, ClapMcp)]
enum TestRootSkipCommands {
    #[clap_mcp_output_literal = "ok"]
    Foo,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(name = "test-skip-requires")]
enum TestSkipRequires {
    #[clap_mcp_output_literal = "exposed"]
    Exposed,
    #[clap_mcp(skip)]
    #[clap_mcp_output_literal = "hidden"]
    Hidden,
    #[clap_mcp_output = "format!(\"path: {:?}\", path)"]
    Read {
        #[clap_mcp(requires)]
        #[arg(long)]
        path: Option<String>,
    },
    /// Variant-level requires: path and input become required in MCP
    #[clap_mcp(requires = "path, input")]
    #[clap_mcp_output = "format!(\"path={}, input={}\", clap_mcp::opt_str(&path, \"\"), clap_mcp::opt_str(&input, \"\"))"]
    Process {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        input: Option<String>,
    },
    /// Single optional positional made required in MCP via variant-level requires = "versions"
    #[clap_mcp(requires = "versions")]
    #[clap_mcp_output = "format!(\"versions: {:?}\", versions)"]
    Sort {
        versions: Option<String>,
    },
}

#[test]
fn test_clap_mcp_skip_command() {
    let cmd = TestSkipRequires::command();
    let metadata = TestSkipRequires::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let commands = schema.root.all_commands();
    let names: Vec<_> = commands.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"exposed"));
    assert!(names.contains(&"read"));
    assert!(!names.contains(&"hidden"));
}

#[test]
fn test_clap_mcp_skip_root_struct_field() {
    let cmd = TestRootSkip::command();
    let metadata = TestRootSkip::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    // Root command should not include the skipped "out" arg in MCP schema
    let root = &schema.root;
    assert_eq!(root.name, "test-root-skip");
    let out_arg = root.args.iter().find(|a| a.id == "out");
    assert!(
        out_arg.is_none(),
        "root-level #[clap_mcp(skip)] field 'out' should be excluded from MCP schema"
    );
}

#[test]
fn test_skip_root_command_when_subcommands() {
    let cmd = TestStructOptionalCli::command();
    let mut metadata = TestStructOptionalCli::clap_mcp_schema_metadata();
    metadata.skip_root_command_when_subcommands = true;
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_config_and_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        !names.contains(&"test-struct-optional-cli"),
        "root should be excluded when skip_root_command_when_subcommands is true"
    );
    assert!(
        names.contains(&"done"),
        "subcommand 'done' should still be in tool list"
    );
}

#[test]
fn test_skip_root_when_subcommands_derive() {
    let cmd = TestRootSkipWhenSubcommands::command();
    let metadata = TestRootSkipWhenSubcommands::clap_mcp_schema_metadata();
    assert!(
        metadata.skip_root_command_when_subcommands,
        "derive with #[clap_mcp(skip_root_when_subcommands)] should set the flag"
    );
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_config_and_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        !names.contains(&"test-root-skip-when-subcommands"),
        "root should be excluded when using #[clap_mcp(skip_root_when_subcommands)]"
    );
    assert!(
        names.contains(&"done"),
        "subcommand 'done' should still be in tool list"
    );
}

#[test]
fn test_clap_mcp_requires_arg() {
    let cmd = TestSkipRequires::command();
    let metadata = TestSkipRequires::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let commands = schema.root.all_commands();
    let read_cmd = commands
        .iter()
        .find(|c| c.name == "read")
        .expect("read command");
    let path_arg = read_cmd
        .args
        .iter()
        .find(|a| a.id == "path")
        .expect("path arg");
    assert!(path_arg.required, "path should be required in MCP schema");
}

#[test]
fn test_clap_mcp_requires_arg_single_positional() {
    let cmd = TestSkipRequires::command();
    let metadata = TestSkipRequires::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let commands = schema.root.all_commands();
    let sort_cmd = commands
        .iter()
        .find(|c| c.name == "sort")
        .expect("sort command");
    let versions_arg = sort_cmd
        .args
        .iter()
        .find(|a| a.id == "versions")
        .expect("versions arg");
    assert!(
        versions_arg.required,
        "variant-level #[clap_mcp(requires = \"versions\")] should mark versions required in MCP schema"
    );
}

// --- #[clap_mcp_output_result] Result<T, E> support ---

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(name = "test-cli-result")]
enum TestCliResult {
    #[clap_mcp_output_result]
    #[clap_mcp_output = "if n >= 0 { Ok(format!(\"sqrt ~{}\", n)) } else { Err(format!(\"negative: {}\", n)) }"]
    Sqrt {
        #[arg(long)]
        n: i32,
    },
    #[clap_mcp_output_result]
    #[clap_mcp_output = "Ok::<_, String>(format!(\"double: {}\", x * 2))"]
    Double {
        #[arg(long)]
        x: i32,
    },
}

#[derive(Debug, Serialize)]
struct MyError {
    code: i32,
    msg: String,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(name = "test-cli-result-structured-error")]
enum TestCliResultStructuredError {
    #[clap_mcp_output_result]
    #[clap_mcp_error_type = "MyError"]
    #[clap_mcp_output = "if x > 0 { Ok(format!(\"ok: {}\", x)) } else { Err(MyError { code: -1, msg: format!(\"invalid: {}\", x) }) }"]
    Check {
        #[arg(long)]
        x: i32,
    },
}

#[test]
fn test_clap_mcp_output_result_ok() {
    let cli = TestCliResult::Sqrt { n: 42 };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some("sqrt ~42"));
}

#[test]
fn test_clap_mcp_output_result_err() {
    let cli = TestCliResult::Sqrt { n: -1 };
    let err = cli.execute_for_mcp().expect_err("should fail");
    assert!(err.message.contains("negative"));
    assert!(err.message.contains("-1"));
    assert!(err.structured.is_none());
}

#[test]
fn test_clap_mcp_output_result_double_ok() {
    let cli = TestCliResult::Double { x: 21 };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some("double: 42"));
}

#[test]
fn test_clap_mcp_output_result_structured_error_ok() {
    let cli = TestCliResultStructuredError::Check { x: 10 };
    let out = cli.execute_for_mcp().expect("should succeed");
    assert_eq!(out.as_text(), Some("ok: 10"));
}

#[test]
fn test_clap_mcp_output_result_structured_error_err() {
    let cli = TestCliResultStructuredError::Check { x: -5 };
    let err = cli.execute_for_mcp().expect_err("should fail");
    assert!(err.message.contains("invalid: -5"));
    let structured = err.structured.expect("should have structured error");
    assert_eq!(structured.get("code").and_then(|v| v.as_i64()), Some(-1));
    assert_eq!(
        structured.get("msg").and_then(|v| v.as_str()),
        Some("invalid: -5")
    );
}

#[test]
fn test_clap_mcp_requires_variant() {
    let cmd = TestSkipRequires::command();
    let metadata = TestSkipRequires::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let commands = schema.root.all_commands();
    let process_cmd = commands
        .iter()
        .find(|c| c.name == "process")
        .expect("process command");
    for arg_id in ["path", "input"] {
        let arg = process_cmd
            .args
            .iter()
            .find(|a| a.id == arg_id)
            .expect(arg_id);
        assert!(
            arg.required,
            "{} should be required in MCP schema (variant-level requires)",
            arg_id
        );
    }
}

// --- output_schema (output_type / output_one_of) when feature "output-schema" is enabled ---

#[cfg(feature = "output-schema")]
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct OutputSchemaTestType {
    value: i32,
}

#[cfg(feature = "output-schema")]
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_type = "OutputSchemaTestType"]
#[command(name = "test-cli-output-schema")]
enum TestCliOutputSchema {
    Foo { _x: i32 },
}

#[cfg(feature = "output-schema")]
#[test]
fn test_output_schema_metadata_set() {
    let metadata = TestCliOutputSchema::clap_mcp_schema_metadata();
    assert!(
        metadata.output_schema.is_some(),
        "with output-schema feature and output_type, metadata.output_schema should be set"
    );
}

#[cfg(feature = "output-schema")]
#[test]
fn test_tools_from_schema_with_metadata_output_schema() {
    let metadata = TestCliOutputSchema::clap_mcp_schema_metadata();
    let cmd = TestCliOutputSchema::command();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_config_and_metadata(&schema, &config, &metadata);
    for tool in &tools {
        assert!(
            tool.output_schema.is_some(),
            "tool {} should have output_schema when metadata has it",
            tool.name
        );
    }
}
