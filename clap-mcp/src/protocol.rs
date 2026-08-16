//! When you add a version here, run `cargo xtask conformance` for that version
//! as far as the harness and baseline allow. Do not advertise versions that
//! have not been exercised.
//!
//! MCP `2026-07-28` is the current released protocol version. `draft` is a
//! separate, evolving specification directory and harness suite, not an alias
//! for that dated release. Do not negotiate or advertise a `draft` protocol
//! string until it is conformance-tested as its own identifier.

use rmcp::model::ProtocolVersion;

/// Previous released MCP protocol version `2025-11-25` (initialize fallback).
pub const PROTOCOL_VERSION_STABLE: ProtocolVersion = ProtocolVersion::V_2025_11_25;

/// Current released MCP protocol version `2026-07-28` (stateless core).
///
/// This is the dated release published on 2026-07-28. It is not the evolving
/// `draft` specification tree or harness suite.
pub const PROTOCOL_VERSION_CURRENT: ProtocolVersion = ProtocolVersion::V_2026_07_28;

/// Protocol versions clap-mcp advertises and accepts.
///
/// Returned from `ServerHandler::supported_protocol_versions` so stdio
/// `initialize`, Streamable HTTP discover/negotiate, and per-request version
/// checks share this set. Clients that request a version outside it receive
/// [`PROTOCOL_VERSION_STABLE`].
pub const SUPPORTED_PROTOCOL_VERSIONS: &[ProtocolVersion] =
    &[PROTOCOL_VERSION_STABLE, PROTOCOL_VERSION_CURRENT];

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
    fn echoes_stable_and_current() {
        assert_eq!(
            negotiate_protocol_version(&PROTOCOL_VERSION_STABLE).as_str(),
            "2025-11-25"
        );
        assert_eq!(
            negotiate_protocol_version(&PROTOCOL_VERSION_CURRENT).as_str(),
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

    #[test]
    fn does_not_treat_draft_string_as_supported() {
        let draft =
            serde_json::from_value::<ProtocolVersion>(serde_json::json!("draft")).expect("parse");
        assert_eq!(
            negotiate_protocol_version(&draft).as_str(),
            "2025-11-25",
            "draft is a separate evolving identifier, not a dated release clap-mcp advertises"
        );
    }
}
