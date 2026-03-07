//! Tests for logging integration.
//! Run with: cargo test --features tracing

#![cfg(any(feature = "tracing", feature = "log"))]

use clap_mcp::logging::{level_to_mcp, log_channel, log_params};
use rust_mcp_sdk::schema::LoggingLevel;

#[test]
fn test_level_to_mcp() {
    assert!(matches!(level_to_mcp("trace"), LoggingLevel::Debug));
    assert!(matches!(level_to_mcp("debug"), LoggingLevel::Debug));
    assert!(matches!(level_to_mcp("info"), LoggingLevel::Info));
    assert!(matches!(level_to_mcp("notice"), LoggingLevel::Notice));
    assert!(matches!(level_to_mcp("warn"), LoggingLevel::Notice));
    assert!(matches!(level_to_mcp("warning"), LoggingLevel::Notice));
    assert!(matches!(level_to_mcp("error"), LoggingLevel::Error));
    assert!(matches!(level_to_mcp("critical"), LoggingLevel::Critical));
    assert!(matches!(level_to_mcp("alert"), LoggingLevel::Alert));
    assert!(matches!(level_to_mcp("emergency"), LoggingLevel::Emergency));
    assert!(matches!(level_to_mcp("unknown"), LoggingLevel::Info));
}

#[test]
fn test_log_params() {
    let params = log_params(LoggingLevel::Info, Some("test".into()), "hello");
    assert_eq!(params.level, LoggingLevel::Info);
    assert_eq!(params.logger, Some("test".to_string()));
    assert_eq!(params.data.as_str(), Some("hello"));
}

#[test]
fn test_log_channel() {
    let (tx, mut rx) = log_channel(4);
    let params = log_params(LoggingLevel::Debug, None, "msg");
    tx.try_send(params).unwrap();
    let recv = rx.try_recv().unwrap();
    assert_eq!(recv.data.as_str(), Some("msg"));
}

#[cfg(feature = "tracing")]
#[test]
fn test_tracing_layer_forwards_events() {
    use clap_mcp::logging::ClapMcpTracingLayer;
    use tracing_subscriber::layer::SubscriberExt;

    let (tx, mut rx) = log_channel(4);
    let subscriber = tracing_subscriber::registry()
        .with(ClapMcpTracingLayer::new(tx).with_logger_name("trace-test"));

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("hello tracing");
    });

    let recv = rx.try_recv().unwrap();
    assert_eq!(recv.logger.as_deref(), Some("trace-test"));
    assert_eq!(recv.level, LoggingLevel::Info);
    assert_eq!(recv.data.as_str(), Some("hello tracing"));
}

#[cfg(feature = "log")]
#[test]
fn test_log_bridge_forwards_records() {
    use clap_mcp::logging::ClapMcpLogBridge;
    use log::{Log, Record};

    let (tx, mut rx) = log_channel(4);
    let bridge = ClapMcpLogBridge::new(tx).with_logger_name("log-test");
    let record = Record::builder()
        .args(format_args!("hello log"))
        .level(log::Level::Warn)
        .target("logging-tests")
        .build();

    assert!(bridge.enabled(record.metadata()));
    bridge.log(&record);
    bridge.flush();

    let recv = rx.try_recv().unwrap();
    assert_eq!(recv.logger.as_deref(), Some("log-test"));
    assert_eq!(recv.level, LoggingLevel::Notice);
    assert_eq!(recv.data.as_str(), Some("hello log"));
}
