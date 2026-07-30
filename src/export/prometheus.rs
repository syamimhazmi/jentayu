//! Prometheus text exposition format renderer.

use crate::metrics::{Snapshot, TargetKind};

/// Render a snapshot as Prometheus exposition format (0.0.4 text).
pub fn render_prometheus(snapshot: &Snapshot) -> String {
    let mut out = String::with_capacity(4096);
    let ts_ms = snapshot.sampled_at.timestamp_millis();

    // Host metrics
    help_type(
        &mut out,
        "jentayu_host_cpu_percent",
        "gauge",
        "Host CPU usage percent",
    );
    metric_line(
        &mut out,
        "jentayu_host_cpu_percent",
        &[],
        snapshot.host.cpu_percent as f64,
        ts_ms,
    );

    help_type(
        &mut out,
        "jentayu_host_memory_used_bytes",
        "gauge",
        "Host used memory in bytes",
    );
    metric_line(
        &mut out,
        "jentayu_host_memory_used_bytes",
        &[],
        snapshot.host.memory_used_bytes as f64,
        ts_ms,
    );

    help_type(
        &mut out,
        "jentayu_host_memory_total_bytes",
        "gauge",
        "Host total memory in bytes",
    );
    metric_line(
        &mut out,
        "jentayu_host_memory_total_bytes",
        &[],
        snapshot.host.memory_total_bytes as f64,
        ts_ms,
    );

    help_type(
        &mut out,
        "jentayu_host_load1",
        "gauge",
        "Host load average (1m)",
    );
    metric_line(
        &mut out,
        "jentayu_host_load1",
        &[],
        snapshot.host.load_avg_1,
        ts_ms,
    );

    // Per-target metrics
    help_type(
        &mut out,
        "jentayu_cpu_percent",
        "gauge",
        "Target CPU usage percent",
    );
    help_type(
        &mut out,
        "jentayu_memory_bytes",
        "gauge",
        "Target memory usage in bytes",
    );
    help_type(
        &mut out,
        "jentayu_disk_read_bytes_total",
        "counter",
        "Target cumulative disk read bytes",
    );
    help_type(
        &mut out,
        "jentayu_disk_write_bytes_total",
        "counter",
        "Target cumulative disk write bytes",
    );
    help_type(
        &mut out,
        "jentayu_network_receive_bytes_total",
        "counter",
        "Target cumulative network receive bytes",
    );
    help_type(
        &mut out,
        "jentayu_network_transmit_bytes_total",
        "counter",
        "Target cumulative network transmit bytes",
    );
    help_type(
        &mut out,
        "jentayu_disk_read_bytes_per_second",
        "gauge",
        "Target disk read rate in bytes per second over the last interval",
    );
    help_type(
        &mut out,
        "jentayu_disk_write_bytes_per_second",
        "gauge",
        "Target disk write rate in bytes per second over the last interval",
    );
    help_type(
        &mut out,
        "jentayu_network_receive_bytes_per_second",
        "gauge",
        "Target network receive rate in bytes per second over the last interval",
    );
    help_type(
        &mut out,
        "jentayu_network_transmit_bytes_per_second",
        "gauge",
        "Target network transmit rate in bytes per second over the last interval",
    );
    help_type(
        &mut out,
        "jentayu_host_network_receive_bytes_per_second",
        "gauge",
        "Host network receive rate in bytes per second over the last interval",
    );
    help_type(
        &mut out,
        "jentayu_host_network_transmit_bytes_per_second",
        "gauge",
        "Host network transmit rate in bytes per second over the last interval",
    );

    if let Some(rx) = snapshot.host.net_rx_bps {
        metric_line(
            &mut out,
            "jentayu_host_network_receive_bytes_per_second",
            &[],
            rx,
            ts_ms,
        );
    }
    if let Some(tx) = snapshot.host.net_tx_bps {
        metric_line(
            &mut out,
            "jentayu_host_network_transmit_bytes_per_second",
            &[],
            tx,
            ts_ms,
        );
    }

    for t in &snapshot.targets {
        let kind = match t.kind {
            TargetKind::Process => "process",
            TargetKind::Container => "container",
            TargetKind::Host => "host",
        };
        let labels = [
            ("kind", kind),
            ("id", t.id.as_str()),
            ("name", t.name.as_str()),
        ];

        metric_line(
            &mut out,
            "jentayu_cpu_percent",
            &labels,
            t.cpu_percent as f64,
            ts_ms,
        );
        metric_line(
            &mut out,
            "jentayu_memory_bytes",
            &labels,
            t.memory_bytes as f64,
            ts_ms,
        );
        metric_line(
            &mut out,
            "jentayu_disk_read_bytes_total",
            &labels,
            t.disk_read_bytes as f64,
            ts_ms,
        );
        metric_line(
            &mut out,
            "jentayu_disk_write_bytes_total",
            &labels,
            t.disk_write_bytes as f64,
            ts_ms,
        );
        metric_line(
            &mut out,
            "jentayu_network_receive_bytes_total",
            &labels,
            t.net_rx_bytes as f64,
            ts_ms,
        );
        metric_line(
            &mut out,
            "jentayu_network_transmit_bytes_total",
            &labels,
            t.net_tx_bytes as f64,
            ts_ms,
        );
        if let Some(v) = t.disk_read_bps {
            metric_line(
                &mut out,
                "jentayu_disk_read_bytes_per_second",
                &labels,
                v,
                ts_ms,
            );
        }
        if let Some(v) = t.disk_write_bps {
            metric_line(
                &mut out,
                "jentayu_disk_write_bytes_per_second",
                &labels,
                v,
                ts_ms,
            );
        }
        if let Some(v) = t.net_rx_bps {
            metric_line(
                &mut out,
                "jentayu_network_receive_bytes_per_second",
                &labels,
                v,
                ts_ms,
            );
        }
        if let Some(v) = t.net_tx_bps {
            metric_line(
                &mut out,
                "jentayu_network_transmit_bytes_per_second",
                &labels,
                v,
                ts_ms,
            );
        }
    }

    out
}

