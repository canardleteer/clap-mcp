# Supported CLI shapes

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

Reference for CLI layouts clap-mcp supports, attributes needed for each pattern,
and shapes that are intentionally out of scope. Runnable binaries are listed in
[examples/README.md](../examples/README.md).

> [!NOTE]
> Integrator policy: use a preserve-cli parse helper when shell UX matters; use
> `skip` / `requires` when agent policy matters; do not expect clap help metadata
> to drive MCP visibility unless you opt into that explicitly. See
> [Usage — Preserve CLI parse](usage.md#preserve-cli-parse) and
> [Execution safety — hide vs skip](execution-safety.md#hide-vs-clap_mcpskip).

## Shape matrix

| CLI shape | MCP pattern | Example | Notes |
| --- | --- | --- | --- |
| Flat enum root | `output_from` on enum | `subcommands` | Default happy path |
| Flat struct root (no subcommand) | `output_from` on struct | — | One MCP tool with a wide `inputSchema` (all root fields and flattened `Args` groups) |
| Skipped flattened `Args` | `#[command(flatten)]` + `#[clap_mcp(skip)]` on field | — | Every clap arg id from the flattened type is excluded, not only the Rust field name |
| Skipped subcommand group | `#[command(subcommand)]` + `#[clap_mcp(skip)]` on field | — | `Subcommand::augment_subcommands` probe adds subcommand names to `skip_commands` (recursive) |
| Explicit arg-id skip list | `#[clap_mcp(skip = "id1,id2")]` on field | — | Comma-separated clap arg ids on flatten; subcommand names on `#[command(subcommand)]` |
| Nested `serialize_topic` in flattened `Args` | `#[clap_mcp(args_metadata)]` on shared `Args` + flatten on variant | — | `#[clap_mcp(serialize_topic)]` inside the helper; same-crate `Args` source required |
| Struct root, subcommand only in `run` | Dual derive; delegate | `struct_subcommand_required` | Root globals not in `run` unless struct `output_from` |
| Struct root + globals in `run` | `output_from` on struct; `schema_only` on nested enums | `struct_subcommand_globals` | Tool execution receives full parsed root; root `#[arg(global)]` appear on leaf tool `inputSchema` |
| Multi-level subcommands | `schema_only` on intermediates; auto metadata merge | `nested_subcommands` | Manual `merge_from` rarely needed |
| Skipped shell-only tools | `#[clap_mcp(skip)]`; positionals OK on skipped variants | `optional_commands_and_args` | Skipped variants exempt from multi-positional guard |
| Interactive / TTY / exec | `skip` | [Execution safety — Interactive](execution-safety.md#interactive-and-session-commands) | Not an MCP tool |
| Cross-tool locking | `Mutex` / stateful / `parallel_safe = false` | [Execution safety — Cross-tool](execution-safety.md#cross-tool-serialization) | No lock-group attribute |
| ArgGroup hints (not schema `oneOf`) | clap `#[group]` / `.group()`; `meta.clapMcp.argGroups` + description suffix | `arg_group_hints` | Advisory; parse-time enforcement only |

## Flat struct tradeoff

When the derive root is a struct with no `#[command(subcommand)]`, clap-mcp
exposes a **single** MCP tool whose `inputSchema` includes every non-skipped
argument on the root command. That matches a flat CLI layout but produces a
large schema when many flags and flattened `Args` groups are present. Prefer
subcommands when you want smaller per-tool schemas; use `#[clap_mcp(skip)]` on
flattened groups you do not want agents to set over MCP.

## Known limitations

* Derive metadata (`skip`, `requires`, `serialize_topic`, `serialized = "..."`)
  keys match clap arg ids (field ident by default; `#[arg(id = "...")]` when set).
  Use the clap id in `serialized = "..."` and variant `requires = "..."` lists.
  Imperative [`ClapMcpSchemaMetadata`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpSchemaMetadata.html)
  overrides still apply when derive cannot see your types.
* Flatten skip and nested `serialize_topic` collection require same-crate
  `Args` / `Subcommand` types visible to the proc macro. Opaque or dependency
  types need imperative `skip_commands`, `skip_args`, or `serialize_topic_args`.
* `skip_commands` entries are global by subcommand name across the schema tree.
* Topical serialization (`serialized`, `serialize_topic`) gates concurrent tool
  entry only; it does not isolate
  [`ClapMcpToolExecutorWithState`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.ClapMcpToolExecutorWithState.html)
  session state. See [Stateful MCP tools](stateful-tools.md) and
  [Security](security.md).

## Explicit non-goals

clap-mcp does not currently provide first-class support for:

* Mapping `hide` to MCP tool visibility (use `#[clap_mcp(skip)]` explicitly).
* ArgGroup-aware JSON Schema `oneOf` generation (`meta.clapMcp.argGroups` hints are supported; see [Arg groups](execution-safety.md#arg-groups)).
* In-process `exit` trapping for subprocess mode.
* Bidirectional interactive MCP sessions over stdio.

## Maintainer regression

The canonical compile-time tree for nested skip, struct `output_from`, skipped
multi-positionals, `requires`, and `schema_only` is
[`clap-mcp/tests/complex_cli_fixture/`](../clap-mcp/tests/complex_cli_fixture/mod.rs).
Run `cargo test -p clap-mcp --all-features complex_cli`. See
[maintainer-testing.md](maintainer-testing.md).
