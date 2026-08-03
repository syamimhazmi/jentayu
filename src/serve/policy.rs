//! Pure bind, ranking, and auth-token policy for `jentayu serve`.
//!
//! No I/O, no axum — unit-tested and mutation-tested.

use crate::metrics::{filter_and_rank, Snapshot, SortBy};
use std::net::SocketAddr;
use std::time::Duration;
use thiserror::Error;

/// Default bind address string (IP literal).
pub const DEFAULT_BIND: &str = "127.0.0.1:9105";

/// Default top-N when `--top` is unset (serve only).
pub const DEFAULT_TOP: usize = 50;

/// Hard cap on per-scrape targets even if flags ask for more.
pub const HARD_CAP: usize = 500;

/// Production default collect budget (cooperative — see module docs / SECURITY).
pub const DEFAULT_COLLECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Max accepted token length (env or file) at load time.
pub const MAX_TOKEN_BYTES: usize = 8 * 1024;

/// Why a bind address was refused at startup.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BindPolicyError {
    #[error(
        "non-loopback bind requires --allow-remote and auth \
         (set JENTAYU_METRICS_TOKEN or --token-file)"
    )]
    RemoteNeedsAuthAndFlag,
    #[error(
        "non-loopback bind with --allow-remote still requires auth \
         (set JENTAYU_METRICS_TOKEN or --token-file)"
    )]
    RemoteNeedsAuth,
    #[error(
        "non-loopback bind with auth still requires --allow-remote \
         (refusing open network exposure by default)"
    )]
    RemoteNeedsAllowRemote,
}

/// True when `addr` is loopback (`IpAddr::is_loopback`: IPv4 127.0.0.0/8, IPv6 ::1).
///
/// `0.0.0.0` / `::` are **not** loopback.
pub fn is_loopback(addr: SocketAddr) -> bool {
    addr.ip().is_loopback()
}

/// Resolve serve top-N: default 50 when unset, then hard-cap at 500.
pub fn effective_top(top: Option<usize>) -> usize {
    top.unwrap_or(DEFAULT_TOP).min(HARD_CAP)
}

/// Startup bind gate: loopback always ok; remote needs `--allow-remote` **and** auth.
pub fn bind_allowed(
    addr: SocketAddr,
    allow_remote: bool,
    auth_configured: bool,
) -> Result<(), BindPolicyError> {
    if is_loopback(addr) {
        return Ok(());
    }
    match (allow_remote, auth_configured) {
        (true, true) => Ok(()),
        (false, false) => Err(BindPolicyError::RemoteNeedsAuthAndFlag),
        (true, false) => Err(BindPolicyError::RemoteNeedsAuth),
        (false, true) => Err(BindPolicyError::RemoteNeedsAllowRemote),
    }
}

/// Whether handlers must require a Bearer token (token was loaded at startup).
pub fn auth_required(auth_configured: bool) -> bool {
    auth_configured
}

/// Rank a full snapshot for serve export (always apply top; default + hard cap).
///
/// Unlike [`crate::metrics::snapshot_for_export`], this **never** full-dumps when
/// filter/top are unset.
pub fn snapshot_for_serve(
    snapshot: &Snapshot,
    name_filter: Option<&str>,
    sort_by: SortBy,
    top: Option<usize>,
) -> Snapshot {
    let n = effective_top(top);
    Snapshot {
        host: snapshot.host.clone(),
        targets: filter_and_rank(&snapshot.targets, name_filter, sort_by, Some(n)),
        sampled_at: snapshot.sampled_at,
    }
}

