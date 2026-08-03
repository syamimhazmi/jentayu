//! Build [`ServeConfig`] from CLI / env (I/O at the edge).

use super::policy::{
    auth_required as policy_auth_required, bind_allowed, effective_top, DEFAULT_BIND,
    DEFAULT_COLLECT_TIMEOUT, MAX_TOKEN_BYTES,
};
use crate::metrics::SortBy;
use anyhow::{bail, Context, Result};
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;
use tracing::info;

/// Environment variable for scrape Bearer token (wins over `--token-file` when non-empty).
pub const TOKEN_ENV: &str = "JENTAYU_METRICS_TOKEN";

/// Runtime configuration for the scrape server (token loaded once; no hot-reload).
#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub bind: SocketAddr,
    pub allow_remote: bool,
    /// Present when auth is required on every scrape (including loopback).
    pub token: Option<String>,
    pub name_filter: Option<String>,
    pub sort_by: SortBy,
    pub top: Option<usize>,
    /// Cooperative collect budget (inject short values in tests).
    pub collect_timeout: Duration,
}

impl ServeConfig {
    /// Whether scrapes must present a valid Bearer token.
    pub fn auth_configured(&self) -> bool {
        self.token.is_some()
    }

    pub fn effective_top(&self) -> usize {
        effective_top(self.top)
    }
}

/// Parse `--bind` as an IP literal [`SocketAddr`] (no DNS / hostnames).
pub fn parse_bind(bind: &str) -> Result<SocketAddr> {
    bind.parse::<SocketAddr>().with_context(|| {
        format!(
            "invalid --bind '{bind}': must be an IP literal with port \
             (e.g. {DEFAULT_BIND} or [::1]:9105); hostnames are not resolved"
        )
    })
}

/// Load token: non-empty trimmed `JENTAYU_METRICS_TOKEN` wins; else `--token-file`.
pub fn load_token(token_file: Option<&Path>) -> Result<Option<String>> {
    if let Ok(val) = std::env::var(TOKEN_ENV) {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            if trimmed.len() > MAX_TOKEN_BYTES {
                bail!("{TOKEN_ENV} exceeds max length of {MAX_TOKEN_BYTES} bytes");
            }
            return Ok(Some(trimmed.to_string()));
        }
    }

    let Some(path) = token_file else {
        return Ok(None);
    };

    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read --token-file {}", path.display()))?;
    if bytes.len() > MAX_TOKEN_BYTES {
        bail!(
            "--token-file {} exceeds max length of {MAX_TOKEN_BYTES} bytes",
            path.display()
        );
    }
    let text = String::from_utf8(bytes)
        .with_context(|| format!("--token-file {} is not valid UTF-8", path.display()))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        bail!(
            "--token-file {} is empty after trim; provide a non-empty token or omit the flag",
            path.display()
        );
    }
    Ok(Some(trimmed.to_string()))
}

/// Build and validate config from CLI-level inputs.
pub fn build_serve_config(
    bind: &str,
    allow_remote: bool,
    token_file: Option<&Path>,
    name_filter: Option<String>,
    sort_by: SortBy,
    top: Option<usize>,
) -> Result<ServeConfig> {
    build_serve_config_with_timeout(
        bind,
        allow_remote,
        token_file,
        name_filter,
        sort_by,
        top,
        DEFAULT_COLLECT_TIMEOUT,
    )
}

