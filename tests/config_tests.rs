//! Tests for ClapMcpConfig and configuration possibilities.

use clap::{CommandFactory, Parser};
use clap_mcp::{ClapMcpConfig, ClapMcpConfigProvider, ClapMcpRunnable, tools_from_schema_with_config, schema_from_command};

#[derive(Debug, Parser, clap_mcp::ClapMcp)]
#[clap_mcp(parallel_safe = false, reinvocation_safe = false)]
#[command(name = "test-cli")]
enum TestCliDefaults {
    Foo,
}

#[derive(Debug, Parser, clap_mcp::ClapMcp)]
#[clap_mcp(parallel_safe, reinvocation_safe)]
#[command(name = "test-cli-both-true")]
enum TestCliBothTrue {
    Bar,
}

#[derive(Debug, Parser, clap_mcp::ClapMcp)]
#[clap_mcp(parallel_safe = true, reinvocation_safe = false)]
#[command(name = "test-cli-parallel-only")]
enum TestCliParallelOnly {
    Baz,
}

#[derive(Debug, Parser, clap_mcp::ClapMcp)]
#[clap_mcp(parallel_safe = false, reinvocation_safe)]
#[command(name = "test-cli-reinvoke-only")]
enum TestCliReinvokeOnly {
    #[clap_mcp_output = "format!(\"result: {}\", x)"]
    Qux { x: i32 },
}

#[test]
fn test_config_default() {
    let config = ClapMcpConfig::default();
    assert!(!config.reinvocation_safe, "reinvocation_safe should default to false");
    assert!(!config.parallel_safe, "parallel_safe should default to false");
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
fn test_tools_from_schema_with_config_meta() {
    let cmd = TestCliDefaults::command();
    let schema = schema_from_command(&cmd);

    let config_false_false = ClapMcpConfig { reinvocation_safe: false, parallel_safe: false };
    let tools = tools_from_schema_with_config(&schema, &config_false_false);
    assert!(!tools.is_empty());
    for tool in &tools {
        let meta = tool.meta.as_ref().expect("tool should have meta");
        let clap_mcp = meta.get("clapMcp").expect("meta should have clapMcp");
        let obj = clap_mcp.as_object().expect("clapMcp should be object");
        assert_eq!(obj.get("reinvocationSafe").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(obj.get("parallelSafe").and_then(|v| v.as_bool()), Some(false));
    }

    let config_true_true = ClapMcpConfig { reinvocation_safe: true, parallel_safe: true };
    let tools = tools_from_schema_with_config(&schema, &config_true_true);
    for tool in &tools {
        let meta = tool.meta.as_ref().expect("tool should have meta");
        let clap_mcp = meta.get("clapMcp").expect("meta should have clapMcp");
        let obj = clap_mcp.as_object().expect("clapMcp should be object");
        assert_eq!(obj.get("reinvocationSafe").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(obj.get("parallelSafe").and_then(|v| v.as_bool()), Some(true));
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
