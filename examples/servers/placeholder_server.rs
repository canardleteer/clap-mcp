use clap::{Arg, Command};
use clap_mcp::{
    ClapMcpConfig, ClapMcpSchemaMetadata, McpListen, ServeMcpBuilder, schema_from_command,
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

    ServeMcpBuilder::new()
        .listen(McpListen::Stdio)
        .schema_json(schema_json)
        .config(ClapMcpConfig::default())
        .metadata(ClapMcpSchemaMetadata::default())
        .serve_blocking()
        .expect("placeholder server should start");
}
