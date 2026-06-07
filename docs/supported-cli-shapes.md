# Supported CLI shapes

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

Reference for CLI layouts clap-mcp supports, attributes needed for each pattern,
and shapes that are intentionally out of scope. Runnable binaries are listed in
[examples/README.md](../examples/README.md).

## Shape matrix

| CLI shape | MCP pattern | Example | Notes |
| --- | --- | --- | --- |
| Flat enum root | `output_from` on enum | `subcommands` | Default happy path |
| Struct root, subcommand only in `run` | Dual derive; delegate | `struct_subcommand_required` | Root globals not in `run` unless struct `output_from` |
| Struct root + globals in `run` | `output_from` on struct; `schema_only` on nested enums | `struct_subcommand_globals` | Tool execution receives full parsed root; root `#[arg(global)]` appear on leaf tool `inputSchema` |
| Multi-level subcommands | `schema_only` on intermediates; auto metadata merge | `nested_subcommands` | Manual `merge_from` rarely needed |
| Skipped shell-only tools | `#[clap_mcp(skip)]`; positionals OK on skipped variants | `optional_commands_and_args` | Skipped variants exempt from multi-positional guard |
| Interactive / TTY / exec | `skip` | [Execution safety — Interactive](execution-safety.md#interactive-and-session-commands) | Not an MCP tool |
| Cross-tool locking | `Mutex` / stateful / `parallel_safe = false` | [Execution safety — Cross-tool](execution-safety.md#cross-tool-serialization) | No lock-group attribute |

## Explicit non-goals

clap-mcp does not currently provide first-class support for:

- Mapping `hide` to MCP tool visibility (use `#[clap_mcp(skip)]` explicitly).
- ArgGroup-aware JSON Schema generation.
- In-process `exit` trapping for subprocess mode.
- Bidirectional interactive MCP sessions over stdio.

## Maintainer regression

The canonical compile-time tree for nested skip, struct `output_from`, skipped
multi-positionals, `requires`, and `schema_only` is
[`clap-mcp/tests/complex_cli_fixture/`](../clap-mcp/tests/complex_cli_fixture/mod.rs).
Run `cargo test -p clap-mcp --all-features complex_cli`. See
[maintainer-testing.md](maintainer-testing.md).
