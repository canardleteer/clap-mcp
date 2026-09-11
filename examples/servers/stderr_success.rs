//! Subprocess tool that writes stderr on success, with `SubprocessStderr::Notify`.
//!
//! Non-empty stderr stays in the tool result text and is also forwarded as
//! `notifications/message` (`logger: "stderr"`). See [Logging](../../docs/logging.md).
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin stderr_success -- succeed-with-stderr
//!   cargo run -p clap-mcp-examples --bin stderr_success -- --mcp

use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpConfigProvider, ClapMcpServeOptions, SubprocessStderr};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "stderr-success", subcommand_required = true)]
enum Cli {
    SucceedWithStderr,
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::SucceedWithStderr => {
            println!("stdout ok");
            eprintln!("stderr note");
            "stdout ok".to_string()
        }
    }
}

fn main() {
    let serve = ClapMcpServeOptions::default().with_subprocess_stderr(SubprocessStderr::Notify);

    let cli = clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions {
        config: Cli::clap_mcp_config(),
        serve,
    });
    println!("{}", run(cli));
}
