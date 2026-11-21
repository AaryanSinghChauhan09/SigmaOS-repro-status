//! Composite package type joining archweb metadata with rebuilderd status.

use std::path::PathBuf;

use crate::{
    archweb::ArchwebPackage,
    error::ReproError,
    rebuilderd::Status,
};

/// Type of logs that rebuilderd provides.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LogType {
    /// Build logs.
    Build,
    /// Diffoscope comparison output.
    Diffoscope,
}

impl std::fmt::Display for LogType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}").to_lowercase())
    }
}

/// Package that combines archweb data with rebuilderd status.
#[derive(Debug, Clone)]
pub struct Package {
    /// Package metadata from archlinux.org (or the local pacman db).
    pub data: ArchwebPackage,
    /// Reproducibility status.
    pub status: Status,
    /// rebuilderd build ID (0 when unknown).
    pub build_id: i64,
    /// Whether rebuilderd recorded a diffoscope for the build.
    pub has_diffoscope: bool,
}

impl Default for Package {
    fn default() -> Self {
        Self {
            data: ArchwebPackage::default(),
            status: Status::Unknown,
            build_id: 0,
            has_diffoscope: false,
        }
    }
}

impl Package {
    /// Build a package with unknown reproducibility status.
    pub fn unknown(data: ArchwebPackage) -> Self {
        Self {
            data,
            status: Status::Unknown,
            build_id: 0,
            has_diffoscope: false,
        }
    }

    /// Path to cache logs for this package, e.g. `<cache>/<build_id>_<type>.log`.
    pub fn get_log_path(
        &self,
        log_type: LogType,
        cache_dir: Option<PathBuf>,
    ) -> Result<PathBuf, ReproError> {
        let dir = cache_dir
            .or_else(dirs_cache_dir)
            .ok_or_else(|| {
                ReproError::PacmanDb("cannot determine cache directory".to_string())
            })?;
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{}_{}.log", self.build_id, log_type)))
    }
}

/// `$XDG_CACHE_HOME/sigmaos-repro-status` or `~/.cache/sigmaos-repro-status`.
fn dirs_cache_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("sigmaos-repro-status"));
        }
    }
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".cache/sigmaos-repro-status"))
}

impl std::fmt::Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.data.pkgname,
            self.data.version_string(),
            self.status
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_package() {
        let pkg = Package {
            data: ArchwebPackage {
                pkgname: "test".into(),
                pkgver: "0.1".into(),
                pkgrel: "2".into(),
                ..Default::default()
            },
            status: Status::Good,
            build_id: 0,
            has_diffoscope: false,
        };
        assert_eq!(pkg.to_string(), "test 0.1-2 GOOD");
    }

    #[test]
    fn test_get_log_path() {
        let pkg = Package {
            build_id: 42,
            ..Default::default()
        };
        let path = pkg
            .get_log_path(LogType::Diffoscope, Some(PathBuf::from("testcache")))
            .unwrap();
        assert_eq!(path, PathBuf::from("testcache/42_diffoscope.log"));
    }
}
