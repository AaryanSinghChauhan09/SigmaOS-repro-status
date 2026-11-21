//! Error types for the reproducibility status component.

/// Errors that can occur while querying archweb, rebuilderd, or pacman dbs.
#[derive(Debug, thiserror::Error)]
pub enum ReproError {
    /// An HTTP request failed.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// A JSON payload could not be parsed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A pacman database file could not be read.
    #[error("pacman db error: {0}")]
    PacmanDb(String),

    /// A cache/log directory could not be created or written.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The pacman database path is invalid.
    #[error("invalid dbpath {0}: not a pacman database")]
    InvalidDbPath(String),
}
