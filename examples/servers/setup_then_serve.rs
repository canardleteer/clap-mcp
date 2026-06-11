//! Embedder pattern: parse argv, load config, then start MCP on a `serve` subcommand.
//!
//! Run: `cargo run -p clap-mcp-examples --bin setup_then_serve -- ping`
//! Run: `cargo run -p clap-mcp-examples --bin setup_then_serve -- --config /tmp/app.toml serve`
//!
//! Unlike `parse_or_serve_mcp`, this path does not inject `--mcp` or intercept argv
//! before normal clap parsing. See docs/usage.md — Setup then serve (embedder).
//! For multiplexing MCP over an existing JSON-RPC pipe, chain
//! `ServeMcpBuilder::stdio_io(read, write)` before `.serve()` or `.serve_blocking()`.

use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, McpListen, ServeMcpBuilder};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from = "run_app"]
#[command(
    name = "setup-then-serve-example",
    about = "Parse config, then ServeMcpBuilder::for_cli on serve subcommand",
    subcommand_required = true
)]
struct App {
    #[arg(long, global = true, default_value = "/tmp/app.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Commands {
    /// Start MCP (embedder entry; not exposed as an MCP tool).
    #[clap_mcp(skip)]
    Serve,

    Ping,
}

fn run_app(app: App) -> String {
    match app.command {
        Commands::Serve => unreachable!("handled in main"),
        Commands::Ping => "pong".to_string(),
    }
}

fn load_config(path: &Path) {
    eprintln!("loaded config from {}", path.display());
}

fn main() -> Result<(), clap_mcp::ClapMcpError> {
    let app = App::parse();
    load_config(&app.config);

    match app.command {
        Commands::Serve => {
            ServeMcpBuilder::for_cli::<App>(McpListen::Stdio).serve_blocking()?;
        }
        cmd => println!(
            "{}",
            run_app(App {
                config: app.config,
                command: cmd
            })
        ),
    }
    Ok(())
}
