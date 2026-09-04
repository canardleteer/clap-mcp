use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from = "run"]
#[command(name = "test-non-bool-annotation")]
enum Cli {
    #[clap_mcp(annotation(destructive = "yes"))]
    Wipe {
        #[arg(long)]
        force: bool,
    },
}

fn run(_cmd: Cli) -> String {
    "ok".into()
}

fn main() {}
