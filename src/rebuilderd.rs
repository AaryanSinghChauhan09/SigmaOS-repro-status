//! rebuilderd API data types, mirroring `rebuilderd_common` upstream.

use std::str::FromStr;

/// Reproducibility status of a package build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    /// All builders produced identical artifacts.
    Good,
    /// Builders disagreed; the package is not reproducible.
    Bad,
    /// No conclusive result (missing or pending build).
    Unknown,
}

impl FromStr for Status {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "GOOD" => Ok(Status::Good),
            "BAD" => Ok(Status::Bad),
            "UNKWN" | "UNKNOWN" => Ok(Status::Unknown),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Good => write!(f, "GOOD"),
            Status::Bad => write!(f, "BAD"),
            Status::Unknown => write!(f, "UNKN"),
        }
    }
}

/// Marker for a successful reproducibility check.
pub fn is_positive(status: Status) -> bool {
    status == Status::Good
}

/// A package release as reported by `rebuilderd /api/v0/pkgs/list`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuilderdPackage {
    /// Binary package name.
    pub name: String,
    /// Package version string.
    pub version: String,
    /// Reproducibility status.
    pub status: Status,
    /// Build ID, if a build was attempted.
    pub build_id: Option<i64>,
    /// Whether a diffoscope output was recorded.
    #[serde(default)]
    pub has_diffoscope: bool,
}

/// Build entry from `rebuilderd /api/v0/builds`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuilderdBuild {
    /// Build ID.
    pub id: i64,
    /// Package name.
    pub pkgname: String,
    /// Reproducibility status.
    pub status: Status,
}
