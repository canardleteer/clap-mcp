# Logging and observability

> Embedder guide for clap-mcp. See [README](../README.md) for getting started.

[← Documentation index](../README.md#documentation)

clap-mcp can forward application log messages to MCP clients as
`notifications/message`. Two feature-gated paths are available depending on
your logging ecosystem.

## `tracing` feature

Enable with `features = ["tracing"]`. `ClapMcpTracingLayer` is a standard
[`tracing_subscriber::Layer`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html)
and **composes with any other layers** in your subscriber stack — fmt,
`tracing-opentelemetry`, file appenders, etc. Adding it does not interfere with
your existing tracing pipeline:

```rust
use clap_mcp::logging::{log_channel, ClapMcpTracingLayer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

let (log_tx, log_rx) = log_channel(32);

tracing_subscriber::registry()
    .with(ClapMcpTracingLayer::new(log_tx))
    .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
    // .with(tracing_opentelemetry::layer().with_tracer(tracer))  // works alongside
    .init();

let mut opts = clap_mcp::ClapMcpServeOptions::default();
opts.log_rx = Some(log_rx);
// Pass opts to parse_or_serve_mcp_with or ServeMcpBuilder::serve_options
```

**Current limitations:**

* Only the `message` field of each tracing event is forwarded. Other structured
  fields (e.g. `tracing::info!(count = 42, "done")` — `count` is dropped) are
  not yet included.
* Span lifecycle events (`on_new_span`, `on_enter`, `on_close`) are not
  captured.

## `log` feature

Enable with `features = ["log"]`. `ClapMcpLogBridge` implements
[`log::Log`](https://docs.rs/log/latest/log/trait.Log.html) and is installed as
the global logger:

```rust
use clap_mcp::logging::{log_channel, ClapMcpLogBridge};

let (log_tx, log_rx) = log_channel(32);
let bridge = ClapMcpLogBridge::new(log_tx);
log::set_logger(Box::leak(Box::new(bridge))).unwrap();
log::set_max_level(log::LevelFilter::Info);

let mut opts = clap_mcp::ClapMcpServeOptions::default();
opts.log_rx = Some(log_rx);
// Pass opts to parse_or_serve_mcp_with or ServeMcpBuilder::serve_options
```

**Trade-off:** The `log` crate supports exactly **one global logger**.
Installing
`ClapMcpLogBridge` replaces any existing logger (e.g. `env_logger`,
`simplelog`). If you need to log to both disk and MCP simultaneously, you'll
need a multiplexing wrapper — either a custom `Log` impl that fans out to
multiple sinks, or a crate like
[`multi_log`](https://crates.io/crates/multi_log).
