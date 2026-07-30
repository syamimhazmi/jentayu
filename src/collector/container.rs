//! Docker container metrics via the Bollard API.

use crate::error::{JentayuError, Result};
use crate::metrics::{ResourceSample, TargetKind};
use bollard::models::{ContainerStatsResponse, ContainerSummary};
use bollard::query_parameters::{ListContainersOptions, StatsOptions};
use bollard::Docker;
use chrono::{DateTime, Utc};
use futures_util::stream::{self, StreamExt};
use std::default::Default;
use std::future::Future;
use tracing::debug;

/// Maximum concurrent Docker `stats` API calls per sample tick (M4 fan-out cap).
pub const MAX_CONCURRENT_STATS: usize = 8;

/// One container to query for stats (id + display metadata from list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContainerJob {
    pub id: String,
    pub name: String,
    pub status: Option<String>,
}

pub struct ContainerCollector {
    docker: Option<Docker>,
}

impl ContainerCollector {
    pub async fn connect() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| JentayuError::DockerUnavailable(e.to_string()))?;
        Self::from_client_pinged(docker).await
    }

    /// Connect to a Docker Engine HTTP endpoint (e.g. `http://127.0.0.1:2375`).
    ///
    /// Used by integration tests against a mock daemon, and for TCP Docker hosts.
    pub async fn connect_http(addr: &str) -> Result<Self> {
        let docker = Docker::connect_with_http(addr, 10, bollard::API_DEFAULT_VERSION)
            .map_err(|e| JentayuError::DockerUnavailable(e.to_string()))?;
        Self::from_client_pinged(docker).await
    }

    /// Wrap an already-connected Bollard client (no ping). Prefer [`connect`] /
    /// [`connect_http`] for production paths that should verify the daemon.
    pub fn from_client(docker: Docker) -> Self {
        Self {
            docker: Some(docker),
        }
    }

    async fn from_client_pinged(docker: Docker) -> Result<Self> {
        docker
            .ping()
            .await
            .map_err(|e| JentayuError::DockerUnavailable(e.to_string()))?;

        Ok(Self {
            docker: Some(docker),
        })
    }

    pub fn disabled() -> Self {
        Self { docker: None }
    }

    pub fn is_available(&self) -> bool {
        self.docker.is_some()
    }

    pub async fn sample(&self) -> Result<Vec<ResourceSample>> {
        let docker = match &self.docker {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let options = ListContainersOptions {
            all: false,
            ..Default::default()
        };

        let containers: Vec<ContainerSummary> = docker
            .list_containers(Some(options))
            .await
            .map_err(|e| JentayuError::ContainerCollect(e.to_string()))?;

        let now = Utc::now();
        let docker = docker.clone();
        let jobs = jobs_from_summaries(containers);

        let samples = collect_stats_capped(jobs, MAX_CONCURRENT_STATS, |job| {
            let docker = docker.clone();
            async move {
                match Self::stats_for(&docker, &job.id).await {
                    Ok(mut sample) => {
                        sample.name = job.name;
                        sample.status = job.status;
                        sample.sampled_at = now;
                        Some(sample)
                    }
                    Err(e) => {
                        debug!(container = %job.id, error = %e, "skipping container stats");
                        None
                    }
                }
            }
        })
        .await;

        Ok(samples)
    }

    async fn stats_for(docker: &Docker, id: &str) -> Result<ResourceSample> {
        let options = StatsOptions {
            stream: false,
            one_shot: true,
        };

        let mut stream = docker.stats(id, Some(options));
        let stats = stream
            .next()
            .await
            .ok_or_else(|| JentayuError::ContainerCollect("empty stats stream".into()))?
            .map_err(|e| JentayuError::ContainerCollect(e.to_string()))?;

        Ok(sample_from_stats(id, &stats, Utc::now()))
    }
}

/// Build stats jobs from Docker list responses (skips entries without an id).
pub(crate) fn jobs_from_summaries(
    containers: impl IntoIterator<Item = ContainerSummary>,
) -> Vec<ContainerJob> {
    containers
        .into_iter()
        .filter_map(|c| job_from_list_fields(c.id, c.names.as_deref(), c.status))
        .collect()
}

/// Pure mapping from list fields → job (testable without Docker).
pub(crate) fn job_from_list_fields(
    id: Option<String>,
    names: Option<&[String]>,
    status: Option<String>,
) -> Option<ContainerJob> {
    let id = id?;
    let name = names
        .and_then(|ns| ns.first())
        .map(|n| n.trim_start_matches('/').to_string())
        .unwrap_or_else(|| id.chars().take(12).collect());
    Some(ContainerJob { id, name, status })
}

