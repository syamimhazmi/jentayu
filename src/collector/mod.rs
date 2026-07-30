//! Metric collectors for host, processes, and Docker containers.

mod container;
mod host;
mod process;
mod process_net;

pub use container::ContainerCollector;
pub use host::HostCollector;
pub use process::ProcessCollector;
pub use process_net::{process_net_supported, sample_process_net, ProcessNetBytes};

use crate::metrics::{IoRateState, ResourceSample, Snapshot, TargetKind};
use chrono::Utc;
use tracing::{debug, warn};

/// What to collect on each tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectMode {
    /// Host processes only.
    Processes,
    /// Docker containers only.
    Containers,
    /// Both processes and containers.
    All,
}

impl CollectMode {
    pub fn from_flags(processes: bool, containers: bool) -> Self {
        match (processes, containers) {
            (true, false) => Self::Processes,
            (false, true) => Self::Containers,
            _ => Self::All,
        }
    }
}

/// Orchestrates concurrent collection across sources.
pub struct Collector {
    host: HostCollector,
    processes: ProcessCollector,
    containers: ContainerCollector,
    mode: CollectMode,
    /// Previous I/O counters for interval B/s rates.
    io_rates: IoRateState,
}

impl Collector {
    pub async fn new(mode: CollectMode) -> Self {
        let containers = match ContainerCollector::connect().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Docker unavailable; container metrics disabled");
                ContainerCollector::disabled()
            }
        };

        Self::with_containers(mode, containers)
    }

    /// Build a collector with an injected container source (mock Docker, disabled, etc.).
    pub fn with_containers(mode: CollectMode, containers: ContainerCollector) -> Self {
        Self {
            host: HostCollector::new(),
            processes: ProcessCollector::new(),
            containers,
            mode,
            io_rates: IoRateState::new(),
        }
    }

    pub fn mode(&self) -> CollectMode {
        self.mode
    }

    pub fn docker_available(&self) -> bool {
        self.containers.is_available()
    }

    /// Take one full snapshot.
    pub async fn snapshot(&mut self) -> Snapshot {
        let host = self.host.sample();
        let mut targets: Vec<ResourceSample> = Vec::new();

        match self.mode {
            CollectMode::Processes => {
                targets.extend(self.processes.sample());
            }

            CollectMode::Containers => match self.containers.sample().await {
                Ok(rows) => targets.extend(rows),
                Err(e) => warn!(error = %e, "container sample failed"),
            },

            CollectMode::All => {
                targets.extend(self.processes.sample());
                match self.containers.sample().await {
                    Ok(rows) => targets.extend(rows),
                    Err(e) => warn!(error = %e, "container sample failed"),
                }
            }
        }

        debug!(
            processes = targets
                .iter()
                .filter(|t| t.kind == TargetKind::Process)
                .count(),
            containers = targets
                .iter()
                .filter(|t| t.kind == TargetKind::Container)
                .count(),
            "snapshot collected"
        );

        let mut snapshot = Snapshot {
            host,
            targets,
            sampled_at: Utc::now(),
        };
        // Derive disk/net B/s from the previous tick's cumulative counters.
        self.io_rates.apply(&mut snapshot);
        snapshot
    }
}
