//! Error types for Jentayu.

use thiserror::Error;

/// Application-level errors.
#[derive(Debug, Error)]
pub enum JentayuError {
    #[error("Docker is unavailable: {0}")]
    DockerUnavailable(String),

    #[error("failed to collect process metrics: {0}")]
    ProcessCollect(String),

    #[error("failed to collect container metrics: {0}")]
    ContainerCollect(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, JentayuError>;
