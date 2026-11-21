//! Package data from archlinux.org, mirroring `archweb.rs` upstream.

use serde::{Deserialize, Serialize};

/// JSON endpoint for searching packages on archlinux.org.
pub const ARCHWEB_ENDPOINT: &str = "https://archlinux.org/packages/search/json";

/// Search result from archlinux.org.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// API version.
    pub version: i64,
    /// Result limit used.
    pub limit: i64,
    /// Whether the query was valid.
    pub valid: bool,
    /// Matching packages on this page.
    pub results: Vec<ArchwebPackage>,
    /// Total number of pages, if paginated.
    #[serde(rename = "num_pages")]
    pub num_pages: Option<i64>,
    /// Current page number, if paginated.
    pub page: Option<i64>,
}

/// Package data that archlinux.org provides.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchwebPackage {
    /// Binary package name.
    pub pkgname: String,
    /// Package base name.
    pub pkgbase: String,
    /// Repository (e.g. `core`, `extra`).
    pub repo: String,
    /// Architecture (e.g. `x86_64`).
    pub arch: String,
    /// Upstream version.
    pub pkgver: String,
    /// Package release.
    pub pkgrel: String,
    /// Epoch (0 when unset).
    pub epoch: i64,
    /// Short description.
    pub pkgdesc: String,
    /// Upstream URL.
    pub url: String,
    /// Package filename.
    pub filename: String,
    /// Download size in bytes.
    #[serde(rename = "compressed_size")]
    pub compressed_size: i64,
    /// Installed size in bytes.
    #[serde(rename = "installed_size")]
    pub installed_size: i64,
    /// Build date string, if known.
    #[serde(rename = "build_date")]
    pub build_date: Option<String>,
    /// Last update timestamp.
    #[serde(rename = "last_update")]
    pub last_update: String,
    /// Flag-for-orphan date, if flagged.
    #[serde(rename = "flag_date")]
    pub flag_date: Option<String>,
    /// Maintainer usernames.
    pub maintainers: Vec<String>,
    /// Last packager username.
    pub packager: String,
    /// Package groups.
    #[serde(default)]
    pub groups: Vec<serde_json::Value>,
    /// Licenses.
    #[serde(default)]
    pub licenses: Vec<String>,
    /// Conflicting packages.
    #[serde(default)]
    pub conflicts: Vec<serde_json::Value>,
    /// Provided virtual packages.
    #[serde(default)]
    pub provides: Vec<serde_json::Value>,
    /// Replaced packages.
    #[serde(default)]
    pub replaces: Vec<String>,
    /// Runtime dependencies.
    #[serde(default)]
    pub depends: Vec<String>,
    /// Optional dependencies.
    #[serde(default)]
    pub optdepends: Vec<String>,
    /// Build dependencies.
    #[serde(default)]
    pub makedepends: Vec<String>,
    /// Check dependencies.
    #[serde(default)]
    pub checkdepends: Vec<serde_json::Value>,
}

impl ArchwebPackage {
    /// Full version string: `[epoch:]pkgver[-pkgrel]`.
    ///
    /// pkgrel is appended unless pkgver already contains a `-` suffix, matching
    /// upstream's display logic.
    pub fn version_string(&self) -> String {
        let epoch = if self.epoch != 0 {
            format!("{}:", self.epoch)
        } else {
            String::new()
        };
        let rel = if !self.pkgver.contains('-') {
            format!("-{}", self.pkgrel)
        } else {
            String::new()
        };
        format!("{}{}{}", epoch, self.pkgver, rel)
    }

    /// URL of the package page on archlinux.org.
    pub fn package_url(&self) -> String {
        format!(
            "https://archlinux.org/packages/{}/{}/{}/",
            self.repo, self.arch, self.pkgbase
        )
    }
}

impl std::fmt::Display for ArchwebPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = String::new();
        let mut attr = |name: &str, value: &str| {
            out.push_str(&format!("\t{name:<16}: {value}\n"));
        };
        attr("Name", &self.pkgname);
        attr("Version", &self.version_string());
        attr("Architecture", &self.arch);
        if !self.repo.is_empty() {
            attr("Repository", &self.repo);
        }
        attr("Description", &self.pkgdesc);
        attr("Upstream URL", &self.url);
        attr("License(s)", &self.licenses.join(", "));
        if !self.maintainers.is_empty() {
            attr("Maintainer(s)", &self.maintainers.join(", "));
        }
        if self.compressed_size != 0 {
            attr("Package Size", &format_size(self.compressed_size));
        }
        attr("Installed Size", &format_size(self.installed_size));
        attr("Last Packager", &self.packager);
        if let Some(date) = &self.build_date {
            attr("Build Date", date);
        }
        if !self.last_update.is_empty() {
            attr("Last Updated", &self.last_update);
        }
        if let Some(date) = &self.flag_date {
            attr("Flag Date", date);
        }
        if !self.repo.is_empty() {
            attr("Package URL", &self.package_url());
        }
        f.write_str(&out)
    }
}

/// Human-readable byte size (IEC units), e.g. `645.2 kiB`.
pub fn format_size(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "kiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_string_epoch_and_rel() {
        let p = ArchwebPackage {
            epoch: 2,
            pkgver: "1.2.3".into(),
            pkgrel: "10".into(),
            ..Default::default()
        };
        assert_eq!(p.version_string(), "2:1.2.3-10");
    }

    #[test]
    fn test_version_string_no_epoch() {
        let p = ArchwebPackage {
            pkgver: "1.2.3".into(),
            pkgrel: "1".into(),
            ..Default::default()
        };
        assert_eq!(p.version_string(), "1.2.3-1");
    }

    #[test]
    fn test_version_string_pkgver_with_dash() {
        let p = ArchwebPackage {
            pkgver: "1.2.3-rc1".into(),
            pkgrel: "2".into(),
            ..Default::default()
        };
        assert_eq!(p.version_string(), "1.2.3-rc1");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(660662), "645.2 kiB");
        assert_eq!(format_size(512), "512 B");
    }
}
