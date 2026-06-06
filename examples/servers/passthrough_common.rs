//! Passthrough / trailing args patterns for MCP and direct CLI.

use clap::Subcommand;
use clap_mcp::{ClapMcp, ClapMcpToolError, ClapMcpToolOutput};

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run"]
pub enum Command {
    /// Forward argv after `--` (cargo-style); MCP uses `command` JSON array.
    Exec {
        #[arg(long)]
        dry_run: bool,
        #[arg(last = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Explicit named passthrough list (MCP-friendly).
    Forward {
        #[arg(long)]
        args: Vec<String>,
    },
    /// Normal field plus MCP-skipped internal token.
    Run {
        #[arg(long)]
        input: String,
        #[clap_mcp(skip)]
        #[arg(long, hide = true)]
        internal: Option<String>,
    },
}

#[derive(Debug)]
pub enum CommandOutput {
    Text(String),
}

impl clap_mcp::IntoClapMcpResult for CommandOutput {
    fn into_tool_result(self) -> Result<ClapMcpToolOutput, ClapMcpToolError> {
        match self {
            CommandOutput::Text(s) => Ok(ClapMcpToolOutput::Text(s)),
        }
    }
}

pub fn run(cmd: Command) -> CommandOutput {
    match cmd {
        Command::Exec { dry_run, command } => {
            CommandOutput::Text(format!("dry_run={dry_run} command={command:?}"))
        }
        Command::Forward { args } => CommandOutput::Text(format!("args={args:?}")),
        Command::Run { input, internal } => {
            CommandOutput::Text(format!("input={input} internal={internal:?}"))
        }
    }
}

pub fn run_interactive(cmd: Command) {
    match &cmd {
        Command::Exec { dry_run, command } => {
            println!("dry_run={dry_run} command={command:?}");
        }
        Command::Forward { args } => println!("args={args:?}"),
        Command::Run { input, internal } => {
            println!("input={input} internal={internal:?}");
        }
    }
}
