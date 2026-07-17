# Custom resources and prompts

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

In addition to the built-in **`clap://schema`** resource and the optional
**logging guide** prompt, you can expose custom MCP resources and prompts. Add
them to [`ClapMcpServeOptions`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html)
and pass that into `parse_or_serve_mcp_with`, [`ServeMcpBuilder`], or the
lower-level [`serve_mcp`] / [`serve_mcp_blocking`] functions.

## Custom resources

Set [`custom_resources`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html#structfield.custom_resources)
to a list of [`CustomResource`](https://docs.rs/clap-mcp/latest/clap_mcp/content/struct.CustomResource.html)
values. Each has:

* **Identity:** `uri`, `name`, optional `title`, `description`, `mime_type`. Use
  a stable URI (e.g. `myapp://config`) so clients can list and read.
* **Content:** One of:
  * **static text** (`ResourceContent::Static(String)`) for MCP text contents
  * **static binary** (`ResourceContent::StaticBlob { base64 }`) for MCP `blob`
    contents (pass base64-encoded bytes; clap-mcp does not encode or decode)
  * **dynamic text** (`ResourceContent::Dynamic(Arc<dyn ResourceContentProvider>)`)
    via async [`ResourceContentProvider::read`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.ResourceContentProvider.html#tymethod.read)

Example (static text):

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

Example (static binary / blob):

```rust
use clap_mcp::content::{CustomResource, ResourceContent};

opts.custom_resources.push(CustomResource {
    uri: "myapp://icon".into(),
    name: "icon".into(),
    title: Some("Icon".into()),
    description: Some("PNG icon".into()),
    mime_type: Some("image/png".into()),
    content: ResourceContent::StaticBlob {
        base64: "iVBORw0KGgo...".into(),
    },
});
```

For dynamic text content, implement
[`ResourceContentProvider`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.ResourceContentProvider.html)
(async `read(uri)`).

## Custom prompts

Set [`custom_prompts`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html#structfield.custom_prompts)
to a list of [`CustomPrompt`](https://docs.rs/clap-mcp/latest/clap_mcp/content/struct.CustomPrompt.html)
values. Each has:

* **Identity:** `name`, optional `title`, `description`, optional `arguments`
  (MCP prompt argument descriptors).
* **Content:** Either **static** (`PromptContent::Static(Vec<PromptMessage>)`)
  or **dynamic** (`PromptContent::Dynamic(Arc<dyn PromptContentProvider>)`).
  Dynamic uses the async [`PromptContentProvider::get`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.PromptContentProvider.html#tymethod.get).

The built-in **`clap-mcp-logging-guide`** prompt is only listed when logging is
enabled (`serve_options.log_rx.is_some()`). Custom prompts are always merged
into the list.

## Resource URI templates

Set [`custom_resource_templates`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html#structfield.custom_resource_templates)
to expose `resources/templates/list` and template-matched `resources/read`.

Templates use a simple dialect: single-segment `{param}` placeholders such as
`myapp://item/{id}`. This is not full RFC 6570. On read, clap-mcp substitutes
captured values into static (and dynamic) text content. Exact
`custom_resources` URIs take precedence over template matches. `StaticBlob` is
not supported for template content in this release.

```rust
use clap_mcp::content::{CustomResourceTemplate, ResourceContent};

opts.custom_resource_templates.push(CustomResourceTemplate {
    uri_template: "myapp://item/{id}".into(),
    name: "item".into(),
    title: Some("Item".into()),
    description: Some("Item by id".into()),
    mime_type: Some("application/json".into()),
    content: ResourceContent::Static(r#"{"id":"{id}"}"#.into()),
});
```

## Resource subscribe

When resources are enabled, clap-mcp advertises `resources.subscribe` and
accepts `resources/subscribe` and `resources/unsubscribe` for any URI. The
server records subscribed URIs in memory and returns success.

clap-mcp does **not** send `notifications/resources/updated` after subscribe.
There is no clap-shaped invalidation or watch API yet; clients must re-read
resources when they need fresh content.

## URI and name conventions

Prefer a stable prefix (e.g. `myapp://`) for custom resource URIs so they don’t
clash with the built-in `clap://schema`. Prompt names must be unique; avoid
`clap-mcp-logging-guide` for custom prompts.
