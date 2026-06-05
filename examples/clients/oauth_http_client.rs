//! OAuth HTTP MCP client sketch (`http-oauth` feature).
//!
//! Full browser PKCE is manual; this binary documents the rmcp `AuthClient` entrypoint.

#[cfg(feature = "http-oauth")]
fn main() {
    eprintln!(
        "See docs/oauth.md. Wire AuthClient + StreamableHttpClientTransport to your OAuth-protected MCP URL."
    );
    let _type_check: Option<clap_mcp::oauth::AuthClient<reqwest::Client>> = None;
}

#[cfg(not(feature = "http-oauth"))]
fn main() {
    eprintln!("Build with --features http-oauth");
    std::process::exit(1);
}
