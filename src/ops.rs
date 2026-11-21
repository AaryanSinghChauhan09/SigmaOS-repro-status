//! Query orchestration and result printing, mirroring upstream `lib.rs`.

use std::io::Write;

use crate::{
    archweb::ArchwebPackage,
    error::ReproError,
    fetch::{client, fetch_archweb_packages, fetch_rebuilderd_packages},
    localdb::read_local_packages,
    package::Package,
    rebuilderd::{RebuilderdPackage, Status},
    syncdb::SyncDbIndex,
};

/// User agent is defined in `fetch`.

/// Join archweb packages with rebuilderd statuses for a maintainer.
pub fn maintainer_packages(
    maintainer: &str,
    rebuilderd_url: &str,
) -> Result<Vec<Package>, ReproError> {
    let client = client()?;
    let archweb = fetch_archweb_packages(&client, maintainer)?;
    let rebuilderd = fetch_rebuilderd_packages(&client, rebuilderd_url)?;
    Ok(join_packages(archweb, &rebuilderd))
}

/// Join locally installed packages (filtered to sync repos unless `all`)
/// with rebuilderd statuses.
pub fn user_packages(
    db_path: &std::path::Path,
    repos: &[String],
    all: bool,
    rebuilderd_url: &str,
) -> Result<Vec<Package>, ReproError> {
    let client = client()?;
    let rebuilderd = fetch_rebuilderd_packages(&client, rebuilderd_url)?;
    let local = read_local_packages(db_path)?;
    let packages = if all {
        local
    } else {
        let index = SyncDbIndex::from_db_path(db_path, repos)?;
        local
            .into_iter()
            .filter(|p| index.contains(&p.pkgbase))
            .collect()
    };
    Ok(join_packages(packages, &rebuilderd))
}

/// Match archweb/local packages against rebuilderd results by pkgname.
fn join_packages(
    archweb: Vec<ArchwebPackage>,
    rebuilderd: &[RebuilderdPackage],
) -> Vec<Package> {
    archweb
        .into_iter()
        .map(|pkg| match rebuilderd.iter().find(|p| p.name == pkg.pkgname) {
            Some(p) => Package {
                data: pkg,
                status: p.status,
                build_id: p.build_id.unwrap_or(0),
                has_diffoscope: p.has_diffoscope,
            },
            None => Package::unknown(pkg),
        })
        .collect()
}

/// Filter packages by status and/or pkgname list.
pub fn filter_packages(
    packages: &mut Vec<Package>,
    filter: Option<Status>,
    pkgnames: &[String],
) {
    packages.retain(|p| {
        if let Some(f) = filter {
            if p.status != f {
                return false;
            }
        }
        pkgnames.is_empty() || pkgnames.contains(&p.data.pkgname)
    });
}

/// Summary statistics for a package list.
#[derive(Debug, Default)]
pub struct Summary {
    /// Total packages.
    pub total: usize,
    /// Packages with GOOD status.
    pub good: usize,
    /// Packages with BAD status.
    pub bad: usize,
    /// Packages with unknown status.
    pub unknown: usize,
}

impl Summary {
    /// Percentage of packages that are reproducible (GOOD).
    pub fn reproducible_pct(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.good as f64 / self.total as f64) * 100.0
        }
    }
}

/// Compute a summary over packages.
pub fn summarize(packages: &[Package]) -> Summary {
    let mut s = Summary {
        total: packages.len(),
        ..Default::default()
    };
    for p in packages {
        match p.status {
            Status::Good => s.good += 1,
            Status::Bad => s.bad += 1,
            Status::Unknown => s.unknown += 1,
        }
    }
    s
}

/// Print results in upstream's `[+]/[-]/[?]` style.
///
/// `color` enables ANSI styling; when false output is plain text.
pub fn print_results<W: Write>(
    packages: &[Package],
    is_local: bool,
    color: bool,
    output: &mut W,
) -> Result<(), ReproError> {
    let mut negatives = 0usize;
    for pkg in packages {
        if pkg.status != Status::Good {
            negatives += 1;
        }
        let marker = match pkg.status {
            Status::Good => if color { "\x1b[32m+\x1b[0m" } else { "+" },
            Status::Bad => if color { "\x1b[31m-\x1b[0m" } else { "-" },
            Status::Unknown => if color { "\x1b[33m?\x1b[0m" } else { "?" },
        };
        writeln!(output, "[{marker}] {pkg}")?;
    }
    let s = summarize(packages);
    if packages.is_empty() {
        writeln!(output, "No packages found.")?;
        return Ok(());
    }
    match negatives {
        0 => writeln!(output, "All packages are reproducible!")?,
        1 => writeln!(
            output,
            "1/{} package is not reproducible.{}",
            packages.len(),
            if packages.len() > 1 { " Almost there." } else { "" }
        )?,
        _ => writeln!(
            output,
            "{}/{} packages are not reproducible.",
            negatives, packages.len()
        )?,
    }
    writeln!(
        output,
        "Your {} {:.2}% reproducible.",
        if is_local { "system is" } else { "packages are" },
        s.reproducible_pct()
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rebuilderd::RebuilderdPackage;

    fn pkg(name: &str, status: Status) -> Package {
        Package {
            data: ArchwebPackage {
                pkgname: name.into(),
                pkgver: "1.0".into(),
                pkgrel: "1".into(),
                ..Default::default()
            },
            status,
            build_id: 0,
            has_diffoscope: false,
        }
    }

    #[test]
    fn test_join_packages_matches_by_name() {
        let archweb = vec![ArchwebPackage {
            pkgname: "foo".into(),
            pkgver: "1.0".into(),
            pkgrel: "1".into(),
            ..Default::default()
        }];
        let rb = vec![RebuilderdPackage {
            name: "foo".into(),
            version: "1.0-1".into(),
            status: Status::Good,
            build_id: Some(7),
            has_diffoscope: true,
        }];
        let joined = join_packages(archweb, &rb);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].status, Status::Good);
        assert_eq!(joined[0].build_id, 7);
        assert!(joined[0].has_diffoscope);
    }

    #[test]
    fn test_filter_by_status_and_name() {
        let mut pkgs = vec![pkg("a", Status::Good), pkg("b", Status::Bad)];
        filter_packages(&mut pkgs, Some(Status::Bad), &[]);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].data.pkgname, "b");
        filter_packages(&mut pkgs, None, &["zzz".to_string()]);
        assert!(pkgs.is_empty());
    }

    #[test]
    fn test_print_results_plain() {
        let pkgs = vec![pkg("test", Status::Good), pkg("xyz", Status::Bad)];
        let mut out = Vec::new();
        print_results(&pkgs, false, false, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("[+] test 1.0-1 GOOD"));
        assert!(text.contains("[-] xyz 1.0-1 BAD"));
        assert!(text.contains("1/2 package is not reproducible."));
        assert!(text.contains("50.00% reproducible"));
    }

    #[test]
    fn test_summarize() {
        let pkgs = vec![
            pkg("a", Status::Good),
            pkg("b", Status::Good),
            pkg("c", Status::Bad),
        ];
        let s = summarize(&pkgs);
        assert_eq!(s.total, 3);
        assert_eq!(s.good, 2);
        assert!((s.reproducible_pct() - 66.67).abs() < 0.01);
    }
}
