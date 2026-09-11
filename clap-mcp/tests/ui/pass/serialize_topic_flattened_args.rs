//! Compiles when #[clap_mcp(serialize_topic)] lives inside a flattened Args helper.

use clap::{Args, CommandFactory, Parser};
use clap_mcp::ClapMcp;

#[derive(Debug, Args, ClapMcp)]
#[clap_mcp(args_metadata)]
struct TopicArgs {
    #[clap_mcp(serialize_topic)]
    #[arg(long)]
    output: Option<String>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run_serialize_topic_flattened_args"]
#[command(name = "serialize-topic-flattened-args")]
enum Cli {
    #[clap_mcp(serialized = "output")]
    Flush {
        #[command(flatten)]
        args: TopicArgs,
    },
}

fn run_serialize_topic_flattened_args(cmd: Cli) -> String {
    match cmd {
        Cli::Flush { args } => format!("{:?}", args.output),
    }
}

fn main() {
    let _ = Cli::command();
}
