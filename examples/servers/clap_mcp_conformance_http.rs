//! Maintainer conformance fixture only — not a user-facing demo.
//!
//! Used by `cargo xtask conformance` and the weekly MCP conformance workflow.
//! Combines HTTP serving, MCP logging, harness-aligned resources/prompts, and
//! reference tool names required by `@modelcontextprotocol/conformance` scenarios
//! that exercise clap-mcp's shipped capabilities (logging, text resources/prompts,
//! tool errors).

// Logging types remain functional in rmcp 2.x but are deprecated by SEP-2577.
#![allow(deprecated)]

#[cfg(all(feature = "tracing", feature = "http"))]
use async_trait::async_trait;
#[cfg(all(feature = "tracing", feature = "http"))]
use clap::Parser;
#[cfg(all(feature = "tracing", feature = "http"))]
use clap_mcp::content::{
    CustomPrompt, CustomResource, CustomResourceTemplate, PromptContent, PromptContentProvider,
    ResourceContent,
};
#[cfg(all(feature = "tracing", feature = "http"))]
use clap_mcp::logging::{ClapMcpTracingLayer, log_channel, log_params};
#[cfg(all(feature = "tracing", feature = "http"))]
use clap_mcp::{ClapMcp, ClapMcpConfigProvider, ClapMcpToolError, IntoClapMcpToolError};
#[cfg(all(feature = "tracing", feature = "http"))]
use rmcp::model::{ContentBlock, ImageContent, LoggingLevel, PromptArgument, PromptMessage, Role};
#[cfg(all(feature = "tracing", feature = "http"))]
use std::{sync::OnceLock, time::Duration};
#[cfg(all(feature = "tracing", feature = "http"))]
use tracing_subscriber::EnvFilter;
#[cfg(all(feature = "tracing", feature = "http"))]
use tracing_subscriber::Layer;
#[cfg(all(feature = "tracing", feature = "http"))]
use tracing_subscriber::layer::SubscriberExt;
#[cfg(all(feature = "tracing", feature = "http"))]
use tracing_subscriber::util::SubscriberInitExt;

#[cfg(all(feature = "tracing", feature = "http"))]
static LOG_TX: OnceLock<
    tokio::sync::mpsc::Sender<clap_mcp::logging::LoggingMessageNotificationParams>,
> = OnceLock::new();

#[cfg(all(feature = "tracing", feature = "http"))]
#[derive(Debug)]
enum ConformanceToolError {
    Intentional(String),
}

#[cfg(all(feature = "tracing", feature = "http"))]
impl IntoClapMcpToolError for ConformanceToolError {
    fn into_tool_error(self) -> ClapMcpToolError {
        match self {
            ConformanceToolError::Intentional(message) => ClapMcpToolError::text(message),
        }
    }
}

#[cfg(all(feature = "tracing", feature = "http"))]
struct PromptWithArgumentsProvider;

#[cfg(all(feature = "tracing", feature = "http"))]
#[async_trait]
impl PromptContentProvider for PromptWithArgumentsProvider {
    async fn get(
        &self,
        _name: &str,
        arguments: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Vec<PromptMessage>, Box<dyn std::error::Error + Send + Sync>> {
        let arg1 = arguments.get("arg1").and_then(|v| v.as_str()).unwrap_or("");
        let arg2 = arguments.get("arg2").and_then(|v| v.as_str()).unwrap_or("");
        Ok(vec![PromptMessage::new_text(
            Role::User,
            format!("Prompt with arguments: arg1='{arg1}', arg2='{arg2}'"),
        )])
    }
}

/// 1×1 transparent PNG (base64) for `prompts-get-with-image`.
#[cfg(all(feature = "tracing", feature = "http"))]
const CONFORMANCE_PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

#[cfg(all(feature = "tracing", feature = "http"))]
struct PromptWithEmbeddedResourceProvider;

#[cfg(all(feature = "tracing", feature = "http"))]
#[async_trait]
impl PromptContentProvider for PromptWithEmbeddedResourceProvider {
    async fn get(
        &self,
        _name: &str,
        arguments: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Vec<PromptMessage>, Box<dyn std::error::Error + Send + Sync>> {
        let resource_uri = arguments
            .get("resourceUri")
            .and_then(|v| v.as_str())
            .unwrap_or("test://example-resource");
        Ok(vec![
            PromptMessage::new_resource(
                Role::User,
                resource_uri.to_string(),
                Some("text/plain".into()),
                Some("Embedded resource content for testing.".into()),
                None,
                None,
                None,
            ),
            PromptMessage::new_text(Role::User, "Please process the embedded resource above."),
        ])
    }
}

#[cfg(all(feature = "tracing", feature = "http"))]
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime = true)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "clap-mcp-conformance-http",
    about = "Maintainer MCP conformance HTTP fixture (not a user demo)",
    subcommand_required = false
)]
enum Cli {
    /// Conformance harness: tool that returns simple text content.
    #[command(name = "test_simple_text")]
    SimpleText,

    /// Conformance harness: tool that emits MCP log notifications during execution.
    #[command(name = "test_tool_with_logging")]
    ToolWithLogging,

    /// Conformance harness: tool that always returns an MCP error result.
    #[command(name = "test_error_handling")]
    ErrorHandling,
}

#[cfg(all(feature = "tracing", feature = "http"))]
async fn send_conformance_log_async(message: &str) {
    let Some(tx) = LOG_TX.get() else {
        return;
    };
    let params = log_params(
        LoggingLevel::Info,
        Some("clap-mcp-conformance-http".into()),
        serde_json::json!({ "message": message }),
    );
    let _ = tx.send(params).await;
}

