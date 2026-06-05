//! OAuth **client** configuration helpers for calling remote MCP servers.
//!
//! clap-mcp does not embed an authorization server. See [docs/oauth.md](https://github.com/canardleteer/clap-mcp/blob/main/docs/oauth.md).

use rmcp::transport::auth::OAuthClientConfig;
use std::env;
use thiserror::Error;

/// Environment variable: OIDC issuer URL (discovery base).
pub const OAUTH_ISSUER_ENV: &str = "CLAP_MCP_OAUTH_ISSUER";
/// Environment variable: OAuth client id.
pub const OAUTH_CLIENT_ID_ENV: &str = "CLAP_MCP_OAUTH_CLIENT_ID";
/// Environment variable: OAuth client secret (optional for public PKCE clients).
pub const OAUTH_CLIENT_SECRET_ENV: &str = "CLAP_MCP_OAUTH_CLIENT_SECRET";
/// Environment variable: OAuth redirect URI for desktop/CLI flows.
pub const OAUTH_REDIRECT_URI_ENV: &str = "CLAP_MCP_OAUTH_REDIRECT_URI";
/// Environment variable: Space-separated OAuth scopes.
pub const OAUTH_SCOPES_ENV: &str = "CLAP_MCP_OAUTH_SCOPES";

/// Default scopes when [`OAUTH_SCOPES_ENV`] is unset.
pub const DEFAULT_OAUTH_SCOPES: &[&str] = &["openid", "profile"];

/// OAuth client settings loaded from environment variables.
#[derive(Debug, Clone)]
pub struct EnvConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

/// Error loading [`EnvConfig`] from the environment.
#[derive(Debug, Error)]
pub enum FromEnvError {
    #[error("missing required environment variable `{0}`")]
    MissingVar(&'static str),
    #[error("environment variable `{var}` is set but empty")]
    EmptyVar { var: &'static str },
}

impl EnvConfig {
    /// Load client configuration from `CLAP_MCP_OAUTH_*` environment variables.
    ///
    /// Required: [`OAUTH_CLIENT_ID_ENV`], [`OAUTH_REDIRECT_URI_ENV`].
    /// Optional: [`OAUTH_CLIENT_SECRET_ENV`], [`OAUTH_SCOPES_ENV`] (defaults to [`DEFAULT_OAUTH_SCOPES`]).
    /// [`OAUTH_ISSUER_ENV`] is stored for discovery wiring; pass it to rmcp's authorization flow.
    pub fn from_env() -> Result<Self, FromEnvError> {
        let issuer = required_var(OAUTH_ISSUER_ENV)?;
        let client_id = required_var(OAUTH_CLIENT_ID_ENV)?;
        let redirect_uri = required_var(OAUTH_REDIRECT_URI_ENV)?;
        let client_secret = optional_nonempty_var(OAUTH_CLIENT_SECRET_ENV);
        let scopes = env::var(OAUTH_SCOPES_ENV)
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.split_whitespace().map(str::to_string).collect())
            .unwrap_or_else(|| DEFAULT_OAUTH_SCOPES.iter().map(|s| s.to_string()).collect());
        Ok(Self {
            issuer,
            client_id,
            client_secret,
            redirect_uri,
            scopes,
        })
    }

    /// Convert to rmcp's [`OAuthClientConfig`] (issuer is not included — use [`Self::issuer`] with rmcp discovery).
    pub fn oauth_client_config(&self) -> OAuthClientConfig {
        let mut cfg = OAuthClientConfig::new(&self.client_id, &self.redirect_uri);
        if let Some(ref secret) = self.client_secret {
            cfg = cfg.with_client_secret(secret.clone());
        }
        cfg.with_scopes(self.scopes.clone())
    }
}

fn required_var(name: &'static str) -> Result<String, FromEnvError> {
    match env::var(name) {
        Ok(v) if v.is_empty() => Err(FromEnvError::EmptyVar { var: name }),
        Ok(v) => Ok(v),
        Err(_) => Err(FromEnvError::MissingVar(name)),
    }
}

fn optional_nonempty_var(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|s| !s.is_empty())
}

pub use rmcp::transport::{StreamableHttpClientTransport, auth::AuthClient};
