#[cfg(feature = "output-schema")]
use clap::{Parser, Subcommand};
#[cfg(feature = "output-schema")]
use clap_mcp::{ClapMcp, ClapMcpSchemaMetadataProvider};
#[cfg(feature = "output-schema")]
use schemars::JsonSchema;
#[cfg(feature = "output-schema")]
use serde::Serialize;

#[cfg(feature = "output-schema")]
#[derive(Debug, Serialize, JsonSchema)]
struct LeafOut {
    n: i32,
}

#[cfg(feature = "output-schema")]
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[command(name = "nested-output-type-sibling", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[cfg(feature = "output-schema")]
#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Commands {
    #[clap_mcp(output_type = "LeafOut")]
    Leaf {
        #[arg(long)]
        n: i32,
    },
    Group {
        #[command(subcommand)]
        command: Nested,
    },
}

#[cfg(feature = "output-schema")]
#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Nested {
    Ping,
}

#[cfg(feature = "output-schema")]
fn run(cmd: Commands) -> String {
    match cmd {
        Commands::Leaf { n } => format!("{n}"),
        Commands::Group {
            command: Nested::Ping,
        } => "pong".into(),
    }
}

#[cfg(feature = "output-schema")]
fn main() {
    let m = Cli::clap_mcp_schema_metadata();
    assert!(
        m.tool_output_schemas.contains_key("leaf"),
        "leaf sibling output_type must survive nested-enum merge: {:?}",
        m.tool_output_schemas.keys().collect::<Vec<_>>()
    );
}

#[cfg(not(feature = "output-schema"))]
fn main() {}
