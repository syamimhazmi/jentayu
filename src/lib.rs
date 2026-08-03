//! Jentayu — real-time process & Docker container resource monitor.
//!
//! Codename from Nusantara mythology: **Jentayu**, the mythical bird that soars
//! high and observes the world below — fitting for a resource monitor that
//! watches CPU, memory, disk, and network across processes and containers.

pub mod alert;
pub mod cli;
pub mod collector;
pub mod display;
pub mod error;
pub mod export;
pub mod metrics;
pub mod serve;
pub mod util;

pub use alert::{evaluate as evaluate_alerts, Alert, AlertThresholds};
pub use collector::{CollectMode, Collector};
pub use error::{JentayuError, Result};
pub use metrics::{
    filter_and_rank, format_bps, format_bps_pair, io_rate_bps, snapshot_for_export, HostSample,
    IoCounters, IoRateState, ResourceSample, Snapshot, SortBy, TargetKind,
};
pub use util::sanitize_control_chars;
