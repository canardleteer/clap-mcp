use clap::Subcommand;
use clap_mcp::ClapMcp;

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
#[clap_mcp_output_from = "run"]
enum Bad {
    Foo,
}

fn run(_: Bad) -> String {
    "x".into()
}

fn main() {}
