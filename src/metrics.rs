//! Shared metric types for processes, containers, and host resources.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kind of monitored target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Process,
    Container,
    Host,
}

/// A single sampled resource snapshot for one target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSample {
    pub kind: TargetKind,
    /// Process PID, container ID, or "host".
    pub id: String,
    /// Human-readable name (process name or container name).
    pub name: String,
    /// CPU usage percent (0–100+ for multi-core processes).
    pub cpu_percent: f32,
    /// Resident memory in bytes.
    pub memory_bytes: u64,
    /// Total memory available to the system / limit (bytes), if known.
    pub memory_limit_bytes: Option<u64>,
    /// Disk read bytes (cumulative counters from the source).
    pub disk_read_bytes: u64,
    /// Disk write bytes (cumulative).
    pub disk_write_bytes: u64,
    /// Network received bytes (cumulative).
    pub net_rx_bytes: u64,
    /// Network transmitted bytes (cumulative).
    pub net_tx_bytes: u64,
    /// Disk read rate over the last interval (bytes/s), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_read_bps: Option<f64>,
    /// Disk write rate over the last interval (bytes/s), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_write_bps: Option<f64>,
    /// Network receive rate over the last interval (bytes/s), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_rx_bps: Option<f64>,
    /// Network transmit rate over the last interval (bytes/s), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_tx_bps: Option<f64>,
    /// Sample timestamp (UTC).
    pub sampled_at: DateTime<Utc>,
    /// Optional status string (e.g. container state, process status).
    pub status: Option<String>,
}

impl ResourceSample {
    pub fn memory_percent(&self) -> Option<f32> {
        self.memory_limit_bytes
            .filter(|limit| *limit > 0)
            .map(|limit| (self.memory_bytes as f64 / limit as f64 * 100.0) as f32)
    }

    /// Combined disk I/O rate (B/s) when either direction is known.
    pub fn disk_total_bps(&self) -> Option<f64> {
        match (self.disk_read_bps, self.disk_write_bps) {
            (None, None) => None,
            (r, w) => Some(r.unwrap_or(0.0) + w.unwrap_or(0.0)),
        }
    }

    /// Combined network I/O rate (B/s) when either direction is known.
    pub fn net_total_bps(&self) -> Option<f64> {
        match (self.net_rx_bps, self.net_tx_bps) {
            (None, None) => None,
            (r, w) => Some(r.unwrap_or(0.0) + w.unwrap_or(0.0)),
        }
    }
}

/// Host-level aggregate snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSample {
    pub cpu_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_available_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    /// Host network receive rate (B/s) over the last interval, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_rx_bps: Option<f64>,
    /// Host network transmit rate (B/s) over the last interval, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_tx_bps: Option<f64>,
    pub load_avg_1: f64,
    pub load_avg_5: f64,
    pub load_avg_15: f64,
    pub sampled_at: DateTime<Utc>,
}

impl HostSample {
    pub fn memory_percent(&self) -> f32 {
        if self.memory_total_bytes == 0 {
            return 0.0;
        }
        (self.memory_used_bytes as f64 / self.memory_total_bytes as f64 * 100.0) as f32
    }
}

/// Full collection result for one tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub host: HostSample,
    pub targets: Vec<ResourceSample>,
    pub sampled_at: DateTime<Utc>,
}

/// Sort order for ranking consumers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortBy {
    #[default]
    Cpu,
    Memory,
    Disk,
    Network,
}

impl SortBy {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "mem" | "memory" | "ram" => Self::Memory,
            "disk" | "io" => Self::Disk,
            "net" | "network" => Self::Network,
            _ => Self::Cpu,
        }
    }
}

/// Cumulative I/O counters used to derive interval rates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IoCounters {
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
}

impl IoCounters {
    pub fn from_target(s: &ResourceSample) -> Self {
        Self {
            disk_read_bytes: s.disk_read_bytes,
            disk_write_bytes: s.disk_write_bytes,
            net_rx_bytes: s.net_rx_bytes,
            net_tx_bytes: s.net_tx_bytes,
        }
    }

