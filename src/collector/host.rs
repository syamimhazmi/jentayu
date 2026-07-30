//! Host-level metrics via sysinfo.

use crate::metrics::HostSample;
use chrono::Utc;
use sysinfo::{Disks, Networks, System};

pub struct HostCollector {
    system: System,
    disks: Disks,
    networks: Networks,
}

impl HostCollector {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        // First CPU sample is often 0; refresh again after a short settle.
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_all();

        Self {
            system,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
        }
    }

    pub fn sample(&mut self) -> HostSample {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.disks.refresh(true);
        self.networks.refresh(true);

        let cpu_percent = self.system.global_cpu_usage();
        let (disk_total, disk_available) =
            self.disks.list().iter().fold((0u64, 0u64), |(t, a), d| {
                (
                    t.saturating_add(d.total_space()),
                    a.saturating_add(d.available_space()),
                )
            });

        let (net_rx, net_tx) = self
            .networks
            .iter()
            .fold((0u64, 0u64), |(rx, tx), (_, data)| {
                (
                    rx.saturating_add(data.total_received()),
                    tx.saturating_add(data.total_transmitted()),
                )
            });

        let load = System::load_average();

        HostSample {
            cpu_percent,
            memory_used_bytes: self.system.used_memory(),
            memory_total_bytes: self.system.total_memory(),
            swap_used_bytes: self.system.used_swap(),
            swap_total_bytes: self.system.total_swap(),
            disk_total_bytes: disk_total,
            disk_available_bytes: disk_available,
            // Cumulative interface totals; Collector derives B/s across ticks.
            net_rx_bytes: net_rx,
            net_tx_bytes: net_tx,
            net_rx_bps: None,
            net_tx_bps: None,
            load_avg_1: load.one,
            load_avg_5: load.five,
            load_avg_15: load.fifteen,
            sampled_at: Utc::now(),
        }
    }
}

impl Default for HostCollector {
    fn default() -> Self {
        Self::new()
    }
}
