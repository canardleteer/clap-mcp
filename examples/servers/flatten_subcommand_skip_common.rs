//! Shared types for flatten_subcommand_skip_* example binaries.

use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;

pub mod flat {
    use super::*;

    #[derive(Debug, Subcommand, ClapMcp)]
    #[clap_mcp(schema_only)]
    pub enum HiddenSubcommands {
        #[command(name = "hidden-a")]
        HiddenA,
        #[command(name = "hidden-b")]
        HiddenB,
    }

    #[derive(Debug, Parser, ClapMcp)]
    #[clap_mcp(reinvocation_safe, parallel_safe = false)]
    #[clap_mcp_output_from = "run_flat"]
    #[command(
        name = "flatten-subcommand-skip-flat",
        about = "Skip flattened Subcommand enum — flat hidden tools"
    )]
    pub struct FlatCli {
        #[command(subcommand)]
        #[clap_mcp(skip)]
        pub commands: HiddenSubcommands,
        #[arg(long)]
        pub visible: Option<String>,
    }

    pub fn run_flat(cli: FlatCli) -> String {
        format!("visible={:?}", cli.visible)
    }
}

pub mod nested {
    use super::*;

    #[derive(Debug, Subcommand, ClapMcp)]
    #[clap_mcp(schema_only)]
    pub enum NestedBuildActions {
        #[command(name = "compile")]
        Compile,
        #[command(name = "link")]
        Link,
    }

    #[derive(Debug, Subcommand, ClapMcp)]
    #[clap_mcp(schema_only)]
    pub enum NestedOuterCommands {
        #[command(name = "build")]
        Build {
            #[command(subcommand)]
            action: NestedBuildActions,
        },
        #[command(name = "clean")]
        Clean,
    }

    #[derive(Debug, Parser, ClapMcp)]
    #[clap_mcp(reinvocation_safe, parallel_safe = false)]
    #[clap_mcp_output_from = "run_nested"]
    #[command(
        name = "flatten-subcommand-skip-nested",
        about = "Skip flattened Subcommand enum — nested hidden tools"
    )]
    pub struct NestedCli {
        #[command(subcommand)]
        #[clap_mcp(skip)]
        pub commands: NestedOuterCommands,
    }

    pub fn run_nested(cli: NestedCli) -> String {
        format!("{cli:?}")
    }
}
