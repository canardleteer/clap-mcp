//! Compiles when derive metadata uses clap arg ids from #[arg(id = "...")].

use clap::{CommandFactory, Parser};
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run_custom_arg_id_metadata"]
#[command(name = "custom-arg-id-metadata")]
enum Cli {
    #[clap_mcp(serialized = "custom-out")]
    Flush {
        #[clap_mcp(serialize_topic)]
        #[arg(id = "custom-out", long = "custom-out")]
        output_path: Option<String>,
    },
    Read {
        #[clap_mcp(requires)]
        #[arg(id = "custom-path", long = "custom-path")]
        file_path: Option<String>,
    },
}

fn run_custom_arg_id_metadata(cmd: Cli) -> String {
    match cmd {
        Cli::Flush { output_path } => format!("flush:{output_path:?}"),
        Cli::Read { file_path } => file_path.unwrap_or_default(),
    }
}

fn main() {
    let _ = Cli::command();
}