/// Same as [`build_serve_config`] with injectable timeout (tests / advanced).
pub fn build_serve_config_with_timeout(
    bind: &str,
    allow_remote: bool,
    token_file: Option<&Path>,
    name_filter: Option<String>,
    sort_by: SortBy,
    top: Option<usize>,
    collect_timeout: Duration,
) -> Result<ServeConfig> {
    let addr = parse_bind(bind)?;
    let token = load_token(token_file)?;
    let auth = token.is_some();
    bind_allowed(addr, allow_remote, auth).map_err(|e| anyhow::anyhow!("{e}"))?;

    let cfg = ServeConfig {
        bind: addr,
        allow_remote,
        token,
        name_filter,
        sort_by,
        top,
        collect_timeout,
    };

    info!(
        bind = %cfg.bind,
        auth = if policy_auth_required(cfg.auth_configured()) {
            "on"
        } else {
            "off"
        },
        allow_remote = cfg.allow_remote,
        effective_top = cfg.effective_top(),
        collect_timeout_ms = cfg.collect_timeout.as_millis() as u64,
        "serve config ready"
    );

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    // Serialize env-mutating tests.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parse_bind_ip_literal_ok() {
        let a = parse_bind("127.0.0.1:9105").unwrap();
        assert_eq!(a.port(), 9105);
        assert!(a.ip().is_loopback());
    }

    #[test]
    fn parse_bind_rejects_hostname() {
        let err = parse_bind("localhost:9105").unwrap_err().to_string();
        assert!(
            err.contains("IP literal") || err.contains("invalid"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn remote_bind_refused_without_dual_gate() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(TOKEN_ENV);
        let err = build_serve_config("0.0.0.0:9105", false, None, None, SortBy::Cpu, None)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("allow-remote") || err.contains("auth"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn loopback_ok_without_auth() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(TOKEN_ENV);
        let cfg = build_serve_config("127.0.0.1:0", false, None, None, SortBy::Cpu, None).unwrap();
        assert!(!cfg.auth_configured());
    }

    #[test]
    fn token_file_loads() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(TOKEN_ENV);
        let dir = std::env::temp_dir();
        let path = dir.join(format!("jentayu-token-test-{}", std::process::id()));
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "  file-secret  ").unwrap();
        }
        let tok = load_token(Some(&path)).unwrap().unwrap();
        assert_eq!(tok, "file-secret");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn env_wins_over_token_file() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var(TOKEN_ENV, "env-secret");
        let dir = std::env::temp_dir();
        let path = dir.join(format!("jentayu-token-test-env-{}", std::process::id()));
        {
            let mut f = std::fs::File::create(&path).unwrap();
            write!(f, "file-secret").unwrap();
        }
        let tok = load_token(Some(&path)).unwrap().unwrap();
        assert_eq!(tok, "env-secret");
        std::env::remove_var(TOKEN_ENV);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_token_file_errors() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(TOKEN_ENV);
        let path = std::env::temp_dir().join(format!("jentayu-token-empty-{}", std::process::id()));
        std::fs::write(&path, "   \n").unwrap();
        let err = load_token(Some(&path)).unwrap_err().to_string();
        assert!(err.contains("empty"), "unexpected: {err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn whitespace_only_env_falls_through_to_file() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var(TOKEN_ENV, "  \t  ");
        let path = std::env::temp_dir().join(format!("jentayu-token-ws-{}", std::process::id()));
        std::fs::write(&path, "from-file").unwrap();
        let tok = load_token(Some(&path)).unwrap().unwrap();
        assert_eq!(tok, "from-file");
        std::env::remove_var(TOKEN_ENV);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn whitespace_only_env_without_file_is_none() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var(TOKEN_ENV, "   ");
        assert!(load_token(None).unwrap().is_none());
        std::env::remove_var(TOKEN_ENV);
    }

    #[test]
    fn oversized_env_token_errors() {
        let _g = ENV_LOCK.lock().unwrap();
        let big = "x".repeat(MAX_TOKEN_BYTES + 1);
        std::env::set_var(TOKEN_ENV, &big);
        let err = load_token(None).unwrap_err().to_string();
        assert!(
            err.contains("max length") || err.contains(&MAX_TOKEN_BYTES.to_string()),
            "unexpected: {err}"
        );
        std::env::remove_var(TOKEN_ENV);
    }

    #[test]
    fn oversized_token_file_errors() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(TOKEN_ENV);
        let path = std::env::temp_dir().join(format!("jentayu-token-big-{}", std::process::id()));
        std::fs::write(&path, vec![b'a'; MAX_TOKEN_BYTES + 1]).unwrap();
        let err = load_token(Some(&path)).unwrap_err().to_string();
        assert!(
            err.contains("max length") || err.contains(&MAX_TOKEN_BYTES.to_string()),
            "unexpected: {err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_utf8_token_file_errors() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(TOKEN_ENV);
        let path = std::env::temp_dir().join(format!("jentayu-token-bin-{}", std::process::id()));
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
        let err = load_token(Some(&path)).unwrap_err().to_string();
        assert!(
            err.contains("UTF-8") || err.contains("utf-8") || err.contains("utf8"),
            "unexpected: {err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn remote_bind_ok_with_dual_gate_and_env() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var(TOKEN_ENV, "remote-secret");
        let cfg = build_serve_config("0.0.0.0:9105", true, None, None, SortBy::Cpu, None).unwrap();
        assert!(cfg.auth_configured());
        assert!(cfg.allow_remote);
        std::env::remove_var(TOKEN_ENV);
    }

    #[test]
    fn remote_bind_needs_auth_even_with_allow_remote() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(TOKEN_ENV);
        let err = build_serve_config("0.0.0.0:9105", true, None, None, SortBy::Cpu, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("auth"), "unexpected: {err}");
    }

    #[test]
    fn remote_bind_needs_allow_remote_even_with_auth() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var(TOKEN_ENV, "only-token");
        let err = build_serve_config("0.0.0.0:9105", false, None, None, SortBy::Cpu, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("allow-remote"), "unexpected: {err}");
        std::env::remove_var(TOKEN_ENV);
    }

    #[test]
    fn parse_bind_ipv6_loopback() {
        let a = parse_bind("[::1]:9105").unwrap();
        assert_eq!(a.port(), 9105);
        assert!(a.ip().is_loopback());
    }
}
