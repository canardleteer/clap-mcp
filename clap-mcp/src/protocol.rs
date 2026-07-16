//! MCP protocol versions clap-mcp supports and conformance-tests.
//!
//! Keep in sync with `cargo xtask conformance` (`active` @ `2025-11-25` and
//! `draft` @ `draft` / `2026-07-28`). See [`docs/conformance-baseline.md`].
//!
//! [`docs/conformance-baseline.md`]: ../../docs/conformance-baseline.md

use rmcp::model::ProtocolVersion;

/// Stable MCP protocol version (primary advertise / fallback).
pub const PROTOCOL_VERSION_STABLE: ProtocolVersion = ProtocolVersion::V_2025_11_25;

/// Draft MCP protocol version exercised by the draft conformance pass.
pub const PROTOCOL_VERSION_DRAFT: ProtocolVersion = ProtocolVersion::V_2026_07_28;

/// Protocol versions clap-mcp advertises and accepts in `initialize` negotiation.
///
/// Matches the dual conformance gate (`2025-11-25` and draft `2026-07-28`). Older
/// rmcp `KNOWN_VERSIONS` entries are not echoed; clients requesting them receive
/// [`PROTOCOL_VERSION_STABLE`] per the MCP lifecycle rules.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[ProtocolVersion] =
    &[PROTOCOL_VERSION_STABLE, PROTOCOL_VERSION_DRAFT];

/// Negotiate a protocol version for clap-mcp servers.
///
/// If `client_requested` is in [`SUPPORTED_PROTOCOL_VERSIONS`], it is echoed.
/// Otherwise the server falls back to [`PROTOCOL_VERSION_STABLE`].
pub fn negotiate_protocol_version(client_requested: &ProtocolVersion) -> ProtocolVersion {
    if SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .any(|v| v.as_str() == client_requested.as_str())
    {
        client_requested.clone()
    } else {
        tracing::warn!(
            client_requested = %client_requested,
            server_fallback = %PROTOCOL_VERSION_STABLE,
            "client requested a protocol version outside clap-mcp's conformance set; falling back"
        );
        PROTOCOL_VERSION_STABLE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echoes_stable_and_draft() {
        assert_eq!(
            negotiate_protocol_version(&PROTOCOL_VERSION_STABLE).as_str(),
            "2025-11-25"
        );
        assert_eq!(
            negotiate_protocol_version(&PROTOCOL_VERSION_DRAFT).as_str(),
            "2026-07-28"
        );
    }

    #[test]
    fn falls_back_for_older_known_versions() {
        for older in [
            ProtocolVersion::V_2024_11_05,
            ProtocolVersion::V_2025_03_26,
            ProtocolVersion::V_2025_06_18,
        ] {
            assert_eq!(
                negotiate_protocol_version(&older).as_str(),
                "2025-11-25",
                "unexpected echo for {older}"
            );
        }
    }

    #[test]
    fn falls_back_for_unknown_version() {
        let odd = serde_json::from_value::<ProtocolVersion>(serde_json::json!("2099-01-01"))
            .expect("parse");
        assert_eq!(negotiate_protocol_version(&odd).as_str(), "2025-11-25");
    }
}
