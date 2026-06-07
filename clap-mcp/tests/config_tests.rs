//! Tests for ClapMcpConfig and configuration possibilities.

use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_mcp::AsStructured;
use clap_mcp::ClapMcp;
use clap_mcp::{
    ClapMcpConfig, ClapMcpConfigProvider, ClapMcpError, ClapMcpRunnable, ClapMcpSchemaMetadata,
    ClapMcpSchemaMetadataProvider, ClapMcpSerializeScope, ClapMcpToolExecutor, ClapMcpToolOutput,
    LOG_INTERPRETATION_INSTRUCTIONS, LOGGING_GUIDE_CONTENT, McpListen, PROMPT_LOGGING_GUIDE,
    ParseOrServeMcp, ServeMcpBuilder, run_async_tool, schema_from_command,
    schema_from_command_with_metadata, serve_mcp, tools_from_schema_with_metadata,
};
use serde::Serialize;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[clap_mcp_output_from = "run_defaults"]
#[command(name = "test-cli")]
enum TestCliDefaults {
    Foo,
}

fn run_defaults(cmd: TestCliDefaults) -> String {
    match cmd {
        TestCliDefaults::Foo => "foo".to_string(),
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe)]
#[clap_mcp_output_from = "run_both_true"]
#[command(name = "test-cli-both-true")]
enum TestCliBothTrue {
    Bar,
}

fn run_both_true(cmd: TestCliBothTrue) -> String {
    match cmd {
        TestCliBothTrue::Bar => "bar".to_string(),
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = true)]
#[clap_mcp_output_from = "run_parallel_only"]
#[command(name = "test-cli-parallel-only")]
enum TestCliParallelOnly {
    Baz,
}

