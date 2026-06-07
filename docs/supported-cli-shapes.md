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
| Flat struct root (no subcommand) | `output_from` on struct | — | One MCP tool with a wide `inputSchema` (all root fields and flattened `Args` groups) |
| Skipped flattened `Args` | `#[command(flatten)]` + `#[clap_mcp(skip)]` on field | — | Every clap arg id from the flattened type is excluded, not only the Rust field name |
| Explicit arg-id skip list | `#[clap_mcp(skip = "id1,id2")]` on field | — | Comma-separated clap arg ids; combines with flatten probe when both apply |
| Struct root, subcommand only in `run` | Dual derive; delegate | `struct_subcommand_required` | Root globals not in `run` unless struct `output_from` |
| Struct root + globals in `run` | `output_from` on struct; `schema_only` on nested enums | `struct_subcommand_globals` | Tool execution receives full parsed root; root `#[arg(global)]` appear on leaf tool `inputSchema` |
| Multi-level subcommands | `schema_only` on intermediates; auto metadata merge | `nested_subcommands` | Manual `merge_from` rarely needed |
| Skipped shell-only tools | `#[clap_mcp(skip)]`; positionals OK on skipped variants | `optional_commands_and_args` | Skipped variants exempt from multi-positional guard |
| Interactive / TTY / exec | `skip` | [Execution safety — Interactive](execution-safety.md#interactive-and-session-commands) | Not an MCP tool |
| Cross-tool locking | `Mutex` / stateful / `parallel_safe = false` | [Execution safety — Cross-tool](execution-safety.md#cross-tool-serialization) | No lock-group attribute |

## Flat struct tradeoff

When the derive root is a struct with no `#[command(subcommand)]`, clap-mcp
exposes a **single** MCP tool whose `inputSchema` includes every non-skipped
argument on the root command. That matches a flat CLI layout but produces a
large schema when many flags and flattened `Args` groups are present. Prefer
subcommands when you want smaller per-tool schemas; use `#[clap_mcp(skip)]` on
flattened groups you do not want agents to set over MCP.

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
