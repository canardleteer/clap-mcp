use clap::{Arg, Command};
use clap_mcp::{
    ClapMcpConfig, ClapMcpSchemaMetadata, McpListen, ServeMcpBuilder, schema_from_command,
};
use std::path::PathBuf;

fn main() {
    if !std::env::args().any(|arg| arg == "--mcp") {
        eprintln!("run with --mcp");
        std::process::exit(2);
    }

    let schema = schema_from_command(
        &Command::new("invalid-executable-server")
            .subcommand(Command::new("echo").arg(Arg::new("message").long("message"))),
    );
    let schema_json = serde_json::to_string_pretty(&schema).expect("schema should serialize");

    ServeMcpBuilder::new()
        .listen(McpListen::Stdio)
        .schema_json(schema_json)
        .executable_path(Some(PathBuf::from("/definitely/not/a/real/clap-mcp-tool")))
        .config(ClapMcpConfig::default())
        .metadata(ClapMcpSchemaMetadata::default())
        .serve_blocking()
        .expect("invalid executable server should start");
}