fn run_parallel_only(cmd: TestCliParallelOnly) -> String {
    match cmd {
        TestCliParallelOnly::Baz => "baz".to_string(),
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_reinvoke_only"]
#[command(name = "test-cli-reinvoke-only")]
enum TestCliReinvokeOnly {
    Qux { x: i32 },
}

fn run_reinvoke_only(cmd: TestCliReinvokeOnly) -> String {
    match cmd {
        TestCliReinvokeOnly::Qux { x } => format!("result: {}", x),
    }
}

#[derive(Debug, Serialize)]
struct SubResult {
    difference: i32,
    minuend: i32,
    subtrahend: i32,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime = false)]
#[clap_mcp_output_from = "run_structured"]
#[command(name = "test-cli-structured")]
enum TestCliStructured {
    Sub {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
}

fn run_structured(cmd: TestCliStructured) -> AsStructured<SubResult> {
    match cmd {
        TestCliStructured::Sub { a, b } => AsStructured(SubResult {
            difference: a - b,
            minuend: a,
            subtrahend: b,
        }),
    }
}

// --- #[clap_mcp_output_from = "run"] ---

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "test-cli-output-from")]
enum TestCliOutputFrom {
    TextOut {
        x: i32,
    },
    OptionOut {
        present: bool,
    },
    ResultOk,
    ResultErr,
    StructuredOut {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
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
#[clap_mcp_output_from = "run_share_runtime"]
#[command(name = "test-cli-share-runtime")]
enum TestCliShareRuntime {
    Foo,
}

fn run_share_runtime(cmd: TestCliShareRuntime) -> String {
    match cmd {
        TestCliShareRuntime::Foo => "foo".to_string(),
    }
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
#[clap_mcp_output_from = "run_struct_commands"]
enum TestStructCommands {
    Add {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
}

fn run_struct_commands(cmd: TestStructCommands) -> String {
    match cmd {
        TestStructCommands::Add { a, b } => format!("sum: {}", a + b),
    }
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
#[clap_mcp_output_from = "run_struct_optional_commands"]
enum TestStructOptionalCommands {
    Done,
}

fn run_struct_optional_commands(cmd: TestStructOptionalCommands) -> String {
    match cmd {
        TestStructOptionalCommands::Done => "done".to_string(),
    }
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

// Struct root with task_augmented_tools and schema_only nested enum (no root field attrs)
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, task_augmented_tools)]
#[clap_mcp_output_from = "run_struct_task_augmented"]
#[command(name = "test-struct-task-augmented", subcommand_required = true)]
struct TestStructTaskAugmented {
    #[command(subcommand)]
    command: TestStructTaskAugmentedCommands,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum TestStructTaskAugmentedCommands {
    #[clap_mcp(task)]
    Work,
}

fn run_struct_task_augmented(cli: TestStructTaskAugmented) -> String {
    match cli.command {
        TestStructTaskAugmentedCommands::Work => "work".to_string(),
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(
    reinvocation_safe,
    parallel_safe = false,
    mcp_flag = "modelcontextprotocol"
)]
#[clap_mcp_output_from = "run_custom_mcp_flag"]
#[command(name = "test-cli-custom-mcp-flag")]
enum TestCliCustomMcpFlag {
    Ping,
}

fn run_custom_mcp_flag(cmd: TestCliCustomMcpFlag) -> String {
    match cmd {
        TestCliCustomMcpFlag::Ping => "pong".to_string(),
    }
}

#[test]
fn test_custom_mcp_flag_derive_sets_builtin_flags() {
    let config = TestCliCustomMcpFlag::clap_mcp_config();
    assert_eq!(config.builtin_flags.stdio_long, "modelcontextprotocol");
    assert_eq!(
        config.builtin_flags.export_skills_long,
        clap_mcp::EXPORT_SKILLS_FLAG_LONG
    );
}

#[test]
fn test_default_derive_builtin_flags_unchanged() {
    let config = TestCliDefaults::clap_mcp_config();
    assert_eq!(config.builtin_flags.stdio_long, clap_mcp::MCP_FLAG_LONG);
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
fn test_task_augmented_meta_on_tools() {
    let metadata = ClapMcpSchemaMetadata {
        task_augmented_tools: true,
        task_tool_names: vec!["test-cli".into()],
        ..Default::default()
    };
    let config = ClapMcpConfig {
        reinvocation_safe: true,
        parallel_safe: false,
        ..Default::default()
    };
    let cmd = TestCliDefaults::command();
    let schema = schema_from_command(&cmd);
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    assert!(!tools.is_empty());
    for tool in &tools {
        let meta = tool.meta.as_ref().expect("tool meta");
        let clap_mcp = meta.get("clapMcp").expect("clapMcp meta");
        let obj = clap_mcp.as_object().expect("clapMcp object");
        let eligible = metadata.task_tool_names.iter().any(|n| n == &tool.name);
        if eligible {
            assert_eq!(
                obj.get("taskAugmented").and_then(|v| v.as_bool()),
                Some(true)
            );
        } else {
            assert!(obj.get("taskAugmented").is_none());
        }
    }

    let metadata_off = ClapMcpSchemaMetadata {
        task_augmented_tools: false,
        ..Default::default()
    };
    let tools_off = tools_from_schema_with_metadata(&schema, &config, &metadata_off);
    for tool in &tools_off {
        let meta = tool.meta.as_ref().expect("tool meta");
        let clap_mcp = meta.get("clapMcp").expect("clapMcp meta");
        let obj = clap_mcp.as_object().expect("clapMcp object");
        assert!(obj.get("taskAugmented").is_none());
    }
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
    let tools = tools_from_schema_with_metadata(&schema, &config_false_false, &metadata);
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
    let tools = tools_from_schema_with_metadata(
        &schema,
        &config_true_true,
        &ClapMcpSchemaMetadata::default(),
    );
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
    let tools = tools_from_schema_with_metadata(
        &schema,
        &config_share_runtime,
        &ClapMcpSchemaMetadata::default(),
    );
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
    // With output_from, run_defaults returns "foo"
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
    let result = run_async_tool(&config, || async { 42 }).expect("runtime ok");
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
    let result = run_async_tool(&config, || async { 99 }).expect("runtime ok");
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
    let result = run_async_tool(&config, || async { "hello".to_string() }).expect("runtime ok");
    assert_eq!(result, "hello");
}

#[test]
fn test_run_async_tool_returns_complex_type() {
    let config = ClapMcpConfig::default();
    let result = run_async_tool(&config, || async { vec![1u8, 2, 3] }).expect("runtime ok");
    assert_eq!(result, vec![1, 2, 3]);
}

// Shared runtime path (reinvocation_safe=true + share_runtime=true) is exercised
// via integration: run the async_sleep example with share_runtime in #[clap_mcp].

#[test]
fn test_needs_multi_thread_runtime_matrix() {
    let cases = [
        (false, false, false, false),
        (false, true, true, false),
        (true, false, false, false),
        (true, true, false, true),
        (true, false, true, true),
        (true, true, true, true),
    ];
    for (reinvocation_safe, share_runtime, parallel_safe, expected) in cases {
        let config = ClapMcpConfig {
            reinvocation_safe,
            share_runtime,
            parallel_safe,
            ..Default::default()
        };
        assert_eq!(
            config.needs_multi_thread_runtime(),
            expected,
            "reinvocation_safe={reinvocation_safe} share_runtime={share_runtime} parallel_safe={parallel_safe}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_run_async_tool_shared_runtime_on_multi_thread_runtime() {
    let config = ClapMcpConfig {
        reinvocation_safe: true,
        parallel_safe: false,
        share_runtime: true,
        ..Default::default()
    };
    let result = tokio::task::block_in_place(|| {
        run_async_tool(&config, || async { 123 }).expect("shared runtime ok")
    });
    assert_eq!(result, 123);
}

#[test]
fn test_serve_mcp_builder_reports_missing_required_fields() {
    for (builder, field) in [
        (ServeMcpBuilder::new(), "listen"),
        (
            ServeMcpBuilder::new().listen(McpListen::Stdio),
            "schema_json",
        ),
        (
            ServeMcpBuilder::new()
                .listen(McpListen::Stdio)
                .schema_json("{}"),
            "config",
        ),
        (
            ServeMcpBuilder::new()
                .listen(McpListen::Stdio)
                .schema_json("{}")
                .config(ClapMcpConfig::default()),
            "metadata",
        ),
    ] {
        assert!(matches!(
            builder.build(),
            Err(ClapMcpError::InvalidConfig(message)) if message.contains(field)
        ));
    }
}

#[cfg(feature = "http")]
#[test]
fn test_serve_mcp_builder_http_listen_builds() {
    ServeMcpBuilder::new()
        .listen(McpListen::Http("127.0.0.1:0".parse().expect("addr")))
        .schema_json("{}")
        .config(ClapMcpConfig::default())
        .metadata(ClapMcpSchemaMetadata::default())
        .build()
        .expect("http listen should build");
}

#[test]
fn test_serve_mcp_rejects_current_thread_when_multi_thread_required() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let config = ClapMcpConfig {
        reinvocation_safe: true,
        share_runtime: true,
        ..Default::default()
    };
    let err = rt
        .block_on(serve_mcp(
            McpListen::Stdio,
            "{}".to_string(),
            None,
            config,
            None,
            Default::default(),
            &ClapMcpSchemaMetadata::default(),
        ))
        .expect_err("current_thread should be rejected");
    match err {
        ClapMcpError::RequiresMultiThreadRuntime { reason } => {
            assert!(reason.contains("multi-thread"));
        }
        other => panic!("expected RequiresMultiThreadRuntime, got {other:?}"),
    }
}

#[test]
fn test_serve_mcp_blocking_accepts_share_runtime_config() {
    // Smoke: blocking path builds runtime and fails fast on invalid schema (not on runtime).
    let config = ClapMcpConfig {
        reinvocation_safe: true,
        share_runtime: true,
        ..Default::default()
    };
    let err = ServeMcpBuilder::new()
        .listen(McpListen::Stdio)
        .schema_json("not-json".to_string())
        .config(config)
        .metadata(ClapMcpSchemaMetadata::default())
        .serve_blocking()
        .expect_err("invalid schema");
    assert!(matches!(err, ClapMcpError::SchemaJson(_)));
}

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
#[clap_mcp_output_from = "run_root_skip_commands"]
enum TestRootSkipCommands {
    Foo,
}

fn run_root_skip_commands(cmd: TestRootSkipCommands) -> String {
    match cmd {
        TestRootSkipCommands::Foo => "ok".to_string(),
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_skip_requires"]
#[command(name = "test-skip-requires")]
enum TestSkipRequires {
    Exposed,
    #[clap_mcp(skip)]
    Hidden,
    Read {
        #[clap_mcp(requires)]
        #[arg(long)]
        path: Option<String>,
    },
    /// Variant-level requires: path and input become required in MCP
    #[clap_mcp(requires = "path, input")]
    Process {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        input: Option<String>,
    },
    /// Single optional positional made required in MCP via variant-level requires = "versions"
    #[clap_mcp(requires = "versions")]
    Sort {
        versions: Option<String>,
    },
}

fn run_skip_requires(cmd: TestSkipRequires) -> String {
    match cmd {
        TestSkipRequires::Exposed => "exposed".to_string(),
        TestSkipRequires::Hidden => "hidden".to_string(),
        TestSkipRequires::Read { path } => format!("path: {:?}", path),
        TestSkipRequires::Process { path, input } => format!(
            "path={}, input={}",
            path.as_deref().unwrap_or(""),
            input.as_deref().unwrap_or("")
        ),
        TestSkipRequires::Sort { versions } => format!("versions: {:?}", versions),
    }
}

#[derive(Debug, Args)]
struct SkippedFlattenedArgs {
    #[arg(long)]
    alpha: Option<String>,
    #[arg(long)]
    beta: Option<String>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_flat_skip_flatten"]
#[command(name = "test-flat-skip-flatten")]
struct TestFlatSkipFlatten {
    #[command(flatten)]
    #[clap_mcp(skip)]
    hidden: SkippedFlattenedArgs,
    #[arg(long)]
    visible: Option<String>,
}

fn run_flat_skip_flatten(cmd: TestFlatSkipFlatten) -> String {
    format!("visible={:?}", cmd.visible)
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_explicit_skip_ids"]
#[command(name = "test-explicit-skip-ids")]
struct TestExplicitSkipIds {
    #[clap_mcp(skip = "foo,bar")]
    #[arg(long)]
    foo: Option<String>,
    #[arg(long)]
    bar: Option<String>,
    #[arg(long)]
    kept: Option<String>,
}

fn run_explicit_skip_ids(cmd: TestExplicitSkipIds) -> String {
    format!("foo={:?}, bar={:?}, kept={:?}", cmd.foo, cmd.bar, cmd.kept)
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run_serialized_metadata"]
#[command(name = "test-serialized-metadata")]
enum TestSerializedMetadata {
    #[clap_mcp(serialized)]
    Whole,
    #[clap_mcp(serialized = "target")]
    Scoped {
        #[clap_mcp(serialize_topic)]
        #[arg(long)]
        target: Option<String>,
    },
}

fn run_serialized_metadata(cmd: TestSerializedMetadata) -> String {
    match cmd {
        TestSerializedMetadata::Whole => "whole".to_string(),
        TestSerializedMetadata::Scoped { target } => format!("scoped: {target:?}"),
    }
}

#[test]
fn test_clap_mcp_serialized_metadata() {
    let metadata = TestSerializedMetadata::clap_mcp_schema_metadata();
    assert_eq!(
        metadata.serialize_tools.get("whole"),
        Some(&ClapMcpSerializeScope::Tool)
    );
    assert_eq!(
        metadata.serialize_tools.get("scoped"),
        Some(&ClapMcpSerializeScope::Args(vec!["target".into()]))
    );

    let config = ClapMcpConfig {
        reinvocation_safe: true,
        parallel_safe: true,
        ..Default::default()
    };
    let cmd = TestSerializedMetadata::command();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let whole = tools
        .iter()
        .find(|t| t.name == "whole")
        .expect("whole tool");
    let scoped = tools
        .iter()
        .find(|t| t.name == "scoped")
        .expect("scoped tool");
    let whole_meta = whole
        .meta
        .as_ref()
        .and_then(|m| m.get("clapMcp"))
        .and_then(|v| v.as_object())
        .expect("whole clapMcp meta");
    assert_eq!(
        whole_meta.get("serialized").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        whole_meta.get("serializeScope").and_then(|v| v.as_str()),
        Some("tool")
    );
    let scoped_meta = scoped
        .meta
        .as_ref()
        .and_then(|m| m.get("clapMcp"))
        .and_then(|v| v.as_object())
        .expect("scoped clapMcp meta");
    assert_eq!(
        scoped_meta.get("serializeScope").and_then(|v| v.as_str()),
        Some("args")
    );
    assert_eq!(
        scoped_meta.get("serializeArgs").and_then(|v| v.as_array()),
        Some(&vec![serde_json::json!("target")])
    );
    assert_eq!(
        scoped_meta
            .get("serializeTopicArgs")
            .and_then(|v| v.as_array()),
        Some(&vec![serde_json::json!("target")])
    );
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
fn test_clap_mcp_schema_metadata_merge_from() {
    let mut base = ClapMcpSchemaMetadata::default();
    let mut child = ClapMcpSchemaMetadata::default();
    child.skip_commands.push("connect".into());
    child
        .requires_args
        .insert("create".into(), vec!["name".into()]);
    child
        .serialize_tools
        .insert("create".into(), ClapMcpSerializeScope::Tool);
    base.merge_from(child);
    assert!(base.skip_commands.contains(&"connect".to_string()));
    assert_eq!(
        base.requires_args.get("create").map(|v| v.as_slice()),
        Some(["name".to_string()].as_slice())
    );
    assert_eq!(
        base.serialize_tools.get("create"),
        Some(&ClapMcpSerializeScope::Tool)
    );

    let mut override_local = ClapMcpSchemaMetadata::default();
    override_local.serialize_tools.insert(
        "create".into(),
        ClapMcpSerializeScope::Args(vec!["name".into()]),
    );
    base.merge_from(override_local);
    assert_eq!(
        base.serialize_tools.get("create"),
        Some(&ClapMcpSerializeScope::Args(vec!["name".into()]))
    );
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
fn test_skip_flattened_args_excludes_all_arg_ids() {
    let cmd = TestFlatSkipFlatten::command();
    let metadata = TestFlatSkipFlatten::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let root = &schema.root;
    assert_eq!(root.name, "test-flat-skip-flatten");
    assert!(
        root.args.iter().all(|a| a.id != "alpha" && a.id != "beta"),
        "flattened #[clap_mcp(skip)] should exclude every arg id from the flattened Args type, got: {:?}",
        root.args.iter().map(|a| a.id.as_str()).collect::<Vec<_>>()
    );
    assert!(
        root.args.iter().any(|a| a.id == "visible"),
        "non-skipped root field should remain in MCP schema"
    );
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_custom_id_skip"]
#[command(name = "test-custom-id-skip")]
struct TestCustomIdSkip {
    #[clap_mcp(skip)]
    #[arg(id = "custom-out", long = "custom-out")]
    output_path: Option<String>,
    #[arg(long)]
    visible: Option<String>,
}

fn run_custom_id_skip(cmd: TestCustomIdSkip) -> String {
    format!("visible={:?}", cmd.visible)
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_custom_id_requires"]
#[command(name = "test-custom-id-requires")]
enum TestCustomIdRequires {
    Read {
        #[clap_mcp(requires)]
        #[arg(id = "custom-path", long = "custom-path")]
        file_path: Option<String>,
    },
}

fn run_custom_id_requires(cmd: TestCustomIdRequires) -> String {
    match cmd {
        TestCustomIdRequires::Read { file_path } => file_path.unwrap_or_default(),
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run_custom_id_serialized"]
#[command(name = "test-custom-id-serialized")]
enum TestCustomIdSerialized {
    #[clap_mcp(serialized = "custom-out")]
    Flush {
        #[clap_mcp(serialize_topic)]
        #[arg(id = "custom-out", long = "custom-out")]
        output_path: Option<String>,
    },
}

fn run_custom_id_serialized(cmd: TestCustomIdSerialized) -> String {
    match cmd {
        TestCustomIdSerialized::Flush { output_path } => format!("{output_path:?}"),
    }
}

#[test]
fn test_skip_custom_clap_arg_id() {
    let cmd = TestCustomIdSkip::command();
    let metadata = TestCustomIdSkip::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let root = &schema.root;
    assert!(
        root.args.iter().all(|a| a.id != "custom-out"),
        "skip should use clap arg id custom-out, not field name output_path"
    );
    assert!(
        metadata
            .skip_args
            .get("test-custom-id-skip")
            .is_some_and(|ids| ids.contains(&"custom-out".to_string())),
        "skip_args should contain custom-out: {:?}",
        metadata.skip_args
    );
}

#[test]
fn test_requires_custom_clap_arg_id() {
    let cmd = TestCustomIdRequires::command();
    let metadata = TestCustomIdRequires::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let read_cmd = schema
        .root
        .subcommands
        .iter()
        .find(|c| c.name == "read")
        .expect("read subcommand");
    let arg = read_cmd
        .args
        .iter()
        .find(|a| a.id == "custom-path")
        .expect("custom-path arg in schema");
    assert!(
        arg.required,
        "requires on custom-id field should mark custom-path required in MCP schema"
    );
    assert!(
        metadata
            .requires_args
            .get("read")
            .is_some_and(|ids| ids.contains(&"custom-path".to_string()))
    );
}

#[test]
fn test_serialize_topic_custom_clap_arg_id() {
    let metadata = TestCustomIdSerialized::clap_mcp_schema_metadata();
    assert_eq!(
        metadata.serialize_tools.get("flush"),
        Some(&ClapMcpSerializeScope::Args(vec!["custom-out".into()]))
    );
    let topic_args = metadata
        .serialize_topic_args
        .get("flush")
        .expect("serialize_topic_args for flush");
    assert!(
        topic_args.contains_key("custom-out"),
        "serialize_topic key should be clap arg id custom-out, got: {:?}",
        topic_args.keys().collect::<Vec<_>>()
    );
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum HiddenSubcommands {
    #[command(name = "hidden-a")]
    HiddenA,
    #[command(name = "hidden-b")]
    HiddenB,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_flat_skip_flatten_subcommands"]
#[command(name = "test-flat-skip-flatten-subcommands")]
struct TestFlatSkipFlattenSubcommands {
    #[command(subcommand)]
    #[clap_mcp(skip)]
    commands: HiddenSubcommands,
    #[arg(long)]
    visible: Option<String>,
}

fn run_flat_skip_flatten_subcommands(cmd: TestFlatSkipFlattenSubcommands) -> String {
    format!("visible={:?}", cmd.visible)
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum NestedBuildActions {
    #[command(name = "compile")]
    Compile,
    #[command(name = "link")]
    Link,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum NestedOuterCommands {
    #[command(name = "build")]
    Build {
        #[command(subcommand)]
        action: NestedBuildActions,
    },
    #[command(name = "clean")]
    Clean,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_nested_skip_flatten_subcommands"]
#[command(name = "test-nested-skip-flatten-subcommands")]
struct TestNestedSkipFlattenSubcommands {
    #[command(subcommand)]
    #[clap_mcp(skip)]
    commands: NestedOuterCommands,
}

fn run_nested_skip_flatten_subcommands(cmd: TestNestedSkipFlattenSubcommands) -> String {
    format!("{cmd:?}")
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum ExplicitHiddenSubcommands {
    #[command(name = "hidden")]
    Hidden,
    #[command(name = "also_hidden")]
    AlsoHidden,
    #[command(name = "kept")]
    Kept,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_explicit_skip_subcommand_names"]
#[command(name = "test-explicit-skip-subcommand-names")]
struct TestExplicitSkipSubcommandNames {
    #[command(subcommand)]
    #[clap_mcp(skip = "hidden,also_hidden")]
    commands: ExplicitHiddenSubcommands,
}

fn run_explicit_skip_subcommand_names(cmd: TestExplicitSkipSubcommandNames) -> String {
    format!("{cmd:?}")
}

#[derive(Debug, Args, ClapMcp)]
#[clap_mcp(args_metadata)]
struct FlushTopicArgs {
    #[clap_mcp(serialize_topic)]
    #[arg(long)]
    output: Option<String>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run_nested_flatten_serialize_topic"]
#[command(name = "test-nested-flatten-serialize-topic")]
enum TestNestedFlattenSerializeTopic {
    #[clap_mcp(serialized = "output")]
    Flush {
        #[command(flatten)]
        args: FlushTopicArgs,
    },
}

fn run_nested_flatten_serialize_topic(cmd: TestNestedFlattenSerializeTopic) -> String {
    match cmd {
        TestNestedFlattenSerializeTopic::Flush { args } => format!("output={:?}", args.output),
    }
}

#[derive(Debug, Args, ClapMcp)]
#[clap_mcp(args_metadata)]
struct InnerTopicArgs {
    #[clap_mcp(serialize_topic)]
    #[arg(long)]
    topic: Option<String>,
}

#[derive(Debug, Args, ClapMcp)]
#[clap_mcp(args_metadata)]
struct OuterTopicArgs {
    #[command(flatten)]
    inner: InnerTopicArgs,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run_two_level_flatten_serialize_topic"]
#[command(name = "test-two-level-flatten-serialize-topic")]
enum TestTwoLevelFlattenSerializeTopic {
    #[clap_mcp(serialized = "topic")]
    Run {
        #[command(flatten)]
        args: OuterTopicArgs,
    },
}

fn run_two_level_flatten_serialize_topic(cmd: TestTwoLevelFlattenSerializeTopic) -> String {
    match cmd {
        TestTwoLevelFlattenSerializeTopic::Run { args } => {
            format!("topic={:?}", args.inner.topic)
        }
    }
}

#[derive(Debug, Args, ClapMcp)]
#[clap_mcp(args_metadata)]
struct SharedTopicArgs {
    #[clap_mcp(serialize_topic)]
    #[arg(long)]
    key: Option<String>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run_shared_args_serialize_topic"]
#[command(name = "test-shared-args-serialize-topic")]
enum TestSharedArgsSerializeTopic {
    #[clap_mcp(serialized = "key")]
    Alpha {
        #[command(flatten)]
        shared: SharedTopicArgs,
    },
    #[clap_mcp(serialized = "key")]
    Beta {
        #[command(flatten)]
        shared: SharedTopicArgs,
    },
}

fn run_shared_args_serialize_topic(cmd: TestSharedArgsSerializeTopic) -> String {
    match cmd {
        TestSharedArgsSerializeTopic::Alpha { shared } => format!("alpha key={:?}", shared.key),
        TestSharedArgsSerializeTopic::Beta { shared } => format!("beta key={:?}", shared.key),
    }
}

#[test]
fn test_skip_flattened_subcommands_excludes_all_command_names() {
    let cmd = TestFlatSkipFlattenSubcommands::command();
    let metadata = TestFlatSkipFlattenSubcommands::clap_mcp_schema_metadata();
    assert!(
        metadata.skip_commands.iter().any(|s| s == "hidden-a"),
        "skip_commands should include hidden-a: {:?}",
        metadata.skip_commands
    );
    assert!(
        metadata.skip_commands.iter().any(|s| s == "hidden-b"),
        "skip_commands should include hidden-b: {:?}",
        metadata.skip_commands
    );
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(
        !names.contains(&"hidden-a") && !names.contains(&"hidden-b"),
        "flattened #[clap_mcp(skip)] on Subcommand should hide subcommand tools, got: {names:?}"
    );
    assert_eq!(schema.root.name, "test-flat-skip-flatten-subcommands");
    assert!(
        schema.root.args.iter().any(|a| a.id == "visible"),
        "non-skipped root field should remain in MCP schema"
    );
}

#[test]
fn test_skip_flattened_subcommands_nested_depth() {
    let metadata = TestNestedSkipFlattenSubcommands::clap_mcp_schema_metadata();
    for name in ["build", "compile", "link", "clean"] {
        assert!(
            metadata.skip_commands.iter().any(|s| s == name),
            "nested flatten subcommand skip should include {name}: {:?}",
            metadata.skip_commands
        );
    }
    let cmd = TestNestedSkipFlattenSubcommands::command();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    for hidden in ["build", "compile", "link", "clean"] {
        assert!(
            !names.contains(&hidden),
            "tool {hidden} should be skipped, got: {names:?}"
        );
    }
}

#[test]
fn test_skip_explicit_subcommand_name_list() {
    let metadata = TestExplicitSkipSubcommandNames::clap_mcp_schema_metadata();
    assert!(metadata.skip_commands.contains(&"hidden".to_string()));
    assert!(metadata.skip_commands.contains(&"also_hidden".to_string()));
    assert!(!metadata.skip_commands.contains(&"kept".to_string()));
    let cmd = TestExplicitSkipSubcommandNames::command();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"kept"));
    assert!(!names.contains(&"hidden"));
    assert!(!names.contains(&"also_hidden"));
}

#[test]
fn test_nested_flatten_args_serialize_topic_metadata() {
    let metadata = TestNestedFlattenSerializeTopic::clap_mcp_schema_metadata();
    let bindings = metadata
        .serialize_topic_args
        .get("flush")
        .expect("flush tool serialize_topic bindings");
    assert!(bindings.contains_key("output"));
    let config = ClapMcpConfig {
        reinvocation_safe: true,
        parallel_safe: true,
        ..Default::default()
    };
    let cmd = TestNestedFlattenSerializeTopic::command();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let flush = tools
        .iter()
        .find(|t| t.name == "flush")
        .expect("flush tool");
    let meta = flush
        .meta
        .as_ref()
        .and_then(|m| m.get("clapMcp"))
        .and_then(|v| v.as_object())
        .expect("flush clapMcp meta");
    assert_eq!(
        meta.get("serializeTopicArgs").and_then(|v| v.as_array()),
        Some(&vec![serde_json::json!("output")])
    );
}

#[test]
fn test_nested_flatten_args_serialize_topic_two_level() {
    let metadata = TestTwoLevelFlattenSerializeTopic::clap_mcp_schema_metadata();
    let bindings = metadata
        .serialize_topic_args
        .get("run")
        .expect("run tool serialize_topic bindings");
    assert!(bindings.contains_key("topic"));
}

#[test]
fn test_nested_flatten_args_serialized_validation_compiles() {
    let metadata = TestNestedFlattenSerializeTopic::clap_mcp_schema_metadata();
    assert_eq!(
        metadata.serialize_tools.get("flush"),
        Some(&ClapMcpSerializeScope::Args(vec!["output".into()]))
    );
}

#[test]
fn test_shared_args_type_two_variants_serialize_topic() {
    let metadata = TestSharedArgsSerializeTopic::clap_mcp_schema_metadata();
    for tool in ["alpha", "beta"] {
        let bindings = metadata
            .serialize_topic_args
            .get(tool)
            .unwrap_or_else(|| panic!("{tool} serialize_topic bindings"));
        assert!(
            bindings.contains_key("key"),
            "{tool} should bind serialize_topic for key"
        );
    }
}

#[test]
fn test_skip_explicit_arg_id_list() {
    let cmd = TestExplicitSkipIds::command();
    let metadata = TestExplicitSkipIds::clap_mcp_schema_metadata();
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let root = &schema.root;
    assert!(
        root.args.iter().all(|a| a.id != "foo" && a.id != "bar"),
        "#[clap_mcp(skip = \"foo,bar\")] should exclude both listed arg ids, got: {:?}",
        root.args.iter().map(|a| a.id.as_str()).collect::<Vec<_>>()
    );
    assert!(
        root.args.iter().any(|a| a.id == "kept"),
        "unlisted arg should remain in MCP schema"
    );
}

#[test]
fn test_skip_root_command_when_subcommands() {
    let cmd = TestStructOptionalCli::command();
    let mut metadata = TestStructOptionalCli::clap_mcp_schema_metadata();
    metadata.skip_root_command_when_subcommands = true;
    let schema = schema_from_command_with_metadata(&cmd, &metadata);
    let config = ClapMcpConfig::default();
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
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
fn test_struct_root_task_augmented_tools_metadata_delegate() {
    let metadata = TestStructTaskAugmented::clap_mcp_schema_metadata();
    assert!(
        metadata.task_augmented_tools,
        "struct-root #[clap_mcp(task_augmented_tools)] must apply on metadata delegate path without root field attrs"
    );
    assert!(
        metadata.task_tool_names.contains(&"work".to_string()),
        "task variant names should merge from nested enum: {:?}",
        metadata.task_tool_names
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
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
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

// --- #[clap_mcp_output_from] Result<T, E> support ---

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_result"]
#[command(name = "test-cli-result")]
enum TestCliResult {
    Sqrt {
        #[arg(long)]
        n: i32,
    },
    Double {
        #[arg(long)]
        x: i32,
    },
}

fn run_result(cmd: TestCliResult) -> Result<String, String> {
    match cmd {
        TestCliResult::Sqrt { n } => {
            if n >= 0 {
                Ok(format!("sqrt ~{}", n))
            } else {
                Err(format!("negative: {}", n))
            }
        }
        TestCliResult::Double { x } => Ok(format!("double: {}", x * 2)),
    }
}

#[derive(Debug, Serialize)]
struct MyError {
    code: i32,
    msg: String,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_result_structured_error"]
#[command(name = "test-cli-result-structured-error")]
enum TestCliResultStructuredError {
    Check {
        #[arg(long)]
        x: i32,
    },
}

impl clap_mcp::IntoClapMcpToolError for MyError {
    fn into_tool_error(self) -> clap_mcp::ClapMcpToolError {
        clap_mcp::ClapMcpToolError::structured(
            format!("{:?}", self),
            serde_json::to_value(&self)
                .unwrap_or_else(|_| serde_json::Value::String(format!("{:?}", self))),
        )
    }
}

fn run_result_structured_error(cmd: TestCliResultStructuredError) -> Result<String, MyError> {
    match cmd {
        TestCliResultStructuredError::Check { x } => {
            if x > 0 {
                Ok(format!("ok: {}", x))
            } else {
                Err(MyError {
                    code: -1,
                    msg: format!("invalid: {}", x),
                })
            }
        }
    }
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
#[clap_mcp_output_from = "run_output_schema"]
#[clap_mcp_output_type = "OutputSchemaTestType"]
#[command(name = "test-cli-output-schema")]
enum TestCliOutputSchema {
    Foo { _x: i32 },
}

#[cfg(feature = "output-schema")]
fn run_output_schema(cmd: TestCliOutputSchema) -> String {
    match cmd {
        TestCliOutputSchema::Foo { _x } => "output_schema_test".to_string(),
    }
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
    let tools = tools_from_schema_with_metadata(&schema, &config, &metadata);
    for tool in &tools {
        assert!(
            tool.output_schema.is_some(),
            "tool {} should have output_schema when metadata has it",
            tool.name
        );
    }
}
