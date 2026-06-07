use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[command(name = "bad-serialized")]
enum Cli {
    #[clap_mcp(serialized = "missing")]
    Cmd {
        #[arg(long)]
        present: Option<String>,
    },
}

fn main() {}
