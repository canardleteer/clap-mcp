//! Example CLI demonstrating `#[clap_mcp(skip)]` and `#[clap_mcp(requires)]`.
//!
//! - **skip**: Exclude subcommands or arguments from MCP exposure
//! - **requires** (argument-level): Make an optional arg required in MCP
//! - **requires** (variant-level): When you prefer declaring multiple required args at once
//!
//! When the client omits a required arg, a clear error is returned.

use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(
    name = "optional-commands-and-args",
    about = "Demonstrates #[clap_mcp(skip)] and #[clap_mcp(requires)]",
    subcommand_required = false
)]
enum Cli {
    /// Exposed to MCP
    #[clap_mcp_output_literal = "done"]
    Public,

    /// Hidden from MCP (internal use only)
    #[clap_mcp(skip)]
    #[clap_mcp_output_literal = "internal"]
    Internal,

    /// Read: path is optional in CLI but required in MCP (argument-level)
    #[clap_mcp_output = "clap_mcp::opt_str(&path, \"<none>\").to_string()"]
    Read {
        #[clap_mcp(requires)]
        #[arg(long)]
        path: Option<String>,
    },

    /// Process: path and input are optional in CLI but required in MCP (variant-level)
    #[clap_mcp(requires = "path, input")]
    #[clap_mcp_output = "format!(\"path={}, input={}\", clap_mcp::opt_str(&path, \"\"), clap_mcp::opt_str(&input, \"\"))"]
    Process {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        input: Option<String>,
    },
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();

    match cli {
        Cli::Public => println!("done"),
        Cli::Internal => println!("internal"),
        Cli::Read { path } => println!("path: {}", path.as_deref().unwrap_or("<none>")),
        Cli::Process { path, input } => {
            println!(
                "path: {}; input: {}",
                path.as_deref().unwrap_or("<none>"),
                input.as_deref().unwrap_or("<none>")
            );
        }
    }
}
