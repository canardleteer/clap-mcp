use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[command(name = "stderr-success", subcommand_required = true)]
enum Cli {
    SucceedWithStderr,
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
    match cli {
        Cli::SucceedWithStderr => {
            println!("stdout ok");
            eprintln!("stderr note");
        }
    }
}
