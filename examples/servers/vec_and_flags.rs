//! Example: Vec (list) and action-based args (boolean flag, count) in MCP.
//!
//! Shows that:
//! - **Option args**: `Vec<T>` with `--opt val1 --opt val2` is exposed as an MCP **array** input.
//! - **Positional args**: `Option<Vec<T>>` (e.g. `versions: Option<Vec<Version>>`) is also exposed
//!   as an MCP **array**; clients pass a list and it becomes multiple positional values in order.
//! - `bool` with `SetTrue` is exposed as **boolean** with a hint.
//! - `u8` with `Count` (e.g. `-v -v -v`) is exposed as **integer** with a hint.
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin vec_and_flags -- --help
//!   cargo run -p clap-mcp-examples --bin vec_and_flags -- --mcp
//!   cargo run -p clap-mcp-examples --bin vec_and_flags -- run --files a --files b --files c 1.0 2.0 --dry-run -vv

use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "vec-and-flags-example",
    about = "Example: list (Vec) and flag/count args exposed correctly in MCP",
    subcommand_required = false
)]
enum Cli {
    /// Run with optional list of files, optional positional versions, a dry-run flag, and verbosity count.
    Run {
        /// Files to process (MCP shows this as an array; pass a list).
        #[arg(long, value_name = "FILE")]
        files: Vec<String>,

        /// Version numbers as positionals (MCP shows this as an array; pass a list, e.g. ["1.0", "2.0"]).
        #[arg(value_name = "VERSION")]
        versions: Option<Vec<String>>,

        /// Dry run: don't change anything (MCP shows this as boolean).
        #[arg(long)]
        dry_run: bool,

        /// Verbosity level, e.g. -v -v -v for 3 (MCP shows this as integer).
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Run {
            files,
            versions,
            dry_run,
            verbose,
        } => format!(
            "files={:?} (len={}), versions={:?} (len={}), dry_run={}, verbose={}",
            files,
            files.len(),
            versions,
            versions.as_ref().map(|v| v.len()).unwrap_or(0),
            dry_run,
            verbose
        ),
    }
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();

    match cli {
        Cli::Run {
            files,
            versions,
            dry_run,
            verbose,
        } => {
            println!(
                "files={:?}, versions={:?}, dry_run={}, verbose={}",
                files, versions, dry_run, verbose
            );
        }
    }
}
