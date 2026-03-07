use clap::Parser;
use clap_mcp::{ClapMcp, clap_mcp_main, output_schema_one_of};

#[derive(Debug, Parser, ClapMcp)]
#[command(name = "macro-rules-pass", subcommand_required = true)]
enum Cli {
    Foo,
}

fn main() {
    let schema = output_schema_one_of!(String, i32);
    assert!(schema.is_none() || schema.is_some());

    if false {
        clap_mcp_main!(Cli, |args| match args {
            Cli::Foo => (),
        });
    }
}
