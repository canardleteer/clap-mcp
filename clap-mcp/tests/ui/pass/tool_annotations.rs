use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from = "run"]
#[command(name = "test-cli")]
enum TestCli {
    #[clap_mcp(read_only, idempotent, tool_title = "Fetch Item")]
    Fetch {
        #[arg(long)]
        id: String,
    },
    #[clap_mcp(destructive, open_world)]
    Delete {
        #[arg(long)]
        id: String,
    },
    #[clap_mcp(annotation(read_only = false, title = "Custom Title"))]
    Mutate,
}

fn run(_cmd: TestCli) -> String {
    "ok".into()
}

fn main() {}
