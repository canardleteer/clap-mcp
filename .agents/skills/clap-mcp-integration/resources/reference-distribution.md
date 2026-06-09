# Distribution and CI

Read when MCP is feature-gated and ships to users.

## Dual binary layout (optional)

When default crates.io install should stay slim:

```
mycli/          # default binary, no mcp feature
mycli-mcp/      # workspace member, depends on mycli with features = ["mcp"]
```

```toml
# mycli-mcp/Cargo.toml
[dependencies]
mycli = { path = "..", features = ["mcp"] }
```

```rust
// mycli-mcp/src/main.rs
fn main() -> ExitCode {
    match mycli::run_app() {
        Ok(c) => c,
        Err(e) => { eprintln!("{e}"); ExitCode::FAILURE }
    }
}
```

Mark dist-only crate `publish = false` if only release artifacts ship the MCP binary.

## cargo-dist

Add MCP workspace member to `dist-workspace.toml` / `[workspace.metadata.dist]` when publishing `mycli-mcp` installers alongside the default binary.

## CI matrix

Test both feature modes:

```yaml
strategy:
  matrix:
    features: [default, mcp]
steps:
  - run: cargo test ${{ matrix.features == 'mcp' && '--features mcp' || '' }}
```

When help text diverges (`--mcp` flags), gate insta snapshots with env vars so default and MCP jobs do not fight the same golden files.

## README

Document:

- `cargo install mycli --features mcp` vs pre-built `mycli-mcp`
- `mycli-mcp --mcp` stdio transport
- `--mcp-http` if `http` feature enabled
- `--export-skills` for agent skill export
- Experimental status if feature is not default

## crates.io default features

Keep `default = []` — MCP stays opt-in unless user chose always-on in Phase 0.
