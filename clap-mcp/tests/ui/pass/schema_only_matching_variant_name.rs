//! Nested schema_only enum whose name matches a variant must not require CommandFactory.
use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ClapMcpSchemaMetadataProvider};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp(skip_root_when_subcommands)]
#[clap_mcp_output_from = "run"]
#[command(name = "status-variant-name", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: Status,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Status {
    Status {
        #[arg(long)]
        count: u32,
    },
}

fn run(_: Cli) -> String {
    "ok".into()
}

fn main() {
    let _ = Cli::clap_mcp_schema_metadata();
}