    pub fn from_host(h: &HostSample) -> Self {
        Self {
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            net_rx_bytes: h.net_rx_bytes,
            net_tx_bytes: h.net_tx_bytes,
        }
    }
}

/// Previous I/O counters keyed for delta rate calculation between snapshots.
#[derive(Debug, Clone, Default)]
pub struct IoRateState {
    /// Wall-clock timestamp of the previous snapshot (`None` = first sample).
    pub prev_at: Option<DateTime<Utc>>,
    pub host: Option<IoCounters>,
    /// Keyed as `"{kind}:{id}"` so process/container IDs cannot collide.
    pub targets: HashMap<String, IoCounters>,
}

impl IoRateState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute B/s rates on `snapshot` from the previous counters, then store
    /// the current counters for the next tick.
    pub fn apply(&mut self, snapshot: &mut Snapshot) {
        let elapsed = self
            .prev_at
            .map(|prev| {
                (snapshot.sampled_at - prev)
                    .to_std()
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0)
            })
            .unwrap_or(0.0);

        if let Some(prev_host) = self.host {
            let (rx, tx) = rates_from_counters(
                prev_host.net_rx_bytes,
                snapshot.host.net_rx_bytes,
                prev_host.net_tx_bytes,
                snapshot.host.net_tx_bytes,
                elapsed,
            );
            snapshot.host.net_rx_bps = rx;
            snapshot.host.net_tx_bps = tx;
        }

        for t in &mut snapshot.targets {
            let key = target_io_key(t.kind, &t.id);
            if let Some(prev) = self.targets.get(&key).copied() {
                let (disk_r, disk_w) = rates_from_counters(
                    prev.disk_read_bytes,
                    t.disk_read_bytes,
                    prev.disk_write_bytes,
                    t.disk_write_bytes,
                    elapsed,
                );
                let (net_r, net_w) = rates_from_counters(
                    prev.net_rx_bytes,
                    t.net_rx_bytes,
                    prev.net_tx_bytes,
                    t.net_tx_bytes,
                    elapsed,
                );
                t.disk_read_bps = disk_r;
                t.disk_write_bps = disk_w;
                t.net_rx_bps = net_r;
                t.net_tx_bps = net_w;
            }
        }

        // Replace previous state with current counters (drop exited targets).
        self.prev_at = Some(snapshot.sampled_at);
        self.host = Some(IoCounters::from_host(&snapshot.host));
        self.targets.clear();
        for t in &snapshot.targets {
            self.targets
                .insert(target_io_key(t.kind, &t.id), IoCounters::from_target(t));
        }
    }
}

fn target_io_key(kind: TargetKind, id: &str) -> String {
    let k = match kind {
        TargetKind::Process => "process",
        TargetKind::Container => "container",
        TargetKind::Host => "host",
    };
    format!("{k}:{id}")
}

/// Bytes-per-second from a pair of cumulative counters over `elapsed_secs`.
///
/// Returns `None` when the interval is non-positive (first sample or clock skew).
/// Counter resets (curr < prev) yield a rate of `0.0` rather than wrapping.
pub fn io_rate_bps(prev: u64, curr: u64, elapsed_secs: f64) -> Option<f64> {
    if elapsed_secs <= 0.0 || !elapsed_secs.is_finite() {
        return None;
    }
    let delta = curr.saturating_sub(prev);
    Some(delta as f64 / elapsed_secs)
}

fn rates_from_counters(
    prev_a: u64,
    curr_a: u64,
    prev_b: u64,
    curr_b: u64,
    elapsed: f64,
) -> (Option<f64>, Option<f64>) {
    (
        io_rate_bps(prev_a, curr_a, elapsed),
        io_rate_bps(prev_b, curr_b, elapsed),
    )
}

