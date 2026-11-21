//! Pacman local database reader.
//!
//! Reads `<dbpath>/local/<pkg>/desc` files (the `%FIELD%` + value-line format)
//! and converts them into [`ArchwebPackage`] entries, splitting the pacman
//! `VERSION` field back into epoch/pkgver/pkgrel.

use std::{collections::HashMap, path::Path};

use crate::{archweb::ArchwebPackage, error::ReproError};

/// Read all locally installed packages from a pacman database path.
pub fn read_local_packages(db_path: &Path) -> Result<Vec<ArchwebPackage>, ReproError> {
    let local_dir = db_path.join("local");
    if !local_dir.is_dir() {
        return Err(ReproError::InvalidDbPath(db_path.display().to_string()));
    }
    let mut packages = Vec::new();
    for entry in std::fs::read_dir(&local_dir)? {
        let entry = entry?;
        let desc = entry.path().join("desc");
        if desc.is_file() {
            if let Some(pkg) = parse_desc(&desc) {
                packages.push(pkg);
            }
        }
    }
    Ok(packages)
}

fn is_marker(line: &str) -> bool {
    line.len() > 2 && line.starts_with('%') && line.ends_with('%')
}

/// Parse a pacman `desc` file into field → value-lines map.
///
/// Format: `%FIELD%` marker lines followed by value lines; a field's value
/// ends at the next `%FIELD%` marker.
fn parse_fields(text: &str) -> HashMap<String, Vec<String>> {
    let mut fields: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim_end();
        if is_marker(line) {
            let name = line.trim_matches('%').to_string();
            fields.entry(name.clone()).or_default();
            current = Some(name);
        } else if let Some(field) = &current {
            fields.get_mut(field).unwrap().push(line.trim().to_string());
        }
    }
    fields
}

fn field<'a>(fields: &'a HashMap<String, Vec<String>>, name: &str) -> &'a str {
    fields
        .get(name)
        .and_then(|v| v.first())
        .map(String::as_str)
        .unwrap_or("")
        .trim()
}

/// Parse a pacman `desc` file into an [`ArchwebPackage`].
fn parse_desc(path: &Path) -> Option<ArchwebPackage> {
    let text = std::fs::read_to_string(path).ok()?;
    let fields = parse_fields(&text);
    let version = field(&fields, "VERSION").to_string();
    if version.is_empty() {
        return None;
    }
    let name = field(&fields, "NAME").to_string();
    let base = field(&fields, "BASE").to_string();
    let (epoch, pkgver, pkgrel) = split_version(&version);
    let build_date = field(&fields, "BUILDDATE")
        .trim()
        .parse::<i64>()
        .ok()
        .map(unix_to_iso);
    let licenses: Vec<String> = fields
        .get("LICENSE")
        .map(|v| v.iter().filter(|s| !s.is_empty()).cloned().collect())
        .unwrap_or_default();
    let desc_text = fields
        .get("DESC")
        .map(|v| {
            v.iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    Some(ArchwebPackage {
        pkgname: if name.is_empty() { base.clone() } else { name },
        pkgbase: if base.is_empty() {
            fields
                .get("NAME")
                .and_then(|v| v.first())
                .cloned()
                .unwrap_or_default()
        } else {
            base
        },
        repo: String::new(),
        arch: {
            let a = field(&fields, "ARCH");
            if a.is_empty() {
                "x86_64".into()
            } else {
                a.to_string()
            }
        },
        pkgver,
        pkgrel,
        epoch,
        pkgdesc: desc_text,
        url: field(&fields, "URL").to_string(),
        filename: String::new(),
        compressed_size: 0,
        installed_size: 0,
        build_date,
        last_update: String::new(),
        flag_date: None,
        maintainers: Vec::new(),
        packager: field(&fields, "PACKAGER").to_string(),
        groups: Vec::new(),
        licenses,
        conflicts: Vec::new(),
        provides: Vec::new(),
        replaces: Vec::new(),
        depends: Vec::new(),
        optdepends: Vec::new(),
        makedepends: Vec::new(),
        checkdepends: Vec::new(),
    })
}

/// Split pacman's `VERSION` (`[epoch:]pkgver-pkgrel`) into components.
pub fn split_version(version: &str) -> (i64, String, String) {
    let (epoch, rest) = match version.split_once(':') {
        Some((e, r)) => (e.parse().unwrap_or(0), r),
        None => (0, version),
    };
    match rest.rsplit_once('-') {
        Some((v, r)) => (epoch, v.to_string(), r.to_string()),
        None => (epoch, rest.to_string(), String::new()),
    }
}

/// Minimal unix-timestamp → `YYYY-MM-DD HH:MM:SS` UTC conversion.
fn unix_to_iso(ts: i64) -> String {
    let days = ts.div_euclid(86400);
    let secs = ts.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m,
        d,
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_version_plain() {
        assert_eq!(split_version("1.2.3-4"), (0, "1.2.3".into(), "4".into()));
    }

    #[test]
    fn test_split_version_epoch() {
        assert_eq!(split_version("2:1.0-1"), (2, "1.0".into(), "1".into()));
    }

    #[test]
    fn test_split_version_no_rel() {
        assert_eq!(split_version("1.0"), (0, "1.0".into(), "".into()));
    }

    #[test]
    fn test_unix_to_iso_epoch() {
        assert_eq!(unix_to_iso(0), "1970-01-01 00:00:00");
    }

    #[test]
    fn test_parse_desc_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let desc = dir.path().join("desc");
        std::fs::write(
            &desc,
            "%NAME%\nfoo\n%BASE%\nfoo\n%VERSION%\n1.0-1\n%ARCH%\nx86_64\n%DESC%\nA test\npackage\n%LICENSE%\nMIT\nGPL\n%PACKAGER%\ntu\n",
        )
        .unwrap();
        let pkg = parse_desc(&desc).unwrap();
        assert_eq!(pkg.pkgname, "foo");
        assert_eq!(pkg.pkgbase, "foo");
        assert_eq!(pkg.pkgver, "1.0");
        assert_eq!(pkg.pkgrel, "1");
        assert_eq!(pkg.pkgdesc, "A test package");
        assert_eq!(pkg.licenses, vec!["MIT", "GPL"]);
        assert_eq!(pkg.packager, "tu");
    }
}
