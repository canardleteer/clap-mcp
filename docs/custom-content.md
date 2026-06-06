# Custom resources and prompts

> Embedder guide for clap-mcp. See [README](../README.md) for getting started.

[← Documentation index](../README.md#documentation)

In addition to the built-in **`clap://schema`** resource and the optional
**logging guide** prompt, you can expose custom MCP resources and prompts. Add
them to
[`ClapMcpServeOptions`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html)
and pass that into `parse_or_serve_mcp_with`, [`ServeMcpBuilder`], or the
lower-level [`serve_mcp`] / [`serve_mcp_blocking`] functions.

## Custom resources

Set
[`custom_resources`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html#structfield.custom_resources)
to a list of
[`CustomResource`](https://docs.rs/clap-mcp/latest/clap_mcp/content/struct.CustomResource.html)
values. Each has:

* **Identity:** `uri`, `name`, optional `title`, `description`, `mime_type`. Use
  a stable URI (e.g. `myapp://config`) so clients can list and read.
* **Content:** Either **static** (`ResourceContent::Static(String)`) or
  **dynamic** (`ResourceContent::Dynamic(Arc<dyn ResourceContentProvider>)`).
  Dynamic content uses the async
  [`ResourceContentProvider::read`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.ResourceContentProvider.html#tymethod.read)
  so the handler can await it.

Example (static):

```rust
use clap_mcp::content::{CustomResource, ResourceContent};

let mut opts = clap_mcp::ClapMcpServeOptions::default();
opts.custom_resources.push(CustomResource {
    uri: "myapp://readme".into(),
    name: "readme".into(),
    title: Some("Readme".into()),
    description: Some("Project readme".into()),
    mime_type: Some("text/markdown".into()),
    content: ResourceContent::Static("# Hello\n".into()),
});
```

For dynamic content, implement
[`ResourceContentProvider`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.ResourceContentProvider.html)
(async `read(uri)`).

## Custom prompts

Set
[`custom_prompts`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html#structfield.custom_prompts)
to a list of
[`CustomPrompt`](https://docs.rs/clap-mcp/latest/clap_mcp/content/struct.CustomPrompt.html)
values. Each has:

* **Identity:** `name`, optional `title`, `description`, optional `arguments`
  (MCP prompt argument descriptors).
* **Content:** Either **static** (`PromptContent::Static(Vec<PromptMessage>)`)
  or **dynamic** (`PromptContent::Dynamic(Arc<dyn PromptContentProvider>)`).
  Dynamic uses the async
  [`PromptContentProvider::get`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.PromptContentProvider.html#tymethod.get).

The built-in **`clap-mcp-logging-guide`** prompt is only listed when logging is
enabled (`serve_options.log_rx.is_some()`). Custom prompts are always merged
into the list.

## URI and name conventions

Prefer a stable prefix (e.g. `myapp://`) for custom resource URIs so they don’t
clash with the built-in `clap://schema`. Prompt names must be unique; avoid
`clap-mcp-logging-guide` for custom prompts.