/// Run async work for each job with a hard concurrency ceiling.
///
/// Failed jobs should return `None` from `f`; successes return `Some(T)`.
/// Used by container stats collection so the cap can be unit-tested without Docker.
pub(crate) async fn collect_stats_capped<J, F, Fut, T>(
    jobs: impl IntoIterator<Item = J>,
    concurrency: usize,
    f: F,
) -> Vec<T>
where
    F: Fn(J) -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let limit = concurrency.max(1);
    stream::iter(jobs)
        .map(f)
        .buffer_unordered(limit)
        .filter_map(|s| async move { s })
        .collect()
        .await
}

fn sample_from_stats(
    id: &str,
    stats: &ContainerStatsResponse,
    sampled_at: DateTime<Utc>,
) -> ResourceSample {
    let cpu_percent = container_cpu_percent(stats);
    let memory_bytes = stats
        .memory_stats
        .as_ref()
        .and_then(|m| m.usage)
        .unwrap_or(0);
    let memory_limit = stats.memory_stats.as_ref().and_then(|m| m.limit);
    let (disk_read, disk_write) = blkio_bytes(stats);
    let (net_rx, net_tx) = network_bytes(stats);

    ResourceSample {
        kind: TargetKind::Container,
        id: id.chars().take(12).collect(),
        name: String::new(),
        cpu_percent,
        memory_bytes,
        memory_limit_bytes: memory_limit,
        // Cumulative Docker counters; Collector derives B/s across ticks.
        disk_read_bytes: disk_read,
        disk_write_bytes: disk_write,
        net_rx_bytes: net_rx,
        net_tx_bytes: net_tx,
        disk_read_bps: None,
        disk_write_bps: None,
        net_rx_bps: None,
        net_tx_bps: None,
        sampled_at,
        status: None,
    }
}

/// Docker CPU % calculation (aligned with `docker stats`).
fn container_cpu_percent(stats: &ContainerStatsResponse) -> f32 {
    let cpu = match stats.cpu_stats.as_ref() {
        Some(c) => c,
        None => return 0.0,
    };
    let precpu = match stats.precpu_stats.as_ref() {
        Some(c) => c,
        None => return 0.0,
    };

    let cpu_total = cpu
        .cpu_usage
        .as_ref()
        .and_then(|u| u.total_usage)
        .unwrap_or(0);
    let precpu_total = precpu
        .cpu_usage
        .as_ref()
        .and_then(|u| u.total_usage)
        .unwrap_or(0);
    let cpu_delta = cpu_total.saturating_sub(precpu_total) as f64;

    let system_delta = cpu
        .system_cpu_usage
        .unwrap_or(0)
        .saturating_sub(precpu.system_cpu_usage.unwrap_or(0)) as f64;

    let online = cpu
        .online_cpus
        .map(|n| n as f64)
        .or_else(|| {
            cpu.cpu_usage
                .as_ref()
                .and_then(|u| u.percpu_usage.as_ref())
                .map(|v| v.len() as f64)
        })
        .unwrap_or(1.0)
        .max(1.0);

    if system_delta > 0.0 && cpu_delta > 0.0 {
        ((cpu_delta / system_delta) * online * 100.0) as f32
    } else {
        0.0
    }
}

fn blkio_bytes(stats: &ContainerStatsResponse) -> (u64, u64) {
    let mut read = 0u64;
    let mut write = 0u64;

    let Some(blkio) = stats.blkio_stats.as_ref() else {
        return (0, 0);
    };
    let Some(entries) = blkio.io_service_bytes_recursive.as_ref() else {
        return (0, 0);
    };

    for e in entries {
        let value = e.value.unwrap_or(0);
        match e.op.as_deref().unwrap_or("") {
            "read" | "Read" => read = read.saturating_add(value),
            "write" | "Write" => write = write.saturating_add(value),
            _ => {}
        }
    }

    (read, write)
}

