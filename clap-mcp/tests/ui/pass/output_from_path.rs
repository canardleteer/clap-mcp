use clap::Parser;
use clap_mcp::{AsStructured, ClapMcp, ClapMcpToolExecutor};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Output {
    doubled: i32,
}

mod handlers {
    use super::{AsStructured, Cli, Output};

    pub fn run(cli: Cli) -> Result<AsStructured<Output>, String> {
        match cli {
            Cli::Double { value } => Ok(AsStructured(Output { doubled: value * 2 })),
        }
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "handlers::run"]
#[command(name = "output-from-path", subcommand_required = true)]
enum Cli {
    Double {
        #[arg(long)]
        value: i32,
    },
}

fn main() {
    let result = Cli::Double { value: 7 }
        .execute_for_mcp()
        .expect("output_from path should execute");
    assert!(matches!(result, clap_mcp::ClapMcpToolOutput::Structured(_)));
}