/// Format a byte rate for display (e.g. `1.5 MB/s`). Missing rate → `—`.
pub fn format_bps(rate: Option<f64>) -> String {
    match rate {
        None => "—".into(),
        Some(bps) if !bps.is_finite() || bps < 0.0 => "—".into(),
        Some(bps) => {
            // Round to nearest whole byte/s for ByteSize; tiny rates stay visible as 0 B/s.
            let rounded = if bps > 0.0 && bps < 1.0 {
                1u64
            } else {
                bps.round().clamp(0.0, u64::MAX as f64) as u64
            };
            format!("{}/s", bytesize::ByteSize::b(rounded))
        }
    }
}

/// Format a read/write or rx/tx rate pair for table cells.
pub fn format_bps_pair(a: Option<f64>, b: Option<f64>) -> String {
    format!("{}/{}", format_bps(a), format_bps(b))
}

/// Build a snapshot view for prometheus/json export.
///
/// When neither filter nor top is set: full per-target inventory (host always
/// kept). Collection order is preserved; `sort_by` is ignored for membership
/// and order.
///
/// When either filter or top is set: apply [`filter_and_rank`] with `sort_by`
/// (same ranking as table). Host metrics are always included.
pub fn snapshot_for_export(
    snapshot: &Snapshot,
    name_filter: Option<&str>,
    sort_by: SortBy,
    top: Option<usize>,
) -> Snapshot {
    if name_filter.is_none() && top.is_none() {
        return snapshot.clone();
    }
    Snapshot {
        host: snapshot.host.clone(),
        targets: filter_and_rank(&snapshot.targets, name_filter, sort_by, top),
        sampled_at: snapshot.sampled_at,
    }
}

