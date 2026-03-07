#[cfg(feature = "output-schema")]
use clap::Parser;
#[cfg(feature = "output-schema")]
use clap_mcp::{ClapMcp, ClapMcpSchemaMetadataProvider};
#[cfg(feature = "output-schema")]
use schemars::JsonSchema;
#[cfg(feature = "output-schema")]
use serde::Serialize;

#[cfg(feature = "output-schema")]
#[derive(Debug, Serialize, JsonSchema)]
struct OutputA {
    value: i32,
}

#[cfg(feature = "output-schema")]
#[derive(Debug, Serialize, JsonSchema)]
struct OutputB {
    label: String,
}

#[cfg(feature = "output-schema")]
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp_output_type = "OutputA"]
#[clap_mcp_output_one_of = "OutputA, OutputB"]
#[command(name = "output-one-of-pass", subcommand_required = true)]
enum Cli {
    Foo,
}

#[cfg(feature = "output-schema")]
fn main() {
    let metadata = Cli::clap_mcp_schema_metadata();
    let output_schema = metadata.output_schema.expect("output schema should be present");
    assert!(output_schema.get("oneOf").is_some());
}

#[cfg(not(feature = "output-schema"))]
fn main() {}
