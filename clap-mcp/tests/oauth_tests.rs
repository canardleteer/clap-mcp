#![cfg(feature = "http-oauth")]

use clap_mcp::oauth::{
    DEFAULT_OAUTH_SCOPES, EnvConfig, FromEnvError, OAUTH_CLIENT_ID_ENV, OAUTH_CLIENT_SECRET_ENV,
    OAUTH_ISSUER_ENV, OAUTH_REDIRECT_URI_ENV, OAUTH_SCOPES_ENV,
};
use std::sync::Mutex;

static OAUTH_ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    vars: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn set(vars: &[(&'static str, Option<&str>)]) -> Self {
        let saved = vars
            .iter()
            .map(|(name, _)| (*name, std::env::var(*name).ok()))
            .collect();
        for (name, value) in vars {
            match value {
                Some(v) => unsafe { std::env::set_var(name, v) },
                None => unsafe { std::env::remove_var(name) },
            }
        }
        Self { vars: saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.vars {
            match value {
                Some(v) => unsafe { std::env::set_var(name, v) },
                None => unsafe { std::env::remove_var(name) },
            }
        }
    }
}

fn happy_env() -> EnvGuard {
    EnvGuard::set(&[
        (OAUTH_ISSUER_ENV, Some("https://issuer.example")),
        (OAUTH_CLIENT_ID_ENV, Some("client-id")),
        (OAUTH_REDIRECT_URI_ENV, Some("http://127.0.0.1/callback")),
        (OAUTH_CLIENT_SECRET_ENV, None),
        (OAUTH_SCOPES_ENV, None),
    ])
}

#[test]
fn env_config_loads_required_fields_and_default_scopes() {
    let _guard = OAUTH_ENV_LOCK.lock().unwrap();
    let _env = happy_env();
    let cfg = EnvConfig::from_env().expect("valid env");
    assert_eq!(cfg.issuer, "https://issuer.example");
    assert_eq!(cfg.client_id, "client-id");
    assert_eq!(cfg.redirect_uri, "http://127.0.0.1/callback");
    assert!(cfg.client_secret.is_none());
    assert_eq!(
        cfg.scopes,
        DEFAULT_OAUTH_SCOPES
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    );

    let oauth = cfg.oauth_client_config();
    assert_eq!(oauth.client_id, "client-id");
    assert_eq!(oauth.redirect_uri, "http://127.0.0.1/callback");
    assert!(oauth.client_secret.is_none());
    assert_eq!(oauth.scopes, cfg.scopes);
}

#[test]
fn env_config_parses_custom_scopes_and_secret() {
    let _guard = OAUTH_ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::set(&[
        (OAUTH_ISSUER_ENV, Some("https://issuer.example")),
        (OAUTH_CLIENT_ID_ENV, Some("client-id")),
        (OAUTH_REDIRECT_URI_ENV, Some("http://127.0.0.1/callback")),
        (OAUTH_CLIENT_SECRET_ENV, Some("secret")),
        (OAUTH_SCOPES_ENV, Some("openid email custom")),
    ]);
    let cfg = EnvConfig::from_env().expect("valid env");
    assert_eq!(cfg.client_secret.as_deref(), Some("secret"));
    assert_eq!(cfg.scopes, vec!["openid", "email", "custom"]);

    let oauth = cfg.oauth_client_config();
    assert_eq!(oauth.client_secret.as_deref(), Some("secret"));
}

#[test]
fn env_config_treats_empty_secret_as_absent() {
    let _guard = OAUTH_ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::set(&[
        (OAUTH_ISSUER_ENV, Some("https://issuer.example")),
        (OAUTH_CLIENT_ID_ENV, Some("client-id")),
        (OAUTH_REDIRECT_URI_ENV, Some("http://127.0.0.1/callback")),
        (OAUTH_CLIENT_SECRET_ENV, Some("")),
        (OAUTH_SCOPES_ENV, None),
    ]);
    let cfg = EnvConfig::from_env().expect("valid env");
    assert!(cfg.client_secret.is_none());
}

#[test]
fn env_config_reports_missing_required_vars() {
    let _guard = OAUTH_ENV_LOCK.lock().unwrap();
    for missing in [
        OAUTH_ISSUER_ENV,
        OAUTH_CLIENT_ID_ENV,
        OAUTH_REDIRECT_URI_ENV,
    ] {
        let _env = EnvGuard::set(&[
            (OAUTH_ISSUER_ENV, Some("https://issuer.example")),
            (OAUTH_CLIENT_ID_ENV, Some("client-id")),
            (OAUTH_REDIRECT_URI_ENV, Some("http://127.0.0.1/callback")),
        ]);
        unsafe {
            std::env::remove_var(missing);
        }
        let err = EnvConfig::from_env().expect_err("missing var should error");
        assert!(matches!(err, FromEnvError::MissingVar(name) if name == missing));
    }
}

#[test]
fn env_config_reports_empty_required_vars() {
    let _guard = OAUTH_ENV_LOCK.lock().unwrap();
    for empty in [
        OAUTH_ISSUER_ENV,
        OAUTH_CLIENT_ID_ENV,
        OAUTH_REDIRECT_URI_ENV,
    ] {
        let _env = happy_env();
        unsafe {
            std::env::set_var(empty, "");
        }
        let err = EnvConfig::from_env().expect_err("empty var should error");
        assert!(matches!(err, FromEnvError::EmptyVar { var } if var == empty));
    }
}
