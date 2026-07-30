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
        std::fs::write(p, &body)?;
    }
    Ok(body)
}

/// Write Prometheus text exposition format.
pub fn write_prometheus(snapshot: &Snapshot, path: Option<&Path>) -> Result<String> {
    let body = render_prometheus(snapshot);
    if let Some(p) = path {
        std::fs::write(p, &body)?;
    }
    Ok(body)
}
