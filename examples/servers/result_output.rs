//! Example CLI demonstrating `#[clap_mcp_output_result]` for fallible tool output.
//!
//! When the expression returns `Result<T, E>`:
//! - `Ok(value)` → normal MCP tool output (Text or Structured)
//! - `Err(e)` → MCP error response (`is_error: true`) with the error message
//!
//! Use `#[clap_mcp_error_type = "TypeName"]` when `E: Serialize` for structured error JSON.

use clap::Parser;
use clap_mcp::ClapMcp;
use serde::Serialize;

// --- Inline error type (same file, used by Check) ---

#[derive(Debug, Serialize)]
struct ValidationError {
    code: i32,
    message: String,
}

// --- Error type defined elsewhere (separate module) ---
//
// This pattern is useful when you have shared error types across your crate.
// The type must implement Debug (for the message) and Serialize (when using
// #[clap_mcp_error_type]). Implement Display if you prefer human-readable
// messages over Debug output.

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

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(
    name = "result-output",
    about = "Demonstrates #[clap_mcp_output_result] for Result<T, E> expressions",
    subcommand_required = false
)]
enum Cli {
    /// Succeeds for n >= 0, fails otherwise (plain error message)
    #[clap_mcp_output_result]
    #[clap_mcp_output = "if n >= 0 { Ok(format!(\"sqrt ~{}\", n)) } else { Err(format!(\"negative: {}\", n)) }"]
    Sqrt {
        #[arg(long)]
        n: i32,
    },

    /// Always succeeds
    #[clap_mcp_output_result]
    #[clap_mcp_output = "Ok::<_, String>(format!(\"double: {}\", x * 2))"]
    Double {
        #[arg(long)]
        x: i32,
    },

    /// Fails for x <= 0 with structured error via #[clap_mcp_error_type]
    #[clap_mcp_output_result]
    #[clap_mcp_error_type = "ValidationError"]
    #[clap_mcp_output = "if x > 0 { Ok(format!(\"ok: {}\", x)) } else { Err(ValidationError { code: -1, message: format!(\"invalid: {}\", x) }) }"]
    Check {
        #[arg(long)]
        x: i32,
    },

    /// Uses an error type defined in a separate module (see `errors::ParseError`).
    /// The error type lives elsewhere and is referenced by path.
    #[clap_mcp_output_result]
    #[clap_mcp_error_type = "errors::ParseError"]
    #[clap_mcp_output = "parse_file(&path)"]
    Parse {
        #[arg(long)]
        path: String,
    },
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
