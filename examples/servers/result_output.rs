//! Example CLI demonstrating `#[clap_mcp_output_from]` with fallible `run` returning `Result<T, E>`.
//!
//! When `run` returns `Result<T, E>`:
//! - `Ok(value)` → normal MCP tool output (Text or Structured)
//! - `Err(e)` → MCP error response (`is_error: true`); implement [`clap_mcp::IntoClapMcpToolError`]
//!   for structured error JSON.

use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpToolError};
use serde::Serialize;

// --- Inline error type (same file, used by Check) ---

#[derive(Debug, Serialize)]
struct ValidationError {
    code: i32,
    message: String,
}

// --- Error type defined elsewhere (separate module) ---

mod errors {
    use serde::Serialize;
    use std::fmt;

    #[derive(Debug, Serialize)]
    pub struct ParseError {
        pub path: String,
        pub line: u32,
        pub detail: String,
    }

    impl fmt::Display for ParseError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "parse error at {}:{} — {}",
                self.path, self.line, self.detail
            )
        }
    }

    impl std::error::Error for ParseError {}
}

/// Unified error type for `run`; implements [`clap_mcp::IntoClapMcpToolError`] so
/// structured errors (ValidationError, ParseError) can be sent as JSON.
#[derive(Debug)]
enum CliError {
    Message(String),
    Validation(ValidationError),
    Parse(errors::ParseError),
}

impl clap_mcp::IntoClapMcpToolError for CliError {
    fn into_tool_error(self) -> ClapMcpToolError {
        match self {
            CliError::Message(s) => ClapMcpToolError::text(s),
            CliError::Validation(e) => ClapMcpToolError::structured(
                format!("{:?}", e),
                serde_json::to_value(&e)
                    .unwrap_or_else(|_| serde_json::Value::String(format!("{:?}", e))),
            ),
            CliError::Parse(e) => ClapMcpToolError::structured(
                format!("{:?}", e),
                serde_json::to_value(&e)
                    .unwrap_or_else(|_| serde_json::Value::String(format!("{:?}", e))),
            ),
        }
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "result-output",
    about = "Demonstrates output_from with Result<T, E> and custom error conversion",
    subcommand_required = false
)]
enum Cli {
    /// Succeeds for n >= 0, fails otherwise (plain error message)
    Sqrt {
        #[arg(long)]
        n: i32,
    },

    /// Always succeeds
    Double {
        #[arg(long)]
        x: i32,
    },

    /// Fails for x <= 0 with structured error
    Check {
        #[arg(long)]
        x: i32,
    },

    /// Uses an error type from a separate module
    Parse {
        #[arg(long)]
        path: String,
    },
}

fn run(cmd: Cli) -> Result<String, CliError> {
    match cmd {
        Cli::Sqrt { n } => {
            if n >= 0 {
                Ok(format!("sqrt ~{n}"))
            } else {
                Err(CliError::Message(format!("negative: {n}")))
            }
        }
        Cli::Double { x } => Ok(format!("double: {}", x * 2)),
        Cli::Check { x } => {
            if x > 0 {
                Ok(format!("ok: {x}"))
            } else {
                Err(CliError::Validation(ValidationError {
                    code: -1,
                    message: format!("invalid: {x}"),
                }))
            }
        }
        Cli::Parse { path } => parse_file(&path).map_err(CliError::Parse),
    }
}

/// Simulates parsing a file. Returns `Err(errors::ParseError)` for invalid paths.
fn parse_file(path: &str) -> Result<String, errors::ParseError> {
    if path.is_empty() || path == "invalid" {
        Err(errors::ParseError {
            path: path.to_string(),
            line: 1,
            detail: "invalid or empty path".into(),
        })
    } else {
        Ok(format!("parsed: {path}"))
    }
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();

    match cli {
        Cli::Sqrt { n } => {
            if n >= 0 {
                println!("sqrt ~{n}");
            } else {
                eprintln!("negative: {n}");
                std::process::exit(1);
            }
        }
        Cli::Double { x } => println!("double: {}", x * 2),
        Cli::Check { x } => {
            if x > 0 {
                println!("ok: {x}");
            } else {
                eprintln!("invalid: {x}");
                std::process::exit(1);
            }
        }
        Cli::Parse { path } => match parse_file(&path) {
            Ok(out) => println!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
    }
}
