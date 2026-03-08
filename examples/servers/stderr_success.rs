use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "stderr-success", subcommand_required = true)]
enum Cli {
    SucceedWithStderr,
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::SucceedWithStderr => {
            println!("stdout ok");
            eprintln!("stderr note");
            "stdout ok".to_string()
        }
    }
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
    println!("{}", run(cli));
}
