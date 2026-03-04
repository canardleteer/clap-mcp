//! Example: custom MCP resources and prompts, and --export-skills.
//!
//! Run:
//! - `cargo run -p clap-mcp-examples --bin custom_resources_prompts -- --help`
//! - `cargo run -p clap-mcp-examples --bin custom_resources_prompts -- --mcp` (MCP server with extra resource and prompt)
//! - `cargo run -p clap-mcp-examples --bin custom_resources_prompts -- --export-skills` (generate SKILL.md into .agent/skills/)
//! - `cargo run -p clap-mcp-examples --bin custom_resources_prompts -- --export-skills=./out` (generate into ./out)

use clap::Parser;
use clap_mcp::content::{CustomPrompt, CustomResource, PromptContent, ResourceContent};
use clap_mcp::{ClapMcp, ClapMcpServeOptions};
use rust_mcp_sdk::schema::{ContentBlock, PromptMessage, Role};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "custom-resources-prompts",
    about = "Example with custom MCP resources and prompts, and --export-skills",
    subcommand_required = false
)]
enum Cli {
    /// Echo a message.
    Echo {
        #[arg(long)]
        message: Option<String>,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Echo { message } => message.as_deref().unwrap_or("(none)").to_string(),
    }
}

fn main() {
    let mut serve_options = ClapMcpServeOptions::default();

    serve_options.custom_resources.push(CustomResource {
        uri: "example://readme".into(),
        name: "readme".into(),
        title: Some("Example readme".into()),
        description: Some("Static readme content for this example".into()),
        mime_type: Some("text/markdown".into()),
        content: ResourceContent::Static(
            "# Custom resources & prompts example\n\nUse `--mcp` to list/read resources and list/get prompts.\n"
                .into(),
        ),
    });

    serve_options.custom_prompts.push(CustomPrompt {
        name: "example-prompt".into(),
        title: Some("Example prompt".into()),
        description: Some("A static prompt that returns a short instruction".into()),
        arguments: vec![],
        content: PromptContent::Static(vec![PromptMessage {
            content: ContentBlock::text_content(
                "When using this example CLI via MCP, prefer the echo tool for simple text.".into(),
            ),
            role: Role::User,
        }]),
    });

    let config = clap_mcp::ClapMcpConfig::default();
    let cli = clap_mcp::parse_or_serve_mcp_with_config_and_options::<Cli>(config, serve_options);

    match cli {
        Cli::Echo { message } => {
            println!("{}", message.as_deref().unwrap_or("(none)"));
        }
    }
}
