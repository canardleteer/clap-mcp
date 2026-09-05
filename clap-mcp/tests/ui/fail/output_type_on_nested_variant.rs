use clap::Subcommand;
use clap_mcp::ClapMcp;

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Top {
    #[clap_mcp(output_type = "String")]
    Group {
        #[command(subcommand)]
        command: Leaf,
    },
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Leaf {
    Ping,
}

fn run(_: Top) -> String {
    "x".into()
}

fn main() {}
