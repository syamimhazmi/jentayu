//! Executable Gherkin for high-value pure logic under `tests/features/`.
//!
//! Wired features: filter_and_rank, alerts, prometheus_export.
//! Other `.feature` files stay living specs (see README map) until steps land;
//! they are excluded via the `@wip` tag filter.

use chrono::Utc;
use cucumber::{given, then, when, World};
use jentayu::alert::{evaluate, Alert, AlertThresholds};
use jentayu::export::render_prometheus;
use jentayu::metrics::{filter_and_rank, HostSample, ResourceSample, Snapshot, SortBy, TargetKind};

// ── World ───────────────────────────────────────────────────────────────────

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct JentayuWorld {
    samples: Vec<ResourceSample>,
    ranked: Vec<ResourceSample>,
    sort_by: SortBy,
    name_filter: Option<String>,
    top: Option<usize>,

    thresholds: AlertThresholds,
    /// Target samples used when evaluating alerts (host kept quiet by default).
    alert_targets: Vec<ResourceSample>,
    host: HostSample,
    alerts: Vec<Alert>,

    snapshot: Option<Snapshot>,
    prometheus_text: String,
}

impl JentayuWorld {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
            ranked: Vec::new(),
            sort_by: SortBy::Cpu,
            name_filter: None,
            top: None,
            thresholds: AlertThresholds::default(),
            alert_targets: Vec::new(),
            host: default_host(),
            alerts: Vec::new(),
            snapshot: None,
            prometheus_text: String::new(),
        }
    }
}

fn default_host() -> HostSample {
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
        sampled_at: Utc::now(),
    }
}

