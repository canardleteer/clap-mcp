//! Tests for ClapMcpConfig and configuration possibilities.

use clap::{CommandFactory, Parser};
use clap_mcp::ClapMcp;
use clap_mcp::{
    ClapMcpConfig, ClapMcpConfigProvider, ClapMcpRunnable, ClapMcpToolExecutor, ClapMcpToolOutput,
    LOG_INTERPRETATION_INSTRUCTIONS, LOGGING_GUIDE_CONTENT, PROMPT_LOGGING_GUIDE, run_async_tool,
    schema_from_command, tools_from_schema_with_config,
};
use serde::Serialize;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(parallel_safe = false, reinvocation_safe = false)]
#[command(name = "test-cli")]
enum TestCliDefaults {
    Foo,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(parallel_safe, reinvocation_safe)]
#[command(name = "test-cli-both-true")]
enum TestCliBothTrue {
    Bar,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(parallel_safe = true, reinvocation_safe = false)]
#[command(name = "test-cli-parallel-only")]
enum TestCliParallelOnly {
    Baz,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(parallel_safe = false, reinvocation_safe)]
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
    #[clap_mcp_output_type = "SubResult"]
    #[clap_mcp_output = "SubResult { difference: a - b, minuend: a, subtrahend: b }"]
    Sub { a: i32, b: i32 },
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime)]
#[command(name = "test-cli-share-runtime")]
enum TestCliShareRuntime {
    Foo,
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
    let tools = tools_from_schema_with_config(&schema, &config_false_false);
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
    assert!(result.contains("Foo"));
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
    let out = sub.execute_for_mcp();
    let v = out.as_structured().expect("should be structured");
    assert_eq!(v.get("difference").and_then(|x| x.as_i64()), Some(7));
    assert_eq!(v.get("minuend").and_then(|x| x.as_i64()), Some(10));
    assert_eq!(v.get("subtrahend").and_then(|x| x.as_i64()), Some(3));
}

// --- run_async_tool and share_runtime edge cases ---

#[test]
fn test_run_async_tool_dedicated_thread_reinvocation_safe_false() {
    // When reinvocation_safe=false, always uses dedicated thread (share_runtime ignored)
    let config = ClapMcpConfig {
        reinvocation_safe: false,
        parallel_safe: false,
        share_runtime: true, // ignored
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
