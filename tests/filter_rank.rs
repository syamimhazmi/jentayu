//! Integration-style tests for ranking helpers.

use chrono::Utc;
use jentayu::metrics::{filter_and_rank, ResourceSample, SortBy, TargetKind};

fn s(name: &str, cpu: f32, mem: u64, disk: u64, net: u64) -> ResourceSample {
    ResourceSample {
        kind: TargetKind::Process,
        id: name.to_string(),
        name: name.into(),
        cpu_percent: cpu,
        memory_bytes: mem,
        memory_limit_bytes: Some(8 * 1024 * 1024 * 1024),
        disk_read_bytes: disk,
        disk_write_bytes: 0,
        net_rx_bytes: net,
        net_tx_bytes: 0,
        disk_read_bps: None,
        disk_write_bps: None,
        net_rx_bps: None,
        net_tx_bps: None,
        sampled_at: Utc::now(),
        status: None,
    }
}

#[test]
fn sort_by_network() {
    let samples = vec![s("low", 99.0, 1, 0, 10), s("high", 1.0, 1, 0, 10_000)];
    let ranked = filter_and_rank(&samples, None, SortBy::Network, Some(1));
    assert_eq!(ranked[0].name, "high");
}

#[test]
fn sort_by_disk() {
    let samples = vec![s("a", 1.0, 1, 100, 0), s("b", 1.0, 1, 9999, 0)];
    let ranked = filter_and_rank(&samples, None, SortBy::Disk, None);
    assert_eq!(ranked[0].name, "b");
}
