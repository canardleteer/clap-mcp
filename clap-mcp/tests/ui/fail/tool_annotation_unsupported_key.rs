use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from = "run"]
#[command(name = "test-unsupported-annotation")]
enum Cli {
    #[clap_mcp(annotation(invalid_key = true))]
    Fetch {
        #[arg(long)]
        id: String,
    },
}

fn run(_cmd: Cli) -> String {
    "ok".into()
}

fn main() {}
