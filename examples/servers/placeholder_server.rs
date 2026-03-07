use clap::{Arg, Command};
use clap_mcp::{
    ClapMcpConfig, ClapMcpSchemaMetadata, schema_from_command,
    serve_schema_json_over_stdio_blocking,
};

fn main() {
    if !std::env::args().any(|arg| arg == "--mcp") {
        eprintln!("run with --mcp");
        std::process::exit(2);
    }

    let schema = schema_from_command(
        &Command::new("placeholder-server")
            .subcommand(Command::new("echo").arg(Arg::new("message").long("message"))),
    );
    let schema_json = serde_json::to_string_pretty(&schema).expect("schema should serialize");

    serve_schema_json_over_stdio_blocking(
        schema_json,
        None,
        ClapMcpConfig::default(),
        None,
        Default::default(),
        &ClapMcpSchemaMetadata::default(),
    )
    .expect("placeholder server should start");
}
