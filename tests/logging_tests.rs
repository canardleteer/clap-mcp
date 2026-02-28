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
    assert_eq!(
        params.data.as_str(),
        Some("hello")
    );
}

#[test]
fn test_log_channel() {
    let (tx, mut rx) = log_channel(4);
    let params = log_params(LoggingLevel::Debug, None, "msg");
    tx.try_send(params).unwrap();
    let recv = rx.try_recv().unwrap();
    assert_eq!(recv.data.as_str(), Some("msg"));
}