fn process(name: &str, cpu: f32, mem: u64, disk: u64, net: u64) -> ResourceSample {
    ResourceSample {
        kind: TargetKind::Process,
        id: name.into(),
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

fn target(
    kind: TargetKind,
    id: &str,
    name: &str,
    cpu: f32,
    mem: u64,
    mem_limit: Option<u64>,
) -> ResourceSample {
    let mut s = process(name, cpu, mem, 0, 0);
    s.kind = kind;
    s.id = id.into();
    s.memory_limit_bytes = mem_limit;
    s
}

// ── filter_and_rank ─────────────────────────────────────────────────────────

#[given("samples with different CPU percentages")]
fn given_cpu_samples(w: &mut JentayuWorld) {
    w.samples = vec![
        process("low", 10.0, 1, 0, 0),
        process("mid", 50.0, 1, 0, 0),
        process("high", 90.0, 1, 0, 0),
    ];
}

#[given("samples with different memory footprints")]
fn given_memory_samples(w: &mut JentayuWorld) {
    w.samples = vec![
        process("small", 1.0, 100, 0, 0),
        process("large", 1.0, 9_000, 0, 0),
        process("medium", 1.0, 1_000, 0, 0),
    ];
}

#[given("samples with different disk read totals")]
fn given_disk_samples(w: &mut JentayuWorld) {
    w.samples = vec![
        process("a", 1.0, 1, 100, 0),
        process("b", 1.0, 1, 9_999, 0),
        process("c", 1.0, 1, 500, 0),
    ];
}

#[given("samples with different network totals")]
fn given_network_samples(w: &mut JentayuWorld) {
    w.samples = vec![
        process("low", 99.0, 1, 0, 10),
        process("high", 1.0, 1, 0, 10_000),
    ];
}

#[given(regex = r#"^samples named "([^"]+)", "([^"]+)", and "([^"]+)"$"#)]
fn given_named_samples(w: &mut JentayuWorld, a: String, b: String, c: String) {
    w.samples = vec![
        process(&a, 1.0, 1, 0, 0),
        process(&b, 1.0, 1, 0, 0),
        process(&c, 1.0, 1, 0, 0),
    ];
}

#[given("more than N samples")]
fn given_more_than_n(w: &mut JentayuWorld) {
    w.samples = vec![
        process("a", 10.0, 1, 0, 0),
        process("b", 50.0, 1, 0, 0),
        process("c", 90.0, 1, 0, 0),
    ];
    w.sort_by = SortBy::Cpu;
}

#[given("no samples")]
fn given_no_samples(w: &mut JentayuWorld) {
    w.samples.clear();
}

#[when("I rank by cpu with no filter and no top limit")]
fn when_rank_cpu_full(w: &mut JentayuWorld) {
    w.sort_by = SortBy::Cpu;
    w.name_filter = None;
    w.top = None;
    w.ranked = filter_and_rank(&w.samples, None, SortBy::Cpu, None);
}

#[when("I rank by memory")]
fn when_rank_memory(w: &mut JentayuWorld) {
    w.sort_by = SortBy::Memory;
    w.ranked = filter_and_rank(&w.samples, None, SortBy::Memory, None);
}

#[when("I rank by disk")]
fn when_rank_disk(w: &mut JentayuWorld) {
    w.sort_by = SortBy::Disk;
    w.ranked = filter_and_rank(&w.samples, None, SortBy::Disk, None);
}

#[when("I rank by network")]
fn when_rank_network(w: &mut JentayuWorld) {
    w.sort_by = SortBy::Network;
    w.ranked = filter_and_rank(&w.samples, None, SortBy::Network, None);
}

#[when(regex = r#"^I filter by "([^"]+)"$"#)]
fn when_filter(w: &mut JentayuWorld, filter: String) {
    w.name_filter = Some(filter.clone());
    w.ranked = filter_and_rank(&w.samples, Some(&filter), SortBy::Cpu, None);
}

#[when("I rank with top N equal to 1")]
fn when_top_one(w: &mut JentayuWorld) {
    w.top = Some(1);
    w.ranked = filter_and_rank(&w.samples, None, w.sort_by, Some(1));
}

#[when("I rank with any sort and filter")]
fn when_rank_any(w: &mut JentayuWorld) {
    w.ranked = filter_and_rank(&w.samples, w.name_filter.as_deref(), SortBy::Cpu, None);
}

#[then("samples are ordered by CPU percent descending")]
fn then_cpu_desc(w: &mut JentayuWorld) {
    assert!(!w.ranked.is_empty());
    for window in w.ranked.windows(2) {
        assert!(
            window[0].cpu_percent >= window[1].cpu_percent,
            "expected descending CPU: {} then {}",
            window[0].cpu_percent,
            window[1].cpu_percent
        );
    }
}

#[then("the highest memory sample is first")]
fn then_mem_first(w: &mut JentayuWorld) {
    let max = w
        .samples
        .iter()
        .map(|s| s.memory_bytes)
        .max()
        .expect("samples");
    assert_eq!(w.ranked[0].memory_bytes, max);
}

#[then("the highest disk sample is first")]
fn then_disk_first(w: &mut JentayuWorld) {
    let max = w
        .samples
        .iter()
        .map(|s| s.disk_read_bytes)
        .max()
        .expect("samples");
    assert_eq!(w.ranked[0].disk_read_bytes, max);
}

#[then("the highest network sample is first")]
fn then_net_first(w: &mut JentayuWorld) {
    let max = w
        .samples
        .iter()
        .map(|s| s.net_rx_bytes + s.net_tx_bytes)
        .max()
        .expect("samples");
    let first = w.ranked[0].net_rx_bytes + w.ranked[0].net_tx_bytes;
    assert_eq!(first, max);
    assert_eq!(w.ranked[0].name, "high");
}

#[then("only the Postgres sample remains")]
fn then_only_postgres(w: &mut JentayuWorld) {
    assert_eq!(w.ranked.len(), 1);
    assert_eq!(w.ranked[0].name, "Postgres");
}

#[then("exactly one sample is returned")]
fn then_one_sample(w: &mut JentayuWorld) {
    assert_eq!(w.ranked.len(), 1);
}

#[then("it is the highest ranked by the chosen metric")]
fn then_highest_by_metric(w: &mut JentayuWorld) {
    assert_eq!(w.ranked.len(), 1);
    let full = filter_and_rank(&w.samples, None, w.sort_by, None);
    assert_eq!(w.ranked[0].name, full[0].name);
    assert_eq!(w.ranked[0].id, full[0].id);
}

#[then("the result is empty")]
fn then_empty(w: &mut JentayuWorld) {
    assert!(w.ranked.is_empty());
}

// ── alerts ──────────────────────────────────────────────────────────────────

#[given(regex = r"^an alert CPU threshold of ([\d.]+) percent$")]
fn given_cpu_threshold(w: &mut JentayuWorld, thr: f32) {
    w.thresholds.cpu_percent = Some(thr);
    if w.host.memory_total_bytes == 0 {
        w.host = default_host();
    }
}

#[given(regex = r"^an alert memory threshold of ([\d.]+) percent$")]
fn given_mem_threshold(w: &mut JentayuWorld, thr: f32) {
    w.thresholds.memory_percent = Some(thr);
    if w.host.memory_total_bytes == 0 {
        w.host = default_host();
    }
}

#[given(regex = r"^a sample at exactly ([\d.]+) percent CPU$")]
fn given_sample_cpu_exact(w: &mut JentayuWorld, cpu: f32) {
    w.alert_targets = vec![target(
        TargetKind::Process,
        "1",
        "busy",
        cpu,
        10,
        Some(1000),
    )];
    // Keep host under any CPU threshold so only the target fires.
    w.host = default_host();
}

#[given(regex = r"^a sample at ([\d.]+) percent CPU$")]
fn given_sample_cpu(w: &mut JentayuWorld, cpu: f32) {
    given_sample_cpu_exact(w, cpu);
}

#[given(regex = r"^a sample using ([\d.]+) percent of its memory limit$")]
fn given_sample_mem_pct(w: &mut JentayuWorld, pct: f32) {
    let limit = 1000u64;
    let used = ((pct as f64 / 100.0) * limit as f64) as u64;
    w.alert_targets = vec![target(
        TargetKind::Process,
        "2",
        "hungry",
        0.0,
        used,
        Some(limit),
    )];
    w.host = default_host();
}

#[given("no CPU or memory alert thresholds")]
fn given_no_thresholds(w: &mut JentayuWorld) {
    w.thresholds = AlertThresholds::default();
    w.alert_targets = vec![process("any", 99.0, 999, 0, 0)];
    w.host = default_host();
    w.host.cpu_percent = 99.0;
}

#[when("alerts are evaluated")]
fn when_evaluate_alerts(w: &mut JentayuWorld) {
    let snap = Snapshot {
        host: w.host.clone(),
        targets: w.alert_targets.clone(),
        sampled_at: Utc::now(),
    };
    w.alerts = evaluate(&snap, &w.thresholds);
}

#[when("alerts are evaluated for any samples")]
fn when_evaluate_any(w: &mut JentayuWorld) {
    when_evaluate_alerts(w);
}

#[then("a CPU alert is raised for that sample")]
fn then_cpu_alert(w: &mut JentayuWorld) {
    assert!(
        w.alerts
            .iter()
            .any(|a| a.kind == "cpu" && a.target_id != "host"),
        "expected target CPU alert, got {:?}",
        w.alerts
    );
}

#[then("no CPU alert is raised for that sample")]
fn then_no_cpu_alert(w: &mut JentayuWorld) {
    assert!(
        !w.alerts
            .iter()
            .any(|a| a.kind == "cpu" && a.target_id != "host"),
        "unexpected CPU alert: {:?}",
        w.alerts
    );
}

#[then("a memory alert is raised for that sample")]
fn then_mem_alert(w: &mut JentayuWorld) {
    assert!(
        w.alerts
            .iter()
            .any(|a| a.kind == "memory" && a.target_id != "host"),
        "expected target memory alert, got {:?}",
        w.alerts
    );
}

#[then("no alerts are raised")]
fn then_no_alerts(w: &mut JentayuWorld) {
    assert!(w.alerts.is_empty(), "unexpected alerts: {:?}", w.alerts);
}

// ── prometheus_export ───────────────────────────────────────────────────────

fn host_with_metrics() -> HostSample {
    HostSample {
        cpu_percent: 12.5,
        memory_used_bytes: 1024,
        memory_total_bytes: 2048,
        swap_used_bytes: 0,
        swap_total_bytes: 0,
        disk_total_bytes: 100,
        disk_available_bytes: 50,
        net_rx_bytes: 1,
        net_tx_bytes: 2,
        net_rx_bps: None,
        net_tx_bps: None,
        load_avg_1: 0.5,
        load_avg_5: 0.4,
        load_avg_15: 0.3,
        sampled_at: Utc::now(),
    }
}

#[given("a non-empty snapshot")]
fn given_nonempty_snapshot(w: &mut JentayuWorld) {
    w.snapshot = Some(Snapshot {
        host: host_with_metrics(),
        targets: vec![process("nginx", 3.0, 100, 10, 0)],
        sampled_at: Utc::now(),
    });
}

#[given("process and container samples")]
fn given_process_and_container(w: &mut JentayuWorld) {
    let mut proc = target(TargetKind::Process, "42", "nginx", 3.0, 100, Some(2048));
    proc.disk_read_bytes = 10;
    let mut ctr = target(TargetKind::Container, "abc", "api", 5.0, 200, Some(4096));
    ctr.disk_read_bytes = 20;
    ctr.net_rx_bytes = 5;
    w.snapshot = Some(Snapshot {
        host: host_with_metrics(),
        targets: vec![proc, ctr],
        sampled_at: Utc::now(),
    });
}

#[given("a snapshot with host metrics")]
fn given_host_metrics_snapshot(w: &mut JentayuWorld) {
    w.snapshot = Some(Snapshot {
        host: host_with_metrics(),
        targets: vec![],
        sampled_at: Utc::now(),
    });
}

#[given("a snapshot with interval disk and network rates")]
fn given_rates_snapshot(w: &mut JentayuWorld) {
    let mut host = host_with_metrics();
    host.net_rx_bps = Some(512.0);
    host.net_tx_bps = Some(256.0);
    let mut t = process("nginx", 3.0, 100, 10, 20);
    t.disk_read_bps = Some(100.0);
    t.disk_write_bps = Some(50.0);
    t.net_rx_bps = Some(8.0);
    t.net_tx_bps = Some(4.0);
    w.snapshot = Some(Snapshot {
        host,
        targets: vec![t],
        sampled_at: Utc::now(),
    });
}

#[when("the snapshot is rendered as Prometheus text")]
fn when_render_prometheus(w: &mut JentayuWorld) {
    let snap = w
        .snapshot
        .as_ref()
        .expect("snapshot must be given before render");
    w.prometheus_text = render_prometheus(snap);
}

#[then("every metric name starts with jentayu_")]
fn then_jentayu_prefix(w: &mut JentayuWorld) {
    for line in w.prometheus_text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let name = line
            .split('{')
            .next()
            .unwrap_or(line)
            .split(' ')
            .next()
            .unwrap();
        assert!(
            name.starts_with("jentayu_"),
            "metric {name} missing jentayu_ prefix"
        );
    }
}

#[then("series include identifying labels for id or name and kind")]
fn then_identifying_labels(w: &mut JentayuWorld) {
    assert!(
        w.prometheus_text.contains("kind=\"process\""),
        "missing process kind label"
    );
    assert!(
        w.prometheus_text.contains("kind=\"container\""),
        "missing container kind label"
    );
    assert!(
        w.prometheus_text.contains("name=\"nginx\"") || w.prometheus_text.contains("id=\"42\""),
        "missing process id/name labels"
    );
    assert!(
        w.prometheus_text.contains("name=\"api\"") || w.prometheus_text.contains("id=\"abc\""),
        "missing container id/name labels"
    );
}

#[then("host-level CPU and memory metrics are present")]
fn then_host_metrics(w: &mut JentayuWorld) {
    assert!(w.prometheus_text.contains("jentayu_host_cpu_percent"));
    assert!(w.prometheus_text.contains("jentayu_host_memory_used_bytes"));
    assert!(w
        .prometheus_text
        .contains("jentayu_host_memory_total_bytes"));
}

#[then("jentayu_*_bytes_total counters remain present")]
fn then_bytes_total(w: &mut JentayuWorld) {
    assert!(w.prometheus_text.contains("jentayu_disk_read_bytes_total"));
    assert!(w.prometheus_text.contains("jentayu_disk_write_bytes_total"));
    assert!(w
        .prometheus_text
        .contains("jentayu_network_receive_bytes_total"));
    assert!(w
        .prometheus_text
        .contains("jentayu_network_transmit_bytes_total"));
}

#[then("jentayu_*_bytes_per_second gauges are present for known rates")]
fn then_bytes_per_second(w: &mut JentayuWorld) {
    assert!(w
        .prometheus_text
        .contains("jentayu_disk_read_bytes_per_second{"));
    assert!(w
        .prometheus_text
        .contains("jentayu_disk_write_bytes_per_second{"));
    assert!(w
        .prometheus_text
        .contains("jentayu_network_receive_bytes_per_second{"));
    assert!(w
        .prometheus_text
        .contains("jentayu_network_transmit_bytes_per_second{"));
    assert!(w
        .prometheus_text
        .lines()
        .any(|l| l.starts_with("jentayu_host_network_receive_bytes_per_second ")));
}

// ── runner ──────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Only pure high-value features are wired. Specs tagged `@wip` remain
    // documentation + mapped Rust tests until step definitions land.
    JentayuWorld::cucumber()
        .fail_on_skipped()
        .filter_run_and_exit("tests/features", |feature, _rule, scenario| {
            let tagged_wip = |tags: &[String]| tags.iter().any(|t| t == "wip");
            !tagged_wip(&feature.tags) && !tagged_wip(&scenario.tags)
        })
        .await;
}
