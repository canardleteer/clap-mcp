//! OAuth HTTP MCP client sketch (`http-oauth` feature).
//!
//! Full browser PKCE is manual; this binary documents env-based client config.

#[cfg(feature = "http-oauth")]
fn main() {
    match clap_mcp::oauth::EnvConfig::from_env() {
        Ok(cfg) => {
            let _oauth = cfg.oauth_client_config();
            eprintln!(
                "Loaded OAuth client config for issuer {} (see docs/oauth.md). \
                 Wire AuthClient + StreamableHttpClientTransport to your MCP URL.",
                cfg.issuer
            );
            let _type_check: Option<clap_mcp::oauth::AuthClient<reqwest::Client>> = None;
        }
        Err(e) => {
            eprintln!("{e}");
            eprintln!(
                "Set CLAP_MCP_OAUTH_ISSUER, CLAP_MCP_OAUTH_CLIENT_ID, CLAP_MCP_OAUTH_REDIRECT_URI (see docs/oauth.md)."
            );
            std::process::exit(1);
        }
    }
}

#[cfg(not(feature = "http-oauth"))]
fn main() {
    eprintln!("Build with --features http-oauth");
    std::process::exit(1);
}
