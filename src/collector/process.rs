//! Process-level metrics via sysinfo.
//!
//! Network counters are filled best-effort on Linux (`process_net`); other OSes
//! keep cumulative network at 0.

use crate::collector::process_net::sample_process_net;
use crate::metrics::{ResourceSample, TargetKind};
use chrono::Utc;
use sysinfo::{DiskUsage, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub struct ProcessCollector {
    system: System,
}

impl ProcessCollector {
    pub fn new() -> Self {
        let mut system = System::new();

        system.refresh_memory();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );

        // CPU percent needs two samples.
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage(),
        );

        Self { system }
    }

    pub fn sample(&mut self) -> Vec<ResourceSample> {
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );

        let total_mem = self.system.total_memory();
        let now = Utc::now();

        let pids: Vec<u32> = self
            .system
            .processes()
            .keys()
            .map(|pid| pid.as_u32())
            .collect();
        // Linux: sock-diag + private-netns /proc; other OS: empty → zeros below.
        let net_by_pid = sample_process_net(&pids);

        self.system
            .processes()
            .iter()
            .map(|(pid, proc_)| {
                let disk: DiskUsage = proc_.disk_usage();
                let name = {
                    let n = proc_.name().to_string_lossy();
                    if n.is_empty() {
                        format!("pid-{}", pid.as_u32())
                    } else {
                        n.into_owned()
                    }
                };

                let pid_u = pid.as_u32();
                let (net_rx, net_tx) = net_by_pid
                    .get(&pid_u)
                    .map(|n| (n.rx, n.tx))
                    .unwrap_or((0, 0));

                ResourceSample {
                    kind: TargetKind::Process,
                    id: pid_u.to_string(),
                    name,
                    cpu_percent: proc_.cpu_usage(),
                    memory_bytes: proc_.memory(),
                    memory_limit_bytes: Some(total_mem),
                    // Cumulative totals; Collector derives B/s rates across ticks.
                    disk_read_bytes: disk.total_read_bytes,
                    disk_write_bytes: disk.total_written_bytes,
                    net_rx_bytes: net_rx,
                    net_tx_bytes: net_tx,
                    disk_read_bps: None,
                    disk_write_bps: None,
                    net_rx_bps: None,
                    net_tx_bps: None,
                    sampled_at: now,
                    status: Some(format!("{:?}", proc_.status())),
                }
            })
            .collect()
    }
}

impl Default for ProcessCollector {
    fn default() -> Self {
        Self::new()
    }
}
