//! Logging integration for clap-mcp.
//!
//! When the `tracing` or `log` feature is enabled, provides layers and bridges
//! to forward log messages to MCP clients via `notifications/message`.
//!
//! **When changing logger names, level mapping, or default behavior here, update the
//! logging prompt** ([`crate::LOG_INTERPRETATION_INSTRUCTIONS`] and [`crate::LOGGING_GUIDE_CONTENT`]).
//!
//! # Example (tracing feature)
//!
//! ```rust,ignore
//! use clap_mcp::logging::{log_channel, ClapMcpTracingLayer};
//! use tracing_subscriber::layer::SubscriberExt;
//! use tracing_subscriber::util::SubscriberInitExt;
//!
//! let (log_tx, log_rx) = log_channel(32);
//! let layer = ClapMcpTracingLayer::new(log_tx);
//! tracing_subscriber::registry()
//!     .with(layer)
//!     .with(tracing_subscriber::fmt::layer())
//!     .init();
//!
//! let mut opts = clap_mcp::ClapMcpServeOptions::default();
//! opts.log_rx = Some(log_rx);
//! // Pass opts to parse_or_serve_mcp_with_config_and_options
//! ```

use rust_mcp_sdk::schema::{LoggingLevel, LoggingMessageNotificationParams};
use serde_json::Value;
use tokio::sync::mpsc;

/// Maps a level string to MCP `LoggingLevel`.
///
/// Supports: trace, debug, info, notice, warn, warning, error, critical, alert, emergency.
/// Unknown levels default to `Info`.
///
/// # Example
///
/// ```
/// # #[cfg(any(feature = "tracing", feature = "log"))]
/// # {
/// use clap_mcp::logging::level_to_mcp;
/// use rust_mcp_sdk::schema::LoggingLevel;
///
/// assert!(matches!(level_to_mcp("debug"), LoggingLevel::Debug));
/// assert!(matches!(level_to_mcp("info"), LoggingLevel::Info));
/// assert!(matches!(level_to_mcp("error"), LoggingLevel::Error));
/// # }
/// ```
pub fn level_to_mcp(level: &str) -> LoggingLevel {
    match level {
        "trace" | "debug" => LoggingLevel::Debug,
        "info" => LoggingLevel::Info,
        "notice" | "warn" | "warning" => LoggingLevel::Notice,
        "error" => LoggingLevel::Error,
        "critical" => LoggingLevel::Critical,
        "alert" => LoggingLevel::Alert,
        "emergency" => LoggingLevel::Emergency,
        _ => LoggingLevel::Info,
    }
}

/// Creates a channel for forwarding log messages to the MCP server.
///
/// Returns `(sender, receiver)`. Pass the receiver to `ClapMcpServeOptions::log_rx`.
/// Install `ClapMcpTracingLayer::new(tx)` (or `ClapMcpLogBridge::new(tx)`) in your
/// tracing/log setup to send messages into the channel.
///
/// # Example
///
/// ```
/// # #[cfg(any(feature = "tracing", feature = "log"))]
/// # {
/// use clap_mcp::logging::log_channel;
///
/// let (tx, mut rx) = log_channel(16);
/// // Use tx with ClapMcpTracingLayer, pass rx to ClapMcpServeOptions::log_rx
/// # }
/// ```
pub fn log_channel(
    buffer: usize,
) -> (
    mpsc::Sender<LoggingMessageNotificationParams>,
    mpsc::Receiver<LoggingMessageNotificationParams>,
) {
    mpsc::channel(buffer)
}

/// Builds `LoggingMessageNotificationParams` for a log message.
///
/// # Example
///
/// ```
/// # #[cfg(any(feature = "tracing", feature = "log"))]
/// # {
/// use clap_mcp::logging::log_params;
/// use rust_mcp_sdk::schema::LoggingLevel;
///
/// let params = log_params(LoggingLevel::Info, Some("myapp".into()), "Hello");
/// assert_eq!(params.logger, Some("myapp".to_string()));
/// # }
/// ```
pub fn log_params(
    level: LoggingLevel,
    logger: Option<String>,
    message: impl Into<Value>,
) -> LoggingMessageNotificationParams {
    LoggingMessageNotificationParams {
        data: message.into(),
        level,
        logger,
        meta: None,
    }
}

