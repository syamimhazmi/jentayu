//! Command-line interface definition.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Jentayu — real-time process & Docker container resource monitor.
///
/// Named after the mythical Nusantara bird that soars high and sees all.
#[derive(Debug, Parser)]
#[command(
    name = "jentayu",
    version,
    about = "Real-time CPU, memory, disk & network monitor for processes and Docker containers",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Filter targets by name (substring, case-insensitive).
    /// Also applies to `prometheus` / `json` when set (with `--top`).
    #[arg(short = 'f', long = "filter", global = true)]
    pub filter: Option<String>,

    /// Show only the top N consumers.
    /// Also applies to `prometheus` / `json` when set (with `--filter`).
    #[arg(short = 'n', long = "top", global = true)]
    pub top: Option<usize>,

    /// Sort by metric: cpu, memory, disk, network.
    /// For export: used only when `--filter` and/or `--top` is set.
    #[arg(short = 's', long = "sort", default_value = "cpu", global = true)]
    pub sort: SortMetric,

    /// Refresh interval in seconds (watch / tui).
    #[arg(short = 'i', long = "interval", default_value_t = 2, global = true)]
    pub interval: u64,

    /// Collect host processes (default: on when no --containers-only).
    #[arg(long = "processes", global = true)]
    pub processes: bool,

    /// Collect Docker containers only.
    #[arg(long = "containers-only", global = true)]
    pub containers_only: bool,

    /// Skip process collection (containers + host only when Docker available).
    #[arg(long = "no-processes", global = true)]
    pub no_processes: bool,

    /// CPU alert threshold percent (e.g. 80).
    #[arg(long = "alert-cpu", global = true)]
    pub alert_cpu: Option<f32>,

    /// Memory alert threshold percent (e.g. 90).
    #[arg(long = "alert-mem", global = true)]
    pub alert_mem: Option<f32>,

    /// Increase logging verbosity (-v, -vv).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// One-shot snapshot as a table (default).
    Once,

    /// Continuously refresh table output in the terminal.
    Watch,

    /// Interactive TUI dashboard.
    Tui,

    /// Export metrics in Prometheus text format.
    ///
    /// Global `-f` / `-n` apply ranking when either is set; full per-target
    /// inventory when neither is set. `-s` is used only when ranking.
    /// `-o` writes via same-dir temp + rename (same-FS atomic).
    Prometheus {
        /// Write to file instead of stdout (same-dir temp + rename; trusted path).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Export metrics as JSON.
    ///
    /// Global `-f` / `-n` apply ranking when either is set; full per-target
    /// inventory when neither is set. `-s` is used only when ranking.
    /// `-o` writes via same-dir temp + rename (same-FS atomic).
    Json {
        /// Write to file instead of stdout (same-dir temp + rename; trusted path).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum SortMetric {
    #[default]
    Cpu,
    Memory,
    Disk,
    Network,
}

impl From<SortMetric> for crate::metrics::SortBy {
    fn from(value: SortMetric) -> Self {
        match value {
            SortMetric::Cpu => Self::Cpu,
            SortMetric::Memory => Self::Memory,
            SortMetric::Disk => Self::Disk,
            SortMetric::Network => Self::Network,
        }
    }
}

impl Cli {
    /// Resolve collect flags into (want_processes, want_containers).
    pub fn collect_flags(&self) -> (bool, bool) {
        if self.containers_only {
            return (false, true);
        }
        if self.no_processes {
            return (false, true);
        }
        if self.processes {
            return (true, false);
        }
        // Default: both.
        (true, true)
    }
}
