//! Metric exporters (Prometheus text format, JSON).

mod prometheus;

pub use prometheus::render_prometheus;

use crate::metrics::Snapshot;
use anyhow::Result;
use std::path::Path;

/// Write snapshot as pretty JSON.
pub fn write_json(snapshot: &Snapshot, path: Option<&Path>) -> Result<String> {
    let body = serde_json::to_string_pretty(snapshot)?;
    if let Some(p) = path {
        write_atomic(p, &body)?;
    }
    Ok(body)
}

/// Write Prometheus text exposition format.
pub fn write_prometheus(snapshot: &Snapshot, path: Option<&Path>) -> Result<String> {
    let body = render_prometheus(snapshot);
    if let Some(p) = path {
        write_atomic(p, &body)?;
    }
    Ok(body)
}

/// Write `body` to `path` via a same-directory temp file + `rename`.
///
/// Atomic on the same filesystem. Temp name includes pid + nanoseconds so
/// concurrent writers do not collide on pid alone.
fn write_atomic(path: &Path, body: &str) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let base = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("jentayu-out");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(".{base}.{}.{nanos}.tmp", std::process::id()));

    if let Err(e) = std::fs::write(&tmp, body) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{snapshot_for_export, HostSample, ResourceSample, SortBy, TargetKind};
    use chrono::Utc;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn host() -> HostSample {
        HostSample {
            cpu_percent: 1.0,
            memory_used_bytes: 100,
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

    fn sample(id: &str, name: &str, cpu: f32) -> ResourceSample {
        ResourceSample {
            kind: TargetKind::Process,
            id: id.into(),
            name: name.into(),
            cpu_percent: cpu,
            memory_bytes: 100,
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
        }
    }

    fn unique_temp_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "jentayu-export-test-{}-{}",
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn write_atomic_creates_file_with_content() {
        let dir = unique_temp_dir();
        let path = dir.join("out.prom");
        write_atomic(&path, "hello-metrics\n").expect("write");
        let body = fs::read_to_string(&path).expect("read");
        assert_eq!(body, "hello-metrics\n");
        // Temp file must be gone after success.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files left: {leftovers:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_overwrites_existing() {
        let dir = unique_temp_dir();
        let path = dir.join("out.json");
        fs::write(&path, "old").expect("seed");
        write_atomic(&path, "new-content").expect("overwrite");
        assert_eq!(fs::read_to_string(&path).unwrap(), "new-content");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_cleanup_on_rename_failure() {
        // Rename to a non-existent nested path under a file (not a dir) fails;
        // ensure we do not leave a temp in the parent of that broken path's parent.
        // Simpler: write_atomic to a path whose parent is a regular file.
        let dir = unique_temp_dir();
        let blocker = dir.join("not-a-dir");
        fs::write(&blocker, "x").expect("blocker");
        let bad = blocker.join("child.prom");
        let err = write_atomic(&bad, "body");
        assert!(err.is_err());
        // No .tmp under dir after failure cleanup attempt.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp left after failure: {leftovers:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_prometheus_and_json_use_atomic_path() {
        let dir = unique_temp_dir();
        let snap = Snapshot {
            host: host(),
            targets: vec![sample("1", "nginx", 3.0)],
            sampled_at: Utc::now(),
        };
        let prom_path = dir.join("m.prom");
        let json_path = dir.join("m.json");
        let prom = write_prometheus(&snap, Some(&prom_path)).expect("prom");
        let json = write_json(&snap, Some(&json_path)).expect("json");
        assert_eq!(fs::read_to_string(&prom_path).unwrap(), prom);
        assert_eq!(fs::read_to_string(&json_path).unwrap(), json);
        assert!(prom.contains("jentayu_"));
        assert!(json.contains("nginx"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_export_respects_snapshot_for_export_filter_top() {
        let at = Utc::now();
        let snap = Snapshot {
            host: host(),
            targets: vec![
                sample("1", "low", 10.0),
                sample("2", "nginx", 40.0),
                sample("3", "nginx-worker", 90.0),
            ],
            sampled_at: at,
        };
        let export = snapshot_for_export(&snap, Some("nginx"), SortBy::Cpu, Some(1));
        let body = write_json(&export, None).expect("json");
        let v: serde_json::Value = serde_json::from_str(&body).expect("parse");
        let targets = v["targets"].as_array().expect("targets array");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0]["name"], "nginx-worker");
        // Host always present in JSON.
        assert!(v.get("host").is_some());
    }

    #[test]
    fn json_export_full_inventory_when_unset() {
        let at = Utc::now();
        let snap = Snapshot {
            host: host(),
            targets: vec![
                sample("1", "a", 10.0),
                sample("2", "b", 20.0),
                sample("3", "c", 30.0),
            ],
            sampled_at: at,
        };
        let export = snapshot_for_export(&snap, None, SortBy::Cpu, None);
        let body = write_json(&export, None).expect("json");
        let v: serde_json::Value = serde_json::from_str(&body).expect("parse");
        assert_eq!(v["targets"].as_array().unwrap().len(), 3);
    }
}
