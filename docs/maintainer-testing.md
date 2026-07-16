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
cargo test -p clap-mcp --test config_tests skip_flattened
cargo test -p clap-mcp --test config_tests nested_flatten
cargo llvm-cov test -p clap-mcp -p clap-mcp-macros --all-features --summary-only
cargo xtask examples-help
```

Run the full gate in
[`.agents/rules/clap-mcp-ci-gate.md`](../.agents/rules/clap-mcp-ci-gate.md)
before merge or release.

## Macro change checklist

When editing `clap-mcp/macros/` or derive behavior, verify every row that
applies:

| Change | Must verify | Test / doc |
| --- | --- | --- |
| New compile-time guard on variant fields | `#[clap_mcp(skip)]` exempts guard | `complex_cli_*`, UI pass/fail in `tests/ui/` |
| New `ClapMcpSchemaMetadata` field | Deep-merge in `build_schema_metadata_impl` for nested `#[command(subcommand)]` | `complex_cli_nested_skip_*`, `test_clap_mcp_schema_metadata_merge_from` |
| New enum derive requirement | `#[clap_mcp(schema_only)]` escape hatch still works | `tests/ui/pass/schema_only_nested.rs` |
| Struct executor path change | Struct `output_from` receives full root; default still delegates | `complex_cli_struct_output_from_*`, `struct_subcommand_globals` example |
| Struct-root metadata delegate (light path) | Root flags OR onto nested metadata (`task_augmented_tools`, `skip_root_when_subcommands`, `output_schema`) | `test_struct_root_task_augmented_tools_metadata_delegate` |
| Leaf tool schema / argv / validation | Root `#[arg(global)]` on struct roots appear on nested leaf tools | `complex_cli_leaf_tool_schema_includes_root_global`, `example_contract_struct_subcommand_globals_*` |
| `#[clap_mcp(skip)]` on `#[command(flatten)]` | `Args::augment_args` probe skips every flattened arg id | `test_skip_flattened_args_excludes_all_arg_ids`, `test_skip_explicit_arg_id_list`, `tests/ui/pass/skip_arg_list.rs` |
| `#[clap_mcp(skip)]` on `#[command(subcommand)]` | `Subcommand::augment_subcommands` probe → `skip_commands` | `test_skip_flattened_subcommands_*`, `test_skip_explicit_subcommand_name_list`, `tests/ui/pass/skip_flatten_subcommands.rs` |
| Nested `serialize_topic` in flattened `Args` | `#[clap_mcp(args_metadata)]` + merge into `serialize_topic_args` | `test_nested_flatten_args_serialize_topic_*`, `tests/ui/pass/serialize_topic_flattened_args.rs` |
| Derive metadata clap arg ids | `clap_arg_id_from_field` for skip/requires/serialize keys | `test_skip_custom_clap_arg_id`, `test_requires_custom_clap_arg_id`, `test_serialize_topic_custom_clap_arg_id`, `tests/ui/pass/custom_arg_id_metadata.rs` |
| Macro/runtime coverage after new tests | `cargo llvm-cov` on `clap-mcp` + `clap-mcp-macros` | Quick filters above |
| New `#[clap_mcp(...)]` config flag | Documented in supported-shapes matrix if embedder-visible | [supported-cli-shapes.md](supported-cli-shapes.md) |
| New `[[bin]]` in examples | Auto-included in `cargo xtask examples-help` unless on exclude list | [examples/Cargo.toml](../examples/Cargo.toml), [examples/README.md](../examples/README.md); add contract test if MCP semantics matter |
| ArgGroup hints (`argGroups` meta, description suffix) | `mcp_visible_arg_ids_on_command` shared with schema args; per-node groups only | `test_arg_groups_*` in `lib.rs`, `example_contract_arg_group_hints_*`, `arg_group_hints` example; rustdoc `-D warnings` |

### PR self-check

* Run `complex_cli` and `trybuild` when macros change.
* Run `cargo xtask examples-help` when examples change.
* Update [supported-cli-shapes.md](supported-cli-shapes.md) when embedder-visible behavior changes.
* Add an `example_contract` test when an example encodes a shape invariant.

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
| `struct_subcommand_globals` | `greet` in tools; `verbose` on greet `inputSchema`; `greet` + `verbose: true` → output contains `verbose:` |
| `optional_commands_and_args` | `internal` not in tools; `read` schema requires `path` |
| `struct_subcommand_required` | CLI argv parity (see `cli_compat_tests.rs`) |
| `arg_group_hints` | `search` exposes `meta.clapMcp.argGroups`; exec-only round-trip; both exec flags → `is_error` |
| `preserve_cli_parse` | Invalid argv → non-zero exit + `Usage` in stderr (`cli_compat_tests.rs`); MCP `--mcp` via `launch_example` |
| `flat_struct_root` | Exactly one tool (`flat-struct-root`); schema includes root + flattened arg ids |
| `flatten_skip` | Skipped connection args absent; `reindex`/`repair` not in tools; `flush` has `serialized` meta |
| `flatten_subcommand_skip_flat` | One root tool; `visible` on schema; `hidden-a`/`hidden-b` absent |
| `flatten_subcommand_skip_nested` | `build`/`compile`/`link`/`clean` absent from tools |

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

## MCP conformance (local)

Prefer `cargo xtask conformance`. It stops stale
`clap-mcp-conformance-http` processes before running the pinned Docker harness
twice (`active` @ `2025-11-25`, then `draft` @ `draft`). Requires Docker.

| Command | Use |
| --- | --- |
| `cargo xtask conformance` | Dual pass (stable + draft); start → test → stop |
| `cargo xtask conformance --suite active` | Stable pass only (`2025-11-25`) |
| `cargo xtask conformance --suite draft` | Draft pass only |
| `cargo xtask conformance-stop` | Stop `conformance-server` / orphan fixture; remove pid/log/port files |
| `cargo xtask conformance-server` | Advanced debugging only; stale guard unless `--force` |

See [conformance-baseline.md](conformance-baseline.md) for baseline updates and local safety.
