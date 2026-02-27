use clap::Parser;

/// Simple example CLI using clap-mcp with the derive API.
///
/// Try:
/// - `cargo run --example derive -- --help`
/// - `cargo run --example derive -- --mcp`
#[derive(Debug, Parser)]
#[command(
    name = "clap-mcp-derive-example",
    about = "Example CLI that exposes its clap schema and subcommands over MCP stdio via --mcp",
    subcommand_required = false
)]
enum Cli {
    /// Greet someone (or the world) once.
    Greet {
        /// Optional name to greet.
        #[arg(long)]
        name: Option<String>,
    },
    /// Add two integers together.
    Add {
        /// First addend.
        #[arg(long)]
        a: i32,
        /// Second addend.
        #[arg(long)]
        b: i32,
    },
    /// Subtract the second integer from the first.
    Sub {
        /// Minuend.
        a: i32,
        /// Subtrahend.
        b: i32,
    },
}

fn main() {
    // This will start the MCP stdio server and exit if `--mcp` is present;
    // otherwise it returns the parsed `Cli` value as usual.
    //
    // For execution safety configuration (reinvocation/parallel safety), use:
    //   clap_mcp::parse_or_serve_mcp_with_config::<Cli>(clap_mcp::ClapMcpConfig {
    //       reinvocation_safe: true,
    //       parallel_safe: false,  // serialize tool calls
    //       ..Default::default()
    //   })
    let cli = clap_mcp::parse_or_serve_mcp::<Cli>();

    match cli {
        Cli::Greet { name } => {
            let who = name.as_deref().unwrap_or("world");
            println!("Hello, {who}!");
        },
        Cli::Add { a, b } => {
            println!("{a} + {b} = {}", a + b);
        },
        Cli::Sub { a, b } => {
            println!("{a} - {b} = {}", a - b);
        }
    }
}

