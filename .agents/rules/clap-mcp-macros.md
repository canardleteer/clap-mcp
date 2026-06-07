---
name: clap-mcp-macros
description: Checklist when editing the clap-mcp proc-macro crate
activation: paths
paths:
  - clap-mcp/macros/**
---

# Macro changes

Before finishing edits under `clap-mcp/macros/`:

1. Read [docs/maintainer-testing.md](../../docs/maintainer-testing.md) (macro change checklist).
2. Run `cargo test -p clap-mcp --all-features complex_cli` and `cargo test -p clap-mcp --test trybuild`.
3. If behavior is embedder-visible, update [docs/supported-cli-shapes.md](../../docs/supported-cli-shapes.md) and add or extend `example_contract` tests when an example encodes the invariant.
4. Struct-root metadata delegate: OR root flags (`task_augmented_tools`, `skip_root_when_subcommands`, `output_schema`) onto delegated nested metadata on the light path, not only when the merge branch runs.
5. `#[clap_mcp(skip)]` on `#[command(flatten)]` `Args` fields: probe `Args::augment_args` and skip every flattened arg id (not the field ident only). `#[clap_mcp(skip = "id1,id2")]` adds explicit ids. Cover with `test_skip_flattened_args_excludes_all_arg_ids` and `test_skip_explicit_arg_id_list`.
6. `#[clap_mcp(skip)]` on `#[command(subcommand)]` fields: probe `Subcommand::augment_subcommands` → `skip_commands` (recursive names). `skip = "cmd1,cmd2"` lists subcommand names. Tests: `test_skip_flattened_subcommands_*`, `test_skip_explicit_subcommand_name_list`.
7. `#[clap_mcp(serialize_topic)]` inside flattened `Args`: `#[clap_mcp(args_metadata)]` + syn collection into `serialize_topic_bindings`; `flatten_args_contains_field` for `serialized =` validation. Same-crate `Args` only. Tests: `test_nested_flatten_args_serialize_topic_*`, `tests/ui/pass/serialize_topic_flattened_args.rs`.
8. Before merge on macro changes: run `cargo llvm-cov test -p clap-mcp -p clap-mcp-macros --all-features --summary-only` after adding tests.
9. Never weaken `example_contract` tests to hide embedder-visible bugs; fix product behavior instead.
