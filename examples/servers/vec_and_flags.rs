//! Example: Vec (list) and action-based args (boolean flag, count) in MCP.
//!
//! Shows that:
//! - **Option args**: `Vec<T>` with `--opt val1 --opt val2` is exposed as an MCP **array** input.
//! - **Positional args**: `Option<Vec<T>>` (e.g. `versions: Option<Vec<Version>>`) is also exposed
//!   as an MCP **array**; clients pass a list and it becomes multiple positional values in order.
//! - `bool` with `SetTrue` is exposed as **boolean** with a hint.
//! - `u8` with `Count` (e.g. `-v -v -v`) is exposed as **integer** with a hint.
//! - Plain numeric fields advertise `"integer"` / `"number"`; lexical `value_parser`s stay
//!   `"string"` unless you set `#[clap_mcp(input_type = "...")]`.
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin vec_and_flags -- --help
//!   cargo run -p clap-mcp-examples --bin vec_and_flags -- --mcp
//!   cargo run -p clap-mcp-examples --bin vec_and_flags -- run --files a --files b --files c 1.0 2.0 --dry-run -vv --port 8080 --size 8MiB

use clap::Parser;
use clap_mcp::{ClapMcp, ParseOrServeMcp};

fn parse_mib(s: &str) -> Result<u64, String> {
    s.strip_suffix("MiB")
        .unwrap_or(s)
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())
}

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

        /// Plain numeric field (MCP advertises JSON Schema integer).
        #[arg(long)]
        port: Option<u16>,

        /// Lexical size parser (`8MiB`); stays string in MCP unless overridden.
        #[arg(long, num_args(1), value_parser = parse_mib)]
        size: Option<u64>,

        /// Force a string field to advertise as number (demo of `input_type`).
        #[arg(long)]
        #[clap_mcp(input_type = "number")]
        ratio: Option<String>,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Run {
            files,
            versions,
            dry_run,
            verbose,
            port,
            size,
            ratio,
        } => format!(
            "files={:?} (len={}), versions={:?} (len={}), dry_run={}, verbose={}, port={:?}, size={:?}, ratio={:?}",
            files,
            files.len(),
            versions,
            versions.as_ref().map(|v| v.len()).unwrap_or(0),
            dry_run,
            verbose,
            port,
            size,
            ratio
        ),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();

    match cli {
        Cli::Run {
            files,
            versions,
            dry_run,
            verbose,
            port,
            size,
            ratio,
        } => {
            println!(
                "files={:?}, versions={:?}, dry_run={}, verbose={}, port={:?}, size={:?}, ratio={:?}",
                files, versions, dry_run, verbose, port, size, ratio
            );
        }
    }
}
