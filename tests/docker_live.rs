//! Live Docker Engine integration (socket / daemon must be present).
//!
//! Gated two ways:
//! 1. Feature flag: compile with `--features docker-integration`
//! 2. Runtime: skip (not fail) when the daemon is unreachable
//!
//! Run: `cargo test --features docker-integration --test docker_live`

#![cfg(feature = "docker-integration")]

use jentayu::collector::ContainerCollector;
use jentayu::{CollectMode, Collector, TargetKind};

#[tokio::test]
async fn live_docker_connect_sample_when_socket_present() {
    let collector = match ContainerCollector::connect().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip live Docker test: daemon unavailable ({e})");
            return;
        }
    };
    assert!(collector.is_available());

    let samples = collector
        .sample()
        .await
        .expect("list+stats against live daemon must not hard-error");
    for s in &samples {
        assert_eq!(s.kind, TargetKind::Container);
        assert!(!s.id.is_empty());
        assert!(!s.name.is_empty());
    }
}

#[tokio::test]
async fn live_collector_all_mode_includes_processes() {
    let containers = match ContainerCollector::connect().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skip live Docker orchestrator test: daemon unavailable ({e})");
            return;
        }
    };

    let mut collector = Collector::with_containers(CollectMode::All, containers);
    let snap = collector.snapshot().await;

    let processes = snap
        .targets
        .iter()
        .filter(|t| t.kind == TargetKind::Process)
        .count();
    assert!(
        processes > 0,
        "All mode should include host processes even when Docker is up"
    );
}
