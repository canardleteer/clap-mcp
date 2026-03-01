//! Example CLI with MCP logging via the `log` crate.
//!
//! Run: `cargo run -p clap-mcp-examples --bin log_bridge --features log -- echo "hello"`
//! Run: `cargo run -p clap-mcp-examples --bin log_bridge --features log -- --mcp`
//!
//! When run with --mcp, `log::info!` (etc.) messages are forwarded to the MCP client.
//!
//! Because the `log` crate only supports one global logger, this example uses a
//! small multiplexing logger (`TeeLogger`) that fans out to both stderr and the
//! MCP channel. See the README for more on this trade-off.

use clap::Parser;
use clap_mcp::ClapMcp;

#[cfg(feature = "log")]
use clap_mcp::ClapMcpConfigProvider;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(
    name = "log-bridge-example",
    about = "CLI with MCP logging (log crate)",
    subcommand_required = false
)]
enum Cli {
    /// Echo a string, logging via the `log` crate.
    #[clap_mcp_output = "format!(\"Echo: {}\", s)"]
    Echo {
        /// The string to echo.
        s: String,
    },
}

#[cfg(feature = "log")]
mod tee_logger {
    use clap_mcp::logging::ClapMcpLogBridge;

    /// A logger that sends to both `ClapMcpLogBridge` (MCP channel) and stderr.
    /// Demonstrates how to multiplex the `log` crate's single global logger.
    pub struct TeeLogger {
        pub mcp: ClapMcpLogBridge,
    }

    impl log::Log for TeeLogger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            self.mcp.enabled(metadata)
        }

        fn log(&self, record: &log::Record) {
            self.mcp.log(record);
            eprintln!("[{}] {}", record.level(), record.args());
        }

        fn flush(&self) {
            self.mcp.flush();
        }
    }
}

#[cfg(feature = "log")]
fn main() {
    use clap_mcp::logging::{ClapMcpLogBridge, log_channel};
    use tee_logger::TeeLogger;

    let (log_tx, log_rx) = log_channel(32);
    let bridge = ClapMcpLogBridge::new(log_tx);
    let tee = TeeLogger { mcp: bridge };
    log::set_logger(Box::leak(Box::new(tee))).expect("logger must install");
    log::set_max_level(log::LevelFilter::Info);

    let serve_options = clap_mcp::ClapMcpServeOptions {
        log_rx: Some(log_rx),
        ..Default::default()
    };

    let cli = clap_mcp::parse_or_serve_mcp_with_config_and_options::<Cli>(
        Cli::clap_mcp_config(),
        serve_options,
    );

    match cli {
        Cli::Echo { s } => {
            log::info!("Echoing: {}", s);
            println!("Echo: {}", s);
        }
    }
}

#[cfg(not(feature = "log"))]
fn main() {
    eprintln!("This example requires the 'log' feature. Run with:");
    eprintln!("  cargo run -p clap-mcp-examples --bin log_bridge --features log -- --mcp");
    std::process::exit(1);
}