fn network_bytes(stats: &ContainerStatsResponse) -> (u64, u64) {
    let mut rx = 0u64;
    let mut tx = 0u64;

    if let Some(networks) = stats.networks.as_ref() {
        for data in networks.values() {
            rx = rx.saturating_add(data.rx_bytes.unwrap_or(0));
            tx = tx.saturating_add(data.tx_bytes.unwrap_or(0));
        }
    }

    (rx, tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn max_concurrent_stats_is_eight() {
        // Mutation-minded: plan and QUALITY.md document the M4 cap as 8.
        assert_eq!(MAX_CONCURRENT_STATS, 8);
    }

    #[test]
    fn disabled_is_not_available() {
        let c = ContainerCollector::disabled();
        assert!(!c.is_available());
    }

    #[tokio::test]
    async fn disabled_sample_returns_empty() {
        let c = ContainerCollector::disabled();
        let samples = c.sample().await.expect("disabled must not error");
        assert!(samples.is_empty());
    }

    #[test]
    fn job_from_list_strips_leading_slash_name() {
        let job = job_from_list_fields(
            Some("abcdef0123456789".into()),
            Some(&[String::from("/nginx")]),
            Some("Up 2 minutes".into()),
        )
        .expect("id present");
        assert_eq!(job.name, "nginx");
        assert_eq!(job.id, "abcdef0123456789");
        assert_eq!(job.status.as_deref(), Some("Up 2 minutes"));
    }

    #[test]
    fn job_from_list_falls_back_to_short_id_when_unnamed() {
        let job =
            job_from_list_fields(Some("0123456789abcdef".into()), None, None).expect("id present");
        assert_eq!(job.name, "0123456789ab");
        assert_eq!(job.name.chars().count(), 12);
    }

    #[test]
    fn job_from_list_skips_missing_id() {
        assert!(job_from_list_fields(None, Some(&[String::from("x")]), None).is_none());
    }

    #[test]
    fn jobs_from_summaries_drops_entries_without_id() {
        let with_id = ContainerSummary {
            id: Some("deadbeefcafebabe".into()),
            names: Some(vec!["/redis".into()]),
            status: Some("Up".into()),
            ..Default::default()
        };
        let without_id = ContainerSummary {
            id: None,
            names: Some(vec!["/ghost".into()]),
            ..Default::default()
        };
        let jobs = jobs_from_summaries(vec![with_id, without_id]);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "redis");
    }

    #[test]
    fn cpu_percent_zero_without_stats() {
        let stats = ContainerStatsResponse::default();
        assert_eq!(container_cpu_percent(&stats), 0.0);
    }

    #[tokio::test]
    async fn collect_stats_capped_never_exceeds_limit() {
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let n = 32usize;
        let jobs: Vec<usize> = (0..n).collect();

        let out = collect_stats_capped(jobs, MAX_CONCURRENT_STATS, |i| {
            let current = Arc::clone(&current);
            let peak = Arc::clone(&peak);
            async move {
                let running = current.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(running, Ordering::SeqCst);
                // Hold the slot long enough for the pool to fill under load.
                tokio::time::sleep(Duration::from_millis(15)).await;
                current.fetch_sub(1, Ordering::SeqCst);
                Some(i)
            }
        })
        .await;

        assert_eq!(out.len(), n, "all jobs should succeed");
        let peak = peak.load(Ordering::SeqCst);
        assert!(
            peak <= MAX_CONCURRENT_STATS,
            "peak concurrency {peak} exceeded cap {}",
            MAX_CONCURRENT_STATS
        );
        // With many more jobs than the cap, the pool should saturate.
        assert_eq!(
            peak, MAX_CONCURRENT_STATS,
            "expected saturation at cap with {n} jobs; peak was {peak}"
        );
    }

    #[tokio::test]
    async fn collect_stats_capped_skips_failures() {
        let jobs: Vec<u32> = (0..10).collect();
        let out = collect_stats_capped(jobs, 3, |i| async move {
            if i % 2 == 0 {
                Some(i)
            } else {
                None
            }
        })
        .await;

        assert_eq!(out, vec![0, 2, 4, 6, 8]);
    }

    #[tokio::test]
    async fn collect_stats_capped_empty_input() {
        let out: Vec<u8> =
            collect_stats_capped(Vec::<u8>::new(), MAX_CONCURRENT_STATS, |_j| async move {
                unreachable!("no jobs")
            })
            .await;
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn collect_stats_capped_zero_concurrency_becomes_one() {
        // Guard against accidental zero limit stalling or panicking.
        let out = collect_stats_capped(vec![1, 2, 3], 0, |i| async move { Some(i * 10) }).await;
        assert_eq!(out.len(), 3);
        assert!(out.contains(&10) && out.contains(&20) && out.contains(&30));
    }
}
