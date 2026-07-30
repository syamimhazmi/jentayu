//! Integration tests against a mock Docker Engine HTTP API (wiremock).
//!
//! These always run in CI — no real daemon or socket required.
//! Covers list → capped stats fan-out → `ResourceSample` mapping.

use jentayu::collector::ContainerCollector;
use jentayu::{CollectMode, Collector, TargetKind};
use serde_json::json;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const NGINX_ID: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
const REDIS_ID: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const BAD_ID: &str = "000000000000000000000000000000000000000000000000000000000000dead";

fn list_body() -> String {
    json!([
        {
            "Id": NGINX_ID,
            "Names": ["/nginx"],
            "Status": "Up 2 minutes"
        },
        {
            "Id": REDIS_ID,
            "Names": ["/redis"],
            "Status": "Up 5 minutes"
        },
        {
            "Id": BAD_ID,
            "Names": ["/broken"],
            "Status": "Up 1 second"
        }
    ])
    .to_string()
}

/// One-shot stats payload with known CPU % and memory.
///
/// CPU: delta 1e8 / system 1e9 * online 2 * 100 = 20.0%
fn stats_body(id: &str, memory: u64) -> String {
    json!({
        "id": id,
        "name": "/ignored-by-list-name",
        "cpu_stats": {
            "cpu_usage": { "total_usage": 200_000_000u64 },
            "system_cpu_usage": 2_000_000_000u64,
            "online_cpus": 2
        },
        "precpu_stats": {
            "cpu_usage": { "total_usage": 100_000_000u64 },
            "system_cpu_usage": 1_000_000_000u64,
            "online_cpus": 2
        },
        "memory_stats": {
            "usage": memory,
            "limit": 512 * 1024 * 1024
        },
        "networks": {
            "eth0": {
                "rx_bytes": 1_000u64,
                "tx_bytes": 2_000u64
            }
        },
        "blkio_stats": {
            "io_service_bytes_recursive": [
                { "op": "Read", "value": 100 },
                { "op": "Write", "value": 50 }
            ]
        }
    })
    .to_string()
}

async fn mount_mock_daemon(server: &MockServer) {
    // Bollard's Uri::parse embeds `/v1.N` then `Url::join` with an absolute path,
    // so the request path seen by Hyper is unversioned (`/_ping`, `/containers/...`).
    Mock::given(method("GET"))
        .and(path_regex(r"^/_ping$"))
        .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/containers/json$"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(list_body()),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(format!(r"^/containers/{NGINX_ID}/stats$")))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(stats_body(NGINX_ID, 64 * 1024 * 1024)),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(format!(r"^/containers/{REDIS_ID}/stats$")))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(stats_body(REDIS_ID, 32 * 1024 * 1024)),
        )
        .mount(server)
        .await;

    // One container fails stats — sample should omit it, not abort.
    Mock::given(method("GET"))
        .and(path_regex(format!(r"^/containers/{BAD_ID}/stats$")))
        .respond_with(ResponseTemplate::new(500).set_body_string("stats unavailable"))
        .mount(server)
        .await;
}

#[tokio::test]
async fn mock_docker_connect_and_sample_containers() {
    let server = MockServer::start().await;
    mount_mock_daemon(&server).await;

    let addr = format!("http://{}", server.address());
    let collector = ContainerCollector::connect_http(&addr)
        .await
        .expect("mock Docker should ping");
    assert!(collector.is_available());

    let samples = collector.sample().await.expect("sample against mock");
    assert_eq!(
        samples.len(),
        2,
        "two healthy containers; broken stats skipped"
    );

    let nginx = samples
        .iter()
        .find(|s| s.name == "nginx")
        .expect("nginx from list Names");
    assert_eq!(nginx.kind, TargetKind::Container);
    assert_eq!(nginx.id, &NGINX_ID[..12]);
    assert_eq!(nginx.status.as_deref(), Some("Up 2 minutes"));
    assert!(
        (nginx.cpu_percent - 20.0).abs() < 0.01,
        "cpu={}",
        nginx.cpu_percent
    );
    assert_eq!(nginx.memory_bytes, 64 * 1024 * 1024);
    assert_eq!(nginx.memory_limit_bytes, Some(512 * 1024 * 1024));
    assert_eq!(nginx.net_rx_bytes, 1_000);
    assert_eq!(nginx.net_tx_bytes, 2_000);
    assert_eq!(nginx.disk_read_bytes, 100);
    assert_eq!(nginx.disk_write_bytes, 50);

    let redis = samples
        .iter()
        .find(|s| s.name == "redis")
        .expect("redis present");
    assert_eq!(redis.memory_bytes, 32 * 1024 * 1024);
    assert!(!samples.iter().any(|s| s.name == "broken"));
}

#[tokio::test]
async fn mock_docker_orchestrator_containers_only() {
    let server = MockServer::start().await;
    mount_mock_daemon(&server).await;

    let addr = format!("http://{}", server.address());
    let containers = ContainerCollector::connect_http(&addr)
        .await
        .expect("mock connect");
    let mut collector = Collector::with_containers(CollectMode::Containers, containers);
    assert!(collector.docker_available());

    let snap = collector.snapshot().await;
    assert!(
        snap.targets.iter().all(|t| t.kind == TargetKind::Container),
        "containers-only must not include processes"
    );
    assert_eq!(snap.targets.len(), 2);
    // Host sample is always present (sysinfo); just ensure snapshot builds.
    let _ = snap.host.cpu_percent;
    let _ = snap.host.memory_total_bytes;
}

#[tokio::test]
async fn disabled_docker_still_collects_processes() {
    // docker_optional.feature: daemon unavailable ⇒ process path still works.
    let mut collector =
        Collector::with_containers(CollectMode::All, ContainerCollector::disabled());
    assert!(!collector.docker_available());

    let snap = collector.snapshot().await;
    assert!(
        snap.targets.iter().all(|t| t.kind == TargetKind::Process),
        "with Docker disabled, only process targets expected"
    );
    // Hosts without processes would be unusual; allow empty but never panic.
    let _ = snap.targets.len();
}

#[tokio::test]
async fn containers_only_with_disabled_docker_is_empty() {
    let mut collector =
        Collector::with_containers(CollectMode::Containers, ContainerCollector::disabled());
    let snap = collector.snapshot().await;
    assert!(snap.targets.is_empty());
    assert!(!collector.docker_available());
}

#[tokio::test]
async fn connect_http_fails_when_ping_unreachable() {
    // Bind nothing — random high port that should refuse.
    match ContainerCollector::connect_http("http://127.0.0.1:1").await {
        Ok(_) => panic!("ping must fail against closed port"),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("Docker is unavailable") || msg.to_lowercase().contains("unavailable"),
                "error shape: {msg}"
            );
        }
    }
}
