use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ClapMcpSchemaMetadataProvider, ClapMcpToolExecutor};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[command(name = "schema-only-nested", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run_top"]
enum TopLevel {
    Leaf {
        #[command(subcommand)]
        command: LeafCommands,
    },
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum LeafCommands {
    Ping,
}

fn run_top(cmd: TopLevel) -> String {
    match cmd {
        TopLevel::Leaf { command } => match command {
            LeafCommands::Ping => "pong".into(),
        },
    }
}

fn main() {
    let metadata = Cli::clap_mcp_schema_metadata();
    assert!(!metadata.skip_commands.contains(&"ping".to_string()));
    let out = TopLevel::Leaf {
        command: LeafCommands::Ping,
    }
    .execute_for_mcp()
    .expect("ancestor executor should run");
    assert!(matches!(out, clap_mcp::ClapMcpToolOutput::Text(_)));
}
