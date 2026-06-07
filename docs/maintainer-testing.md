# Maintainer testing guide

> Maintainer guide — macro checklists, test filters, and example contracts. See
> [README](../README.md#documentation).

Path-scoped agent rules live in [`.agents/rules/`](../.agents/rules/) per the
[agent-rules-spec RFC](https://github.com/rameshsunkara/agent-rules-spec)
(draft). This document is the human-oriented source of truth.

## Quick filters

```shell
cargo test -p clap-mcp --all-features complex_cli
cargo test -p clap-mcp --all-features example_contract
cargo test -p clap-mcp --test trybuild
cargo xtask examples-help
```

Run the full gate in [AGENTS.md](../AGENTS.md) before merge or release.

## Macro change checklist

When editing `clap-mcp/macros/` or derive behavior, verify every row that
applies:

| Change | Must verify | Test / doc |
| --- | --- | --- |
| New compile-time guard on variant fields | `#[clap_mcp(skip)]` exempts guard | `complex_cli_*`, UI pass/fail in `tests/ui/` |
| New `ClapMcpSchemaMetadata` field | Deep-merge in `build_schema_metadata_impl` for nested `#[command(subcommand)]` | `complex_cli_nested_skip_*`, `test_clap_mcp_schema_metadata_merge_from` |
| New enum derive requirement | `#[clap_mcp(schema_only)]` escape hatch still works | `tests/ui/pass/schema_only_nested.rs` |
| Struct executor path change | Struct `output_from` receives full root; default still delegates | `complex_cli_struct_output_from_*`, `struct_subcommand_globals` example |
| New `#[clap_mcp(...)]` config flag | Documented in supported-shapes matrix if embedder-visible | [supported-cli-shapes.md](supported-cli-shapes.md) |
| New `[[bin]]` in examples | Auto-included in `cargo xtask examples-help` unless on exclude list | [examples/Cargo.toml](../examples/Cargo.toml), [examples/README.md](../examples/README.md); add contract test if MCP semantics matter |

### PR self-check

- Run `complex_cli` and `trybuild` when macros change.
- Run `cargo xtask examples-help` when examples change.
- Update [supported-cli-shapes.md](supported-cli-shapes.md) when embedder-visible behavior changes.
- Add an `example_contract` test when an example encodes a shape invariant.

## Complex CLI fixture

Canonical tree: [`clap-mcp/tests/complex_cli_fixture/`](../clap-mcp/tests/complex_cli_fixture/mod.rs).

Contract tests: [`complex_cli_fixture_tests.rs`](../clap-mcp/tests/complex_cli_fixture_tests.rs)
(filter `complex_cli`).

## Example contract tests

Documented in [`example_contract_tests.rs`](../clap-mcp/tests/example_contract_tests.rs)
(filter `example_contract`).

| Example binary | Contract |
| --- | --- |
| `nested_subcommands` | `child` in tools; `internal` not in tools |
| `struct_subcommand_globals` | `greet` MCP round-trip; struct `output_from` + globals in `complex_cli_struct_output_from_*` |
| `optional_commands_and_args` | `internal` not in tools; `read` schema requires `path` |
| `struct_subcommand_required` | CLI argv parity (see `cli_compat_tests.rs`) |

## Adding an example

1. Add `[[bin]]` and source under `examples/`.
2. Document in [examples/README.md](../examples/README.md).
3. Link from [supported-cli-shapes.md](supported-cli-shapes.md) when the example
   demonstrates a CLI shape.
4. Do **not** edit an opt-in list in xtask. New bins are included in release
   `--help` smoke automatically.
5. Add to `RELEASE_VALIDATION_EXCLUDE` in
   [`xtask/src/examples_help.rs`](../xtask/src/examples_help.rs) only when
   `--help` smoke is impossible or inappropriate, with a comment explaining why.

Current excludes:

| Bin | Reason |
| --- | --- |
| `clap-mcp-conformance-http` | Maintainer conformance fixture |
| `placeholder_server` | No clap `Parser` `--help` |
| `invalid_executable_server` | Bad-executable test fixture |
| `oauth_http_client` | Requires OAuth env vars before `--help` |
