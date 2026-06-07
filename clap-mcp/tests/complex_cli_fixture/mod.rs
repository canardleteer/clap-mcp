//! Canonical complex CLI shape for macro regression tests.
//!
//! Combines struct-root `output_from`, multi-level `schema_only` nesting, nested
//! `skip`, skipped multi-positional siblings, `requires`, and `serialized` in one tree.

use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_complex"]
#[command(name = "test-complex-cli", subcommand_required = true)]
pub struct TestComplexCli {
    #[arg(long, global = true)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: TestComplexTop,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
pub enum TestComplexTop {
    Sandbox {
        #[command(subcommand)]
        command: TestComplexSandbox,
    },
    #[clap_mcp(skip)]
    Upload { name: String, local_path: String },
    #[clap_mcp(serialized)]
    Snapshot,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
pub enum TestComplexSandbox {
    Leaf {
        #[clap_mcp(requires = "name")]
        #[arg(long)]
        name: Option<String>,
    },
    #[clap_mcp(skip)]
    Connect,
}

pub fn run_complex(cli: TestComplexCli) -> String {
    match cli.command {
        TestComplexTop::Sandbox { command } => run_complex_sandbox(command, cli.verbose),
        TestComplexTop::Upload { name, local_path } => format!("upload:{name}:{local_path}"),
        TestComplexTop::Snapshot => format!("snapshot:verbose={}", cli.verbose),
    }
}

fn run_complex_sandbox(cmd: TestComplexSandbox, verbose: bool) -> String {
    match cmd {
        TestComplexSandbox::Leaf { name } => {
            format!(
                "leaf:{}:verbose={}",
                name.as_deref().unwrap_or("anon"),
                verbose
            )
        }
        TestComplexSandbox::Connect => "connect".to_string(),
    }
}
