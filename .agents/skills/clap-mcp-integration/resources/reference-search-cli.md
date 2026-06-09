# Search / ripgrep-shaped CLIs

Read when integrating MCP for **file search**, **grep-like**, or **passthrough** CLIs — including enabling ripgrep-style agent tools (pattern + paths + flags after `--`).

## clap derive search CLIs

MCP sends **named JSON** arguments; clap-mcp rebuilds argv for tool execution.

### Trailing paths

Prefer explicit long flags or a single trailing `Vec`:

```rust
Search {
    #[arg(long)]
    pattern: String,
    #[arg(last = true)]
    paths: Vec<PathBuf>,
}
```

For hyphen-prefixed trailing tokens, use `allow_hyphen_values = true` on the trailing field.

### Passthrough (`--`)

Shell `--` is not inserted in MCP. Pass trailing tokens as a JSON **array** on a `Vec<String>` field (often `#[arg(last = true, allow_hyphen_values = true)]`).

Upstream examples: **passthrough_args**, **passthrough_args_subprocess**, **vec_and_flags** in [examples/README.md](../../../../examples/README.md).

### Pattern repetition

Use `Vec<String>` with `#[arg(long = "regexp", short = 'e')]` rather than multiple bare positionals — clap-mcp rejects two+ scalar positionals per variant at compile time.

### Read-only search default

Search/list tools are usually safe for `reinvocation_safe` + `parallel_safe = true` with no topical locks. Index rebuilds or cache writes get `#[clap_mcp(serialized)]`.

## Upstream ripgrep (BurntSushi/ripgrep)

Ripgrep uses a **custom flag system**, not `#[derive(Parser)]`. Integration options:

1. **Imperative clap-mcp** — build a parallel `clap::Command` tree for MCP schema + `get_matches_or_serve_mcp`, delegating to existing `LowArgs` parsing (most faithful, more work).
2. **Thin derive wrapper binary** — separate `rg-mcp` crate with derive CLI mapping to ripgrep's library API (slimmer MCP surface, possible flag drift).
3. **Skip shell-only flags** — `#[clap_mcp(skip)]` on completion, man, hyperlinks; expose search-relevant tools only.

Do not force `#[derive(ClapMcp)]` on ripgrep's internal flag structs.

## MCP schema hints for search tools

| CLI behavior | MCP attribute |
|--------------|---------------|
| Search without paths searches cwd | Document in tool description; optional `requires = "paths"` if stdin-less MCP should always specify paths |
| `-e` / pattern required | `#[clap_mcp(requires = "pattern")]` on `Option` pattern field |
| Type of output (`json` vs human) | `#[clap_mcp(skip)]` on `-o`; structured output from `run` return type |
| Hidden debug flags | `#[clap_mcp(skip)]` (`hide` alone does not hide from MCP) |

## Validation

Schema test should include at least one search leaf tool name. Live probe: `tools/call` with `pattern` + `paths` array; confirm matches return in `structuredContent` or captured stdout.