/// Constant-time equality when lengths match; length mismatch returns false
/// without comparing content (length is observable — acceptable for this model).
pub fn tokens_equal(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Parse `Authorization` value: case-insensitive `Bearer` + single space + token.
///
/// Returns `None` if the scheme is wrong or the token part is empty.
pub fn parse_bearer_authorization(header: &str) -> Option<&str> {
    let header = header.trim();
    let (scheme, rest) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    // Single-space split: token is the remainder (may contain spaces — rare).
    let token = rest.trim_start();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{HostSample, ResourceSample, TargetKind};
    use chrono::Utc;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    fn addr_v4(a: u8, b: u8, c: u8, d: u8, port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(a, b, c, d)), port)
    }

    fn sample(name: &str, cpu: f32, id: &str) -> ResourceSample {
        ResourceSample {
            kind: TargetKind::Process,
            id: id.into(),
            name: name.into(),
            cpu_percent: cpu,
            memory_bytes: 1,
            memory_limit_bytes: Some(100),
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            disk_read_bps: None,
            disk_write_bps: None,
            net_rx_bps: None,
            net_tx_bps: None,
            sampled_at: Utc::now(),
            status: None,
        }
    }

    fn empty_host() -> HostSample {
        let at = Utc::now();
        HostSample {
            cpu_percent: 1.0,
            memory_used_bytes: 10,
            memory_total_bytes: 100,
            swap_used_bytes: 0,
            swap_total_bytes: 0,
            disk_total_bytes: 0,
            disk_available_bytes: 0,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            net_rx_bps: None,
            net_tx_bps: None,
            load_avg_1: 0.0,
            load_avg_5: 0.0,
            load_avg_15: 0.0,
            sampled_at: at,
        }
    }

    fn multi_snap() -> Snapshot {
        Snapshot {
            host: empty_host(),
            targets: vec![
                sample("low", 10.0, "1"),
                sample("nginx", 40.0, "2"),
                sample("nginx-worker", 90.0, "3"),
            ],
            sampled_at: Utc::now(),
        }
    }

    #[test]
    fn max_token_bytes_is_8_kib() {
        // Kill mutants that flip the 8 * 1024 constant.
        assert_eq!(MAX_TOKEN_BYTES, 8192);
    }

    #[test]
    fn locked_serve_defaults_match_plan() {
        assert_eq!(DEFAULT_BIND, "127.0.0.1:9105");
        assert_eq!(DEFAULT_TOP, 50);
        assert_eq!(HARD_CAP, 500);
        assert_eq!(DEFAULT_COLLECT_TIMEOUT, Duration::from_secs(2));
    }

    #[test]
    fn effective_top_default_is_fifty() {
        assert_eq!(effective_top(None), 50);
    }

    #[test]
    fn effective_top_hard_cap_500() {
        assert_eq!(effective_top(Some(600)), 500);
        assert_eq!(effective_top(Some(500)), 500);
        assert_eq!(effective_top(Some(501)), 500);
    }

    #[test]
    fn effective_top_zero_and_small() {
        assert_eq!(effective_top(Some(0)), 0);
        assert_eq!(effective_top(Some(3)), 3);
        assert_eq!(effective_top(Some(50)), 50);
    }

    #[test]
    fn loopback_ipv4_and_ipv6() {
        assert!(is_loopback(addr_v4(127, 0, 0, 1, 9105)));
        assert!(is_loopback(addr_v4(127, 1, 2, 3, 1)));
        assert!(is_loopback(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            9105
        )));
    }

    #[test]
    fn not_loopback_any_or_public() {
        assert!(!is_loopback(addr_v4(0, 0, 0, 0, 9105)));
        assert!(!is_loopback(addr_v4(1, 2, 3, 4, 9105)));
        assert!(!is_loopback(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            9105
        )));
    }

    #[test]
    fn bind_loopback_always_ok() {
        let a = addr_v4(127, 0, 0, 1, 9105);
        assert!(bind_allowed(a, false, false).is_ok());
        assert!(bind_allowed(a, true, false).is_ok());
        assert!(bind_allowed(a, false, true).is_ok());
    }

    #[test]
    fn bind_remote_dual_gate() {
        let a = addr_v4(0, 0, 0, 0, 9105);
        assert_eq!(
            bind_allowed(a, false, false),
            Err(BindPolicyError::RemoteNeedsAuthAndFlag)
        );
        assert_eq!(
            bind_allowed(a, true, false),
            Err(BindPolicyError::RemoteNeedsAuth)
        );
        assert_eq!(
            bind_allowed(a, false, true),
            Err(BindPolicyError::RemoteNeedsAllowRemote)
        );
        assert!(bind_allowed(a, true, true).is_ok());
    }

    #[test]
    fn auth_required_tracks_token() {
        assert!(!auth_required(false));
        assert!(auth_required(true));
    }

    #[test]
    fn snapshot_for_serve_always_ranks_and_defaults_top() {
        let snap = multi_snap();
        // Without explicit top: default 50 keeps all 3, but ordered by CPU.
        let out = snapshot_for_serve(&snap, None, SortBy::Cpu, None);
        assert_eq!(out.targets.len(), 3);
        assert_eq!(out.targets[0].name, "nginx-worker");
        assert_eq!(out.host.memory_total_bytes, snap.host.memory_total_bytes);
    }

    #[test]
    fn snapshot_for_serve_applies_top_and_filter() {
        let snap = multi_snap();
        let out = snapshot_for_serve(&snap, Some("nginx"), SortBy::Cpu, Some(1));
        assert_eq!(out.targets.len(), 1);
        assert_eq!(out.targets[0].name, "nginx-worker");
    }

    #[test]
    fn snapshot_for_serve_hard_cap() {
        let mut targets = Vec::new();
        for i in 0..600 {
            targets.push(sample(&format!("p{i}"), i as f32, &format!("{i}")));
        }
        let snap = Snapshot {
            host: empty_host(),
            targets,
            sampled_at: Utc::now(),
        };
        let out = snapshot_for_serve(&snap, None, SortBy::Cpu, Some(9999));
        assert_eq!(out.targets.len(), HARD_CAP);
    }

    #[test]
    fn snapshot_for_serve_top_zero_keeps_host() {
        let snap = multi_snap();
        let out = snapshot_for_serve(&snap, None, SortBy::Cpu, Some(0));
        assert!(out.targets.is_empty());
        assert_eq!(out.host.cpu_percent, snap.host.cpu_percent);
    }

    #[test]
    fn tokens_equal_matches() {
        assert!(tokens_equal("secret", "secret"));
        assert!(!tokens_equal("secret", "secreT"));
        assert!(!tokens_equal("ab", "abc"));
        assert!(!tokens_equal("", "x"));
        assert!(tokens_equal("", ""));
    }

    #[test]
    fn parse_bearer_case_insensitive() {
        assert_eq!(parse_bearer_authorization("Bearer tok"), Some("tok"));
        assert_eq!(parse_bearer_authorization("bearer tok"), Some("tok"));
        assert_eq!(parse_bearer_authorization("BEARER tok"), Some("tok"));
        assert_eq!(parse_bearer_authorization("Basic tok"), None);
        assert_eq!(parse_bearer_authorization("Bearer"), None);
        assert_eq!(parse_bearer_authorization("Bearer "), None);
        // Outer trim on the header value; leading spaces after scheme are stripped.
        assert_eq!(parse_bearer_authorization("  Bearer  abc  "), Some("abc"));
    }
}
