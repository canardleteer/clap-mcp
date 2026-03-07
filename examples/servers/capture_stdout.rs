use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpConfigProvider, ClapMcpServeOptions, ClapMcpToolOutput};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct StructuredResult {
    status: &'static str,
}

#[derive(Debug)]
enum Output {
    Text(String),
    Structured(StructuredResult),
}

impl clap_mcp::IntoClapMcpResult for Output {
    fn into_tool_result(
        self,
    ) -> std::result::Result<ClapMcpToolOutput, clap_mcp::ClapMcpToolError> {
        match self {
            Output::Text(text) => Ok(ClapMcpToolOutput::Text(text)),
            Output::Structured(value) => Ok(ClapMcpToolOutput::Structured(
                serde_json::to_value(value).expect("structured result should serialize"),
            )),
        }
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "capture-stdout", subcommand_required = true)]
enum Cli {
    PrintedOnly,
    PrintedAndText,
    Structured,
}

fn run(cmd: Cli) -> Output {
    match cmd {
        Cli::PrintedOnly => {
            print!("captured only");
            Output::Text(String::new())
        }
        Cli::PrintedAndText => {
            print!("captured extra");
            Output::Text("returned text".to_string())
        }
        Cli::Structured => {
            print!("captured ignored");
            Output::Structured(StructuredResult { status: "ok" })
        }
    }
}

fn main() {
    let mut serve_options = ClapMcpServeOptions::default();
    #[cfg(unix)]
    {
        serve_options.capture_stdout = true;
    }
    let cli = clap_mcp::parse_or_serve_mcp_with_config_and_options::<Cli>(
        Cli::clap_mcp_config(),
        serve_options,
    );

    match cli {
        Cli::PrintedOnly => println!("captured only"),
        Cli::PrintedAndText => println!("returned text"),
        Cli::Structured => println!("structured"),
    }
}