#[cfg(all(feature = "tracing", feature = "http"))]
fn run(cmd: Cli) -> Result<String, ConformanceToolError> {
    match cmd {
        Cli::SimpleText => Ok("This is a simple text response for testing.".into()),
        Cli::ToolWithLogging => Ok(clap_mcp::run_async_tool(&Cli::clap_mcp_config(), || async {
            send_conformance_log_async("Tool execution started").await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            send_conformance_log_async("Tool processing data").await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            send_conformance_log_async("Tool execution completed").await;
            "Tool with logging executed".to_string()
        })
        .map_err(|e| ConformanceToolError::Intentional(format!("async tool failed: {e}")))?),
        Cli::ErrorHandling => Err(ConformanceToolError::Intentional(
            "This tool intentionally returns an error for testing".into(),
        )),
    }
}

#[cfg(all(feature = "tracing", feature = "http"))]
fn conformance_serve_options() -> clap_mcp::ClapMcpServeOptions {
    let (log_tx, log_rx) = log_channel(32);
    let _ = LOG_TX.set(log_tx.clone());
    let layer = ClapMcpTracingLayer::new(log_tx).with_logger_name("clap-mcp-conformance-http");
    let stderr_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,clap_mcp=info"));
    tracing_subscriber::registry()
        .with(layer)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_filter(stderr_filter),
        )
        .init();

    let mut serve_options = clap_mcp::ClapMcpServeOptions {
        log_rx: Some(log_rx),
        #[cfg(unix)]
        capture_stdout: false,
        custom_resources: vec![],
        custom_resource_templates: vec![],
        custom_prompts: vec![],
    };

    serve_options.custom_resources.push(CustomResource {
        uri: "test://static-text".into(),
        name: "static-text".into(),
        title: Some("Static text (conformance)".into()),
        description: Some("Plain text resource for MCP conformance harness".into()),
        mime_type: Some("text/plain".into()),
        content: ResourceContent::Static("This is the content of the static text resource.".into()),
    });

    serve_options.custom_resources.push(CustomResource {
        uri: "test://static-binary".into(),
        name: "static-binary".into(),
        title: Some("Static binary (conformance)".into()),
        description: Some("PNG blob resource for MCP conformance harness".into()),
        mime_type: Some("image/png".into()),
        content: ResourceContent::StaticBlob {
            base64: CONFORMANCE_PNG_B64.into(),
        },
    });

    serve_options
        .custom_resource_templates
        .push(CustomResourceTemplate {
            uri_template: "test://template/{id}/data".into(),
            name: "template-data".into(),
            title: Some("Template data (conformance)".into()),
            description: Some("URI template resource for MCP conformance harness".into()),
            mime_type: Some("application/json".into()),
            content: ResourceContent::Static(
                r#"{"id":"{id}","templateTest":true,"data":"Data for ID: {id}"}"#.into(),
            ),
        });

    serve_options.custom_prompts.push(CustomPrompt {
        name: "test_simple_prompt".into(),
        title: Some("Simple conformance prompt".into()),
        description: Some("Simple prompt without arguments".into()),
        arguments: vec![],
        content: PromptContent::Static(vec![PromptMessage::new_text(
            Role::User,
            "This is a simple prompt for testing.",
        )]),
    });

    serve_options.custom_prompts.push(CustomPrompt {
        name: "test_prompt_with_arguments".into(),
        title: Some("Parameterized conformance prompt".into()),
        description: Some("Prompt with arg1 and arg2".into()),
        arguments: vec![
            PromptArgument::new("arg1")
                .with_description("First test argument")
                .with_required(true),
            PromptArgument::new("arg2")
                .with_description("Second test argument")
                .with_required(true),
        ],
        content: PromptContent::Dynamic(std::sync::Arc::new(PromptWithArgumentsProvider)),
    });

    // Custom prompts accept rmcp `PromptMessage` content blocks (including image /
    // embedded resource). The fixture exercises that pass-through for harness IDs
    // that match shipped custom-prompt API, not tool-result media.
    serve_options.custom_prompts.push(CustomPrompt {
        name: "test_prompt_with_image".into(),
        title: Some("Image conformance prompt".into()),
        description: Some("Prompt that includes an image content block".into()),
        arguments: vec![],
        content: PromptContent::Static(vec![
            PromptMessage::new(
                Role::User,
                ContentBlock::Image(ImageContent::new(CONFORMANCE_PNG_B64, "image/png")),
            ),
            PromptMessage::new_text(Role::User, "Please analyze the image above."),
        ]),
    });

    serve_options.custom_prompts.push(CustomPrompt {
        name: "test_prompt_with_embedded_resource".into(),
        title: Some("Embedded resource conformance prompt".into()),
        description: Some("Prompt that embeds a resource from resourceUri".into()),
        arguments: vec![
            PromptArgument::new("resourceUri")
                .with_description("URI of the resource to embed")
                .with_required(true),
        ],
        content: PromptContent::Dynamic(std::sync::Arc::new(PromptWithEmbeddedResourceProvider)),
    });

    serve_options
}

#[cfg(all(feature = "tracing", feature = "http"))]
fn main() {
    let _cli = clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions {
        config: Cli::clap_mcp_config(),
        serve: conformance_serve_options(),
    });
}

#[cfg(not(all(feature = "tracing", feature = "http")))]
fn main() {
    eprintln!("Requires features: http, tracing");
    std::process::exit(1);
}