fn help_type(out: &mut String, name: &str, ty: &str, help: &str) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push(' ');
    out.push_str(ty);
    out.push('\n');
}

fn metric_line(out: &mut String, name: &str, labels: &[(&str, &str)], value: f64, ts_ms: i64) {
    out.push_str(name);
    if !labels.is_empty() {
        out.push('{');
        for (i, (k, v)) in labels.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(k);
            out.push_str("=\"");
            out.push_str(&escape_label(v));
            out.push('"');
        }
        out.push('}');
    }
    out.push(' ');
    out.push_str(&format!("{value}"));
    out.push(' ');
    out.push_str(&ts_ms.to_string());
    out.push('\n');
}

fn escape_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{HostSample, ResourceSample, TargetKind};
    use chrono::Utc;

    #[test]
    fn renders_host_and_target_lines() {
        let snap = Snapshot {
            host: HostSample {
                cpu_percent: 12.5,
                memory_used_bytes: 1024,
                memory_total_bytes: 2048,
                swap_used_bytes: 0,
                swap_total_bytes: 0,
                disk_total_bytes: 100,
                disk_available_bytes: 50,
                net_rx_bytes: 1,
                net_tx_bytes: 2,
                net_rx_bps: Some(512.0),
                net_tx_bps: Some(256.0),
                load_avg_1: 0.5,
                load_avg_5: 0.4,
                load_avg_15: 0.3,
                sampled_at: Utc::now(),
            },
            targets: vec![ResourceSample {
                kind: TargetKind::Process,
                id: "42".into(),
                name: "nginx".into(),
                cpu_percent: 3.0,
                memory_bytes: 100,
                memory_limit_bytes: Some(2048),
                disk_read_bytes: 10,
                disk_write_bytes: 20,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                disk_read_bps: Some(100.0),
                disk_write_bps: Some(50.0),
                net_rx_bps: Some(0.0),
                net_tx_bps: Some(0.0),
                sampled_at: Utc::now(),
                status: None,
            }],
            sampled_at: Utc::now(),
        };

        let text = render_prometheus(&snap);
        assert!(text.contains("jentayu_host_cpu_percent"));
        assert!(text.contains("jentayu_cpu_percent{"));
        assert!(text.contains("name=\"nginx\""));
        // Full label set with commas only *between* keys (no leading comma).
        // Distinguishes `i > 0` from `i >= 0` (which would emit `{,kind=...}`).
        assert!(
            text.contains("{kind=\"process\",id=\"42\",name=\"nginx\"}"),
            "expected label block without leading comma"
        );
        assert!(text.contains("# TYPE jentayu_memory_bytes gauge"));
        assert!(text.contains("jentayu_disk_read_bytes_total"));
        assert!(text.contains("# TYPE jentayu_disk_read_bytes_per_second gauge"));
        assert!(text.contains("jentayu_disk_read_bytes_per_second{"));
        assert!(text.contains("jentayu_host_network_receive_bytes_per_second 512"));
        assert!(text.contains("jentayu_host_network_transmit_bytes_per_second 256"));
        assert!(text.contains("jentayu_disk_write_bytes_per_second{"));
        assert!(text.contains("jentayu_network_receive_bytes_per_second{"));
        assert!(text.contains("jentayu_network_transmit_bytes_per_second{"));
        // Values and host load appear on sample lines.
        assert!(text
            .lines()
            .any(|l| l.starts_with("jentayu_host_cpu_percent 12.5 ")));
        assert!(text
            .lines()
            .any(|l| l.starts_with("jentayu_host_load1 0.5 ")));
        // Mutation-minded: rate gauges must stay jentayu_ prefixed.
        for line in text.lines() {
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

    #[test]
    fn escapes_label_special_characters() {
        let snap = Snapshot {
            host: HostSample {
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
            },
            targets: vec![ResourceSample {
                kind: TargetKind::Container,
                id: "abc".into(),
                name: "a\"b\\c\nd".into(),
                cpu_percent: 1.0,
                memory_bytes: 1,
                memory_limit_bytes: None,
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
            }],
            sampled_at: Utc::now(),
        };
        let text = render_prometheus(&snap);
        assert!(
            text.contains("name=\"a\\\"b\\\\c\\nd\""),
            "label escape broken: {text}"
        );
        assert!(text.contains("kind=\"container\""));
    }

    #[test]
    fn omits_rate_gauges_when_unknown() {
        let snap = Snapshot {
            host: HostSample {
                cpu_percent: 1.0,
                memory_used_bytes: 1,
                memory_total_bytes: 2,
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
            },
            targets: vec![ResourceSample {
                kind: TargetKind::Process,
                id: "1".into(),
                name: "x".into(),
                cpu_percent: 0.0,
                memory_bytes: 0,
                memory_limit_bytes: None,
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
            }],
            sampled_at: Utc::now(),
        };
        let text = render_prometheus(&snap);
        // HELP/TYPE still declared; sample lines only when rates known.
        assert!(!text.contains("jentayu_disk_read_bytes_per_second{"));
        assert!(
            !text
                .lines()
                .any(|l| l.starts_with("jentayu_host_network_receive_bytes_per_second ")),
            "must not emit host rate samples without known rates"
        );
    }
}