/// Filter and rank samples.
pub fn filter_and_rank(
    samples: &[ResourceSample],
    name_filter: Option<&str>,
    sort_by: SortBy,
    top: Option<usize>,
) -> Vec<ResourceSample> {
    let mut out: Vec<ResourceSample> = samples
        .iter()
        .filter(|s| match name_filter {
            Some(f) => {
                let f = f.to_ascii_lowercase();
                s.name.to_ascii_lowercase().contains(&f) || s.id.to_ascii_lowercase().contains(&f)
            }
            None => true,
        })
        .cloned()
        .collect();

    out.sort_by(|a, b| match sort_by {
        SortBy::Cpu => b
            .cpu_percent
            .partial_cmp(&a.cpu_percent)
            .unwrap_or(std::cmp::Ordering::Equal),

        SortBy::Memory => b.memory_bytes.cmp(&a.memory_bytes),

        SortBy::Disk => {
            // Prefer interval rates when available; otherwise cumulative totals.
            let ba = disk_sort_key(a);
            let bb = disk_sort_key(b);
            bb.partial_cmp(&ba).unwrap_or(std::cmp::Ordering::Equal)
        }

        SortBy::Network => {
            let ba = net_sort_key(a);
            let bb = net_sort_key(b);
            bb.partial_cmp(&ba).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    if let Some(n) = top {
        out.truncate(n);
    }

    out
}

fn disk_sort_key(s: &ResourceSample) -> f64 {
    s.disk_total_bps()
        .unwrap_or_else(|| s.disk_read_bytes.saturating_add(s.disk_write_bytes) as f64)
}

fn net_sort_key(s: &ResourceSample) -> f64 {
    s.net_total_bps()
        .unwrap_or_else(|| s.net_rx_bytes.saturating_add(s.net_tx_bytes) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn sample(name: &str, cpu: f32, mem: u64) -> ResourceSample {
        ResourceSample {
            kind: TargetKind::Process,
            id: "1".into(),
            name: name.into(),
            cpu_percent: cpu,
            memory_bytes: mem,
            memory_limit_bytes: Some(8 * 1024 * 1024 * 1024),
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

    fn sample_io(
        id: &str,
        name: &str,
        disk_r: u64,
        disk_w: u64,
        net_r: u64,
        net_w: u64,
        at: DateTime<Utc>,
    ) -> ResourceSample {
        ResourceSample {
            kind: TargetKind::Process,
            id: id.into(),
            name: name.into(),
            cpu_percent: 1.0,
            memory_bytes: 100,
            memory_limit_bytes: Some(1000),
            disk_read_bytes: disk_r,
            disk_write_bytes: disk_w,
            net_rx_bytes: net_r,
            net_tx_bytes: net_w,
            disk_read_bps: None,
            disk_write_bps: None,
            net_rx_bps: None,
            net_tx_bps: None,
            sampled_at: at,
            status: None,
        }
    }

    fn empty_host(at: DateTime<Utc>) -> HostSample {
        HostSample {
            cpu_percent: 0.0,
            memory_used_bytes: 0,
            memory_total_bytes: 1,
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

    #[test]
    fn filter_by_name() {
        let samples = vec![sample("nginx", 10.0, 100), sample("postgres", 50.0, 200)];
        let ranked = filter_and_rank(&samples, Some("post"), SortBy::Cpu, None);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].name, "postgres");
    }

    #[test]
    fn top_n_by_cpu() {
        let samples = vec![
            sample("a", 10.0, 100),
            sample("b", 90.0, 50),
            sample("c", 40.0, 300),
        ];
        let ranked = filter_and_rank(&samples, None, SortBy::Cpu, Some(2));
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].name, "b");
        assert_eq!(ranked[1].name, "c");
    }

    #[test]
    fn memory_percent() {
        let s = sample("x", 1.0, 1024);
        let pct = s.memory_percent().unwrap();
        assert!(pct > 0.0 && pct < 1.0);
    }

    #[test]
    fn memory_percent_none_when_limit_zero_or_missing() {
        // `limit > 0` must reject zero (mutant `>= 0` would incorrectly yield Some).
        let mut zero_limit = sample("x", 1.0, 50);
        zero_limit.memory_limit_bytes = Some(0);
        assert_eq!(zero_limit.memory_percent(), None);

        let mut missing = sample("x", 1.0, 50);
        missing.memory_limit_bytes = None;
        assert_eq!(missing.memory_percent(), None);
    }

    #[test]
    fn memory_percent_exact_ratio() {
        let mut s = sample("x", 1.0, 250);
        s.memory_limit_bytes = Some(1000);
        let pct = s.memory_percent().unwrap();
        assert!((pct - 25.0).abs() < 0.01, "got {pct}");
    }

    #[test]
    fn host_memory_percent_zero_total_and_ratio() {
        let mut h = empty_host(Utc::now());
        h.memory_total_bytes = 0;
        h.memory_used_bytes = 100;
        assert_eq!(h.memory_percent(), 0.0);

        h.memory_total_bytes = 1000;
        h.memory_used_bytes = 250;
        let pct = h.memory_percent();
        assert!((pct - 25.0).abs() < 0.01, "got {pct}");
    }

    #[test]
    fn disk_and_net_total_bps_sum_and_none() {
        let mut s = sample("io", 1.0, 1);
        assert_eq!(s.disk_total_bps(), None);
        assert_eq!(s.net_total_bps(), None);

        s.disk_read_bps = Some(10.0);
        s.disk_write_bps = Some(5.0);
        assert_eq!(s.disk_total_bps(), Some(15.0));
        // One-sided rates still produce a total (missing side = 0).
        s.disk_write_bps = None;
        assert_eq!(s.disk_total_bps(), Some(10.0));

        s.net_rx_bps = Some(7.0);
        s.net_tx_bps = Some(3.0);
        assert_eq!(s.net_total_bps(), Some(10.0));
        s.net_tx_bps = None;
        assert_eq!(s.net_total_bps(), Some(7.0));
    }

    #[test]
    fn sort_by_parse_aliases() {
        assert_eq!(SortBy::parse("cpu"), SortBy::Cpu);
        assert_eq!(SortBy::parse("CPU"), SortBy::Cpu);
        assert_eq!(SortBy::parse("unknown"), SortBy::Cpu);
        assert_eq!(SortBy::parse("mem"), SortBy::Memory);
        assert_eq!(SortBy::parse("memory"), SortBy::Memory);
        assert_eq!(SortBy::parse("RAM"), SortBy::Memory);
        assert_eq!(SortBy::parse("disk"), SortBy::Disk);
        assert_eq!(SortBy::parse("io"), SortBy::Disk);
        assert_eq!(SortBy::parse("net"), SortBy::Network);
        assert_eq!(SortBy::parse("network"), SortBy::Network);
    }

    #[test]
    fn io_rate_bps_happy_path() {
        // 10_000 bytes over 2 seconds → 5000 B/s
        assert_eq!(io_rate_bps(0, 10_000, 2.0), Some(5_000.0));
        // 5_000 bytes over 0.5s → 10_000 B/s
        assert_eq!(io_rate_bps(1_000, 6_000, 0.5), Some(10_000.0));
    }

    #[test]
    fn io_rate_bps_zero_or_negative_interval() {
        assert_eq!(io_rate_bps(0, 100, 0.0), None);
        assert_eq!(io_rate_bps(0, 100, -1.0), None);
        assert_eq!(io_rate_bps(0, 100, f64::NAN), None);
        assert_eq!(io_rate_bps(0, 100, f64::INFINITY), None);
    }

    #[test]
    fn io_rate_bps_counter_reset_yields_zero() {
        // curr < prev → saturating_sub → 0 delta
        assert_eq!(io_rate_bps(1_000, 100, 1.0), Some(0.0));
    }

    #[test]
    fn io_rate_bps_no_change() {
        assert_eq!(io_rate_bps(500, 500, 1.0), Some(0.0));
    }

    #[test]
    fn format_bps_missing_and_values() {
        assert_eq!(format_bps(None), "—");
        // Exact zero must stay 0 B/s (not promoted by a flipped `>= 0` sub-byte guard).
        assert_eq!(
            format_bps(Some(0.0)),
            format!("{}/s", bytesize::ByteSize::b(0))
        );
        assert_eq!(
            format_bps(Some(1024.0)),
            format!("{}/s", bytesize::ByteSize::b(1024))
        );
        // Sub-byte rates still render as at least 1 B/s so activity is visible
        assert_eq!(
            format_bps(Some(0.4)),
            format!("{}/s", bytesize::ByteSize::b(1))
        );
        // Boundary: exactly 1 B/s uses the normal rounding path, not the sub-byte promote.
        assert_eq!(
            format_bps(Some(1.0)),
            format!("{}/s", bytesize::ByteSize::b(1))
        );
        assert_eq!(
            format_bps(Some(2.0)),
            format!("{}/s", bytesize::ByteSize::b(2))
        );
    }

    #[test]
    fn format_bps_non_finite_and_negative_are_em_dash() {
        // Guard `!finite || bps < 0` — mutants that drop either arm still fail one case.
        assert_eq!(format_bps(Some(-1.0)), "—");
        assert_eq!(format_bps(Some(f64::NEG_INFINITY)), "—");
        assert_eq!(format_bps(Some(f64::NAN)), "—");
        assert_eq!(format_bps(Some(f64::INFINITY)), "—");
    }

    #[test]
    fn format_bps_pair_uses_em_dash_when_unknown() {
        assert_eq!(format_bps_pair(None, None), "—/—");
        let left = format!("{}/s", bytesize::ByteSize::b(0));
        let right = format!("{}/s", bytesize::ByteSize::b(1));
        let expected = format!("{left}/{right}");
        assert_eq!(format_bps_pair(Some(0.0), Some(1.0)), expected);
    }

    #[test]
    fn rate_state_first_snapshot_has_no_rates() {
        let t0 = Utc::now();
        let mut state = IoRateState::new();
        let mut snap = Snapshot {
            host: {
                let mut h = empty_host(t0);
                h.net_rx_bytes = 1000;
                h.net_tx_bytes = 2000;
                h
            },
            targets: vec![sample_io("1", "nginx", 100, 200, 10, 20, t0)],
            sampled_at: t0,
        };
        state.apply(&mut snap);
        assert!(snap.host.net_rx_bps.is_none());
        assert!(snap.targets[0].disk_read_bps.is_none());
    }

    #[test]
    fn rate_state_second_snapshot_computes_bps() {
        let t0 = Utc::now();
        let t1 = t0 + Duration::seconds(2);
        let mut state = IoRateState::new();

        let mut snap0 = Snapshot {
            host: {
                let mut h = empty_host(t0);
                h.net_rx_bytes = 1000;
                h.net_tx_bytes = 2000;
                h
            },
            targets: vec![sample_io("1", "nginx", 100, 200, 10, 20, t0)],
            sampled_at: t0,
        };
        state.apply(&mut snap0);

        let mut snap1 = Snapshot {
            host: {
                let mut h = empty_host(t1);
                h.net_rx_bytes = 3000; // +2000 over 2s → 1000 B/s
                h.net_tx_bytes = 6000; // +4000 over 2s → 2000 B/s
                h
            },
            targets: vec![sample_io("1", "nginx", 2100, 2200, 1010, 1020, t1)],
            sampled_at: t1,
        };
        state.apply(&mut snap1);

        assert_eq!(snap1.host.net_rx_bps, Some(1000.0));
        assert_eq!(snap1.host.net_tx_bps, Some(2000.0));
        assert_eq!(snap1.targets[0].disk_read_bps, Some(1000.0)); // +2000 / 2s
        assert_eq!(snap1.targets[0].disk_write_bps, Some(1000.0)); // +2000 / 2s
        assert_eq!(snap1.targets[0].net_rx_bps, Some(500.0)); // +1000 / 2s
        assert_eq!(snap1.targets[0].net_tx_bps, Some(500.0));
    }

    #[test]
    fn rate_state_new_target_has_no_rate_until_second_sighting() {
        let t0 = Utc::now();
        let t1 = t0 + Duration::seconds(1);
        let mut state = IoRateState::new();

        let mut snap0 = Snapshot {
            host: empty_host(t0),
            targets: vec![sample_io("1", "a", 0, 0, 0, 0, t0)],
            sampled_at: t0,
        };
        state.apply(&mut snap0);

        let mut snap1 = Snapshot {
            host: empty_host(t1),
            targets: vec![
                sample_io("1", "a", 100, 0, 0, 0, t1),
                sample_io("2", "new", 9999, 0, 0, 0, t1),
            ],
            sampled_at: t1,
        };
        state.apply(&mut snap1);

        assert_eq!(snap1.targets[0].disk_read_bps, Some(100.0));
        assert!(
            snap1.targets[1].disk_read_bps.is_none(),
            "brand-new target must not invent a rate from zero baseline"
        );
    }

    #[test]
    fn sort_by_disk_prefers_rates_over_cumulative() {
        // High cumulative but low rate should lose to low cumulative high rate.
        let mut low_rate = sample("cumul", 1.0, 1);
        low_rate.disk_read_bytes = 1_000_000;
        low_rate.disk_read_bps = Some(10.0);
        low_rate.disk_write_bps = Some(0.0);

        let mut high_rate = sample("fast", 1.0, 1);
        high_rate.disk_read_bytes = 100;
        high_rate.disk_read_bps = Some(50_000.0);
        high_rate.disk_write_bps = Some(0.0);

        let ranked = filter_and_rank(&[low_rate, high_rate], None, SortBy::Disk, None);
        assert_eq!(ranked[0].name, "fast");
    }

    #[test]
    fn sort_by_network_falls_back_to_cumulative_without_rates() {
        let mut a = sample("a", 1.0, 1);
        a.net_rx_bytes = 10;
        let mut b = sample("b", 1.0, 1);
        b.net_rx_bytes = 10_000;
        let ranked = filter_and_rank(&[a, b], None, SortBy::Network, Some(1));
        assert_eq!(ranked[0].name, "b");
    }

    fn multi_target_snapshot() -> Snapshot {
        let at = Utc::now();
        // Intentionally not sorted by CPU so order-preservation is testable.
        let mut low = sample("low-cpu", 10.0, 100);
        low.id = "1".into();
        let mut mid = sample("nginx", 40.0, 200);
        mid.id = "2".into();
        let mut high = sample("nginx-worker", 90.0, 300);
        high.id = "3".into();
        Snapshot {
            host: empty_host(at),
            targets: vec![low, mid, high],
            sampled_at: at,
        }
    }

    #[test]
    fn snapshot_for_export_no_filter_top_preserves_ids_and_order() {
        let snap = multi_target_snapshot();
        let out = snapshot_for_export(&snap, None, SortBy::Cpu, None);
        let ids: Vec<_> = out.targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "2", "3"]);
        assert_eq!(out.host.memory_total_bytes, snap.host.memory_total_bytes);
        assert_eq!(out.sampled_at, snap.sampled_at);
    }

    #[test]
    fn snapshot_for_export_sort_only_does_not_rerank() {
        // Sort flag alone must not re-order when filter/top unset.
        let snap = multi_target_snapshot();
        let out = snapshot_for_export(&snap, None, SortBy::Cpu, None);
        let names: Vec<_> = out.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["low-cpu", "nginx", "nginx-worker"]);
        // Contrasting: ranking would put high CPU first.
        let ranked = filter_and_rank(&snap.targets, None, SortBy::Cpu, None);
        assert_eq!(ranked[0].name, "nginx-worker");
        assert_ne!(out.targets[0].name, ranked[0].name);
    }

    #[test]
    fn snapshot_for_export_filter_only() {
        let snap = multi_target_snapshot();
        let out = snapshot_for_export(&snap, Some("nginx"), SortBy::Cpu, None);
        assert_eq!(out.targets.len(), 2);
        assert!(out.targets.iter().all(|t| t.name.contains("nginx")));
        // Host always present
        assert_eq!(out.host.memory_total_bytes, snap.host.memory_total_bytes);
    }

    #[test]
    fn snapshot_for_export_top_only_after_sort() {
        let snap = multi_target_snapshot();
        let out = snapshot_for_export(&snap, None, SortBy::Cpu, Some(1));
        assert_eq!(out.targets.len(), 1);
        assert_eq!(out.targets[0].name, "nginx-worker");
        assert!(out.host.memory_total_bytes > 0 || out.host.memory_total_bytes == 1);
    }

    #[test]
    fn snapshot_for_export_filter_and_top() {
        let snap = multi_target_snapshot();
        let out = snapshot_for_export(&snap, Some("nginx"), SortBy::Cpu, Some(1));
        assert_eq!(out.targets.len(), 1);
        assert_eq!(out.targets[0].name, "nginx-worker");
    }

    #[test]
    fn snapshot_for_export_top_zero_keeps_host_empty_targets() {
        let snap = multi_target_snapshot();
        let out = snapshot_for_export(&snap, None, SortBy::Cpu, Some(0));
        assert!(out.targets.is_empty());
        assert_eq!(out.host.memory_total_bytes, snap.host.memory_total_bytes);
    }

    #[test]
    fn snapshot_for_export_filter_matches_nothing_keeps_host() {
        let snap = multi_target_snapshot();
        let out = snapshot_for_export(&snap, Some("zzz-no-match"), SortBy::Memory, None);
        assert!(out.targets.is_empty());
        assert_eq!(out.host.memory_total_bytes, snap.host.memory_total_bytes);
    }
}