#[cfg(feature = "tracing")]
mod tracing_layer {
    use super::*;
    use tracing::Subscriber;
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::Context;

    /// A tracing layer that forwards events to an MCP log channel.
    ///
    /// Add to your tracing subscriber to send `tracing::info!`, `tracing::debug!`, etc.
    /// to the MCP client via `notifications/message`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let (tx, rx) = log_channel(32);
    /// let layer = ClapMcpTracingLayer::new(tx).with_logger_name("myapp");
    /// tracing_subscriber::registry().with(layer).init();
    /// ```
    #[derive(Clone)]
    pub struct ClapMcpTracingLayer {
        tx: mpsc::Sender<LoggingMessageNotificationParams>,
        logger_name: String,
    }

    impl ClapMcpTracingLayer {
        /// Creates a new layer that sends to the given channel.
        pub fn new(tx: mpsc::Sender<LoggingMessageNotificationParams>) -> Self {
            Self {
                tx,
                logger_name: "app".to_string(),
            }
        }

        /// Sets the logger name (default: `"app"`).
        /// The name appears in the MCP log message's `logger` field.
        pub fn with_logger_name(mut self, name: impl Into<String>) -> Self {
            self.logger_name = name.into();
            self
        }
    }

    impl<S> Layer<S> for ClapMcpTracingLayer
    where
        S: Subscriber,
    {
        fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            let mut visitor = LogVisitor::default();
            event.record(&mut visitor);
            let message = visitor.message.unwrap_or_else(|| format!("{:?}", event));
            let level = level_to_mcp(match *event.metadata().level() {
                tracing::Level::TRACE => "trace",
                tracing::Level::DEBUG => "debug",
                tracing::Level::INFO => "info",
                tracing::Level::WARN => "warn",
                tracing::Level::ERROR => "error",
            });
            let params = log_params(level, Some(self.logger_name.clone()), message);
            let _ = self.tx.try_send(params);
        }
    }

    #[derive(Default)]
    struct LogVisitor {
        message: Option<String>,
    }

    impl tracing::field::Visit for LogVisitor {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                self.message = Some(format!("{:?}", value));
            }
        }

        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            if field.name() == "message" {
                self.message = Some(value.to_string());
            }
        }
    }
}

#[cfg(feature = "tracing")]
pub use tracing_layer::ClapMcpTracingLayer;

#[cfg(feature = "log")]
mod log_bridge {
    use super::*;
    use log::Log;
    use std::sync::Arc;

    /// A log crate implementation that forwards to an MCP log channel.
    ///
    /// Implement `log::Log` to capture `log::info!`, `log::debug!`, etc. and send
    /// them to the MCP client. Use with `log::set_logger` / `log::set_max_level`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let (tx, rx) = log_channel(32);
    /// let bridge = ClapMcpLogBridge::new(tx);
    /// log::set_logger(Box::leak(Box::new(bridge)));
    /// log::set_max_level(log::LevelFilter::Info);
    /// ```
    pub struct ClapMcpLogBridge {
        tx: Arc<mpsc::Sender<LoggingMessageNotificationParams>>,
        logger_name: String,
    }

    impl ClapMcpLogBridge {
        /// Creates a new bridge that sends to the given channel.
        /// Use `log_channel` to create the channel.
        pub fn new(tx: mpsc::Sender<LoggingMessageNotificationParams>) -> Self {
            Self {
                tx: Arc::new(tx),
                logger_name: "app".to_string(),
            }
        }

        /// Sets the logger name (default: `"app"`).
        /// The name appears in the MCP log message's `logger` field.
        pub fn with_logger_name(mut self, name: impl Into<String>) -> Self {
            self.logger_name = name.into();
            self
        }
    }

    impl Log for ClapMcpLogBridge {
        fn enabled(&self, _metadata: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            let level = level_to_mcp(match record.level() {
                log::Level::Trace => "trace",
                log::Level::Debug => "debug",
                log::Level::Info => "info",
                log::Level::Warn => "warn",
                log::Level::Error => "error",
            });
            let message = record.args().to_string();
            let params = log_params(level, Some(self.logger_name.clone()), message);
            let _ = self.tx.try_send(params);
        }

        fn flush(&self) {}
    }
}

#[cfg(feature = "log")]
pub use log_bridge::ClapMcpLogBridge;
