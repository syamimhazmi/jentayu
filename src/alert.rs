//! Threshold-based alerts on collected samples.

use crate::metrics::{ResourceSample, Snapshot};
use serde::{Deserialize, Serialize};

/// Threshold configuration (all optional; None = disabled).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Alert when CPU percent exceeds this value.
    pub cpu_percent: Option<f32>,
    /// Alert when memory percent (of limit) exceeds this value.
    pub memory_percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub target_id: String,
    pub target_name: String,
    pub kind: String,
    pub message: String,
    pub value: f32,
    pub threshold: f32,
}

/// Evaluate a snapshot against thresholds.
pub fn evaluate(snapshot: &Snapshot, thresholds: &AlertThresholds) -> Vec<Alert> {
    let mut alerts = Vec::new();

    if let Some(cpu_th) = thresholds.cpu_percent {
        if snapshot.host.cpu_percent >= cpu_th {
            alerts.push(Alert {
                target_id: "host".into(),
                target_name: "host".into(),
                kind: "cpu".into(),
                message: format!(
                    "host CPU {:.1}% >= threshold {:.1}%",
                    snapshot.host.cpu_percent, cpu_th
                ),
                value: snapshot.host.cpu_percent,
                threshold: cpu_th,
            });
        }
    }

    if let Some(mem_th) = thresholds.memory_percent {
        let pct = snapshot.host.memory_percent();
        if pct >= mem_th {
            alerts.push(Alert {
                target_id: "host".into(),
                target_name: "host".into(),
                kind: "memory".into(),
                message: format!("host memory {pct:.1}% >= threshold {mem_th:.1}%"),
                value: pct,
                threshold: mem_th,
            });
        }
    }

    for t in &snapshot.targets {
        evaluate_target(t, thresholds, &mut alerts);
    }

    alerts
}

fn evaluate_target(t: &ResourceSample, thresholds: &AlertThresholds, alerts: &mut Vec<Alert>) {
    if let Some(cpu_th) = thresholds.cpu_percent {
        if t.cpu_percent >= cpu_th {
            alerts.push(Alert {
                target_id: t.id.clone(),
                target_name: t.name.clone(),
                kind: "cpu".into(),
                message: format!(
                    "{} CPU {:.1}% >= threshold {:.1}%",
                    t.name, t.cpu_percent, cpu_th
                ),
                value: t.cpu_percent,
                threshold: cpu_th,
            });
        }
    }

    if let Some(mem_th) = thresholds.memory_percent {
        if let Some(pct) = t.memory_percent() {
            if pct >= mem_th {
                alerts.push(Alert {
                    target_id: t.id.clone(),
                    target_name: t.name.clone(),
                    kind: "memory".into(),
                    message: format!("{} memory {pct:.1}% >= threshold {mem_th:.1}%", t.name),
                    value: pct,
                    threshold: mem_th,
                });
            }
        }
    }
}

pub fn print_alerts(alerts: &[Alert]) {
    if alerts.is_empty() {
        return;
    }
    eprintln!("── alerts ({}) ──", alerts.len());
    for a in alerts {
        eprintln!("  ⚠  [{}] {}", a.kind, a.message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{HostSample, TargetKind};
    use chrono::Utc;

    fn empty_host() -> HostSample {
        HostSample {
            cpu_percent: 90.0,
            memory_used_bytes: 900,
            memory_total_bytes: 1000,
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

    #[test]
    fn fires_host_cpu_alert() {
        let snap = Snapshot {
            host: empty_host(),
            targets: vec![],
            sampled_at: Utc::now(),
        };
        let th = AlertThresholds {
            cpu_percent: Some(80.0),
            memory_percent: None,
        };
        let alerts = evaluate(&snap, &th);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, "cpu");
        assert_eq!(alerts[0].target_id, "host");
        assert!((alerts[0].value - 90.0).abs() < f32::EPSILON);
        assert!((alerts[0].threshold - 80.0).abs() < f32::EPSILON);
        assert!(alerts[0].message.contains("CPU"));
    }

    #[test]
    fn fires_host_memory_at_boundary_equality() {
        // empty_host is 900/1000 = 90%; `>=` must fire at exact threshold.
        let snap = Snapshot {
            host: empty_host(),
            targets: vec![],
            sampled_at: Utc::now(),
        };
        let th = AlertThresholds {
            cpu_percent: None,
            memory_percent: Some(90.0),
        };
        let alerts = evaluate(&snap, &th);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, "memory");
        assert!((alerts[0].value - 90.0).abs() < 0.01);
    }

    #[test]
    fn fires_target_cpu_and_memory_alerts() {
        let snap = Snapshot {
            host: {
                let mut h = empty_host();
                h.cpu_percent = 1.0;
                h.memory_used_bytes = 1;
                h.memory_total_bytes = 1000;
                h
            },
            targets: vec![ResourceSample {
                kind: TargetKind::Process,
                id: "42".into(),
                name: "hot".into(),
                cpu_percent: 95.0,
                memory_bytes: 800,
                memory_limit_bytes: Some(1000),
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
        let th = AlertThresholds {
            cpu_percent: Some(90.0),
            memory_percent: Some(75.0),
        };
        let alerts = evaluate(&snap, &th);
        assert_eq!(alerts.len(), 2);
        assert!(alerts
            .iter()
            .any(|a| a.kind == "cpu" && a.target_id == "42"));
        assert!(alerts
            .iter()
            .any(|a| a.kind == "memory" && a.target_name == "hot"));
    }

    #[test]
    fn target_memory_skipped_without_limit() {
        let snap = Snapshot {
            host: {
                let mut h = empty_host();
                h.cpu_percent = 0.0;
                h.memory_used_bytes = 0;
                h
            },
            targets: vec![ResourceSample {
                kind: TargetKind::Process,
                id: "1".into(),
                name: "nolimit".into(),
                cpu_percent: 0.0,
                memory_bytes: 999_999,
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
        let th = AlertThresholds {
            cpu_percent: None,
            memory_percent: Some(1.0),
        };
        assert!(evaluate(&snap, &th).is_empty());
    }

    #[test]
    fn no_alert_under_threshold() {
        let mut host = empty_host();
        host.cpu_percent = 10.0;
        let snap = Snapshot {
            host,
            targets: vec![ResourceSample {
                kind: TargetKind::Process,
                id: "1".into(),
                name: "idle".into(),
                cpu_percent: 1.0,
                memory_bytes: 10,
                memory_limit_bytes: Some(1000),
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
        let th = AlertThresholds {
            cpu_percent: Some(80.0),
            memory_percent: Some(95.0),
        };
        assert!(evaluate(&snap, &th).is_empty());
    }

    #[test]
    fn disabled_thresholds_produce_no_alerts() {
        let snap = Snapshot {
            host: empty_host(),
            targets: vec![],
            sampled_at: Utc::now(),
        };
        let th = AlertThresholds::default();
        assert!(evaluate(&snap, &th).is_empty());
    }
}
