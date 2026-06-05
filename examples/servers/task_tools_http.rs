//! Task-augmented MCP tools over Streamable HTTP (`http` feature).
//!
//! Run: `cargo run -p clap-mcp-examples --bin task_tools_http --features http,tracing -- --mcp-http 127.0.0.1:8080`

mod task_tools_common;

#[cfg(all(feature = "tracing", feature = "http"))]
use clap_mcp::ClapMcpConfigProvider;

#[cfg(all(feature = "tracing", feature = "http"))]
use clap::Parser;
#[cfg(all(feature = "tracing", feature = "http"))]
use clap_mcp::ClapMcp;

#[cfg(all(feature = "tracing", feature = "http"))]
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, task_augmented_tools)]
#[clap_mcp_output_from = "run"]
#[command(name = "task-tools-http-example", subcommand_required = false)]
enum Cli {
    #[clap_mcp(task)]
    Sleep {
        #[arg(long, default_value_t = 50)]
        ms: u64,
    },
}

#[cfg(all(feature = "tracing", feature = "http"))]
fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Sleep { ms } => {
            clap_mcp::run_async_tool(&Cli::clap_mcp_config(), move || async move {
                task_tools_common::sleep_ms(ms).await
            })
            .unwrap_or_else(|e| format!("async error: {e}"))
        }
    }
}

#[cfg(all(feature = "tracing", feature = "http"))]
fn main() {
    let mut serve_options = task_tools_common::serve_options_with_logging();
    serve_options.elicitation_enabled = false;
    let _cli = clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions {
        config: Cli::clap_mcp_config(),
        serve: serve_options,
    });
}

#[cfg(not(all(feature = "tracing", feature = "http")))]
fn main() {
    eprintln!("Requires features: http, tracing");
    std::process::exit(1);
}
