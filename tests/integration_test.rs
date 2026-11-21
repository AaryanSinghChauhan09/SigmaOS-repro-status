//! Integration tests for the SigmaOS repro-status component.

use std::io::Write;

use sigmaos_repro_status::{
    ArchwebPackage, LogType, Package, RebuilderdPackage, Status, filter_packages,
    localdb::{read_local_packages, split_version},
    ops::{print_results, summarize},
    package::Package as Pkg,
    syncdb::SyncDbIndex,
};

fn arch_pkg(name: &str, version: &str, rel: &str) -> ArchwebPackage {
    ArchwebPackage {
        pkgname: name.into(),
        pkgbase: name.into(),
        pkgver: version.into(),
        pkgrel: rel.into(),
        repo: "extra".into(),
        arch: "x86_64".into(),
        ..Default::default()
    }
}

#[test]
fn test_split_version_public() {
    assert_eq!(split_version("1:2.0-3"), (1, "2.0".into(), "3".into()));
}

#[test]
fn test_read_local_packages_integration() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path().join("local/foo-1.0-1");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(
        pkg.join("desc"),
        "%NAME%\nfoo\n%BASE%\nfoo\n%VERSION%\n1.0-1\n%ARCH%\nx86_64\n%DESC%\nFoo package\n%DESC_END%\n%LICENSE%\nMIT\n",
    )
    .unwrap();
    let pkgs = read_local_packages(dir.path()).unwrap();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].pkgname, "foo");
    assert_eq!(pkgs[0].pkgrel, "1");
}

#[test]
fn test_read_local_packages_rejects_non_pacman_dir() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read_local_packages(dir.path()).is_err());
}

#[test]
fn test_print_results_empty() {
    let mut out = Vec::new();
    print_results(&[], true, false, &mut out).unwrap();
    assert!(String::from_utf8(out).unwrap().contains("No packages found"));
}

#[test]
fn test_print_results_all_good() {
    let pkgs = vec![Package {
        data: arch_pkg("a", "1.0", "1"),
        status: Status::Good,
        build_id: 0,
        has_diffoscope: false,
    }];
    let mut out = Vec::new();
    print_results(&pkgs, false, false, &mut out).unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("All packages are reproducible!"));
    assert!(s.contains("100.00% reproducible"));
}

#[test]
fn test_filter_packages_by_status() {
    let mut pkgs = vec![
        Pkg {
            data: arch_pkg("g", "1", "1"),
            status: Status::Good,
            ..Default::default()
        },
        Pkg {
            data: arch_pkg("b", "1", "1"),
            status: Status::Bad,
            ..Default::default()
        },
    ];
    filter_packages(&mut pkgs, Some(Status::Bad), &[]);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].data.pkgname, "b");
}

#[test]
fn test_summarize_mixed() {
    let pkgs = vec![
        Pkg {
            data: arch_pkg("a", "1", "1"),
            status: Status::Good,
            ..Default::default()
        },
        Pkg {
            data: arch_pkg("b", "1", "1"),
            status: Status::Unknown,
            ..Default::default()
        },
    ];
    let s = summarize(&pkgs);
    assert_eq!(s.total, 2);
    assert_eq!(s.good, 1);
    assert_eq!(s.unknown, 1);
}

#[test]
fn test_syncdb_index_from_files() {
    // Build a plain-tar sync db in a tempdir.
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("extra.db");
    let mut tar_data = Vec::new();
    {
        let mut b = tar::Builder::new(&mut tar_data);
        let content = "%NAME%\nvim\n%BASE%\nvim\n%VERSION%\n9.0-1\n";
        let mut h = tar::Header::new_gnu();
        h.set_size(content.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, "vim/desc", content.as_bytes()).unwrap();
        b.finish().unwrap();
    }
    std::fs::write(&db_path, &tar_data).unwrap();
    let index = SyncDbIndex::from_files(&[db_path]).unwrap();
    assert!(index.contains("vim"));
    assert!(!index.contains("emacs"));
}

#[test]
fn test_rebuilderd_package_deserialize() {
    let json = br#"{"name":"foo","version":"1.0-1","status":"GOOD","buildId":42,"hasDiffoscope":true}"#;
    let pkg: RebuilderdPackage = serde_json::from_slice(json).unwrap();
    assert_eq!(pkg.name, "foo");
    assert_eq!(pkg.status, Status::Good);
    assert_eq!(pkg.build_id, Some(42));
    assert!(pkg.has_diffoscope);
}

#[test]
fn test_status_from_str() {
    assert_eq!("GOOD".parse::<Status>().unwrap(), Status::Good);
    assert_eq!("bad".parse::<Status>().unwrap(), Status::Bad);
    assert!("NOPE".parse::<Status>().is_err());
}

#[test]
fn test_log_type_display() {
    let mut w = Vec::new();
    write!(w, "{}", LogType::Build).unwrap();
    assert_eq!(w, b"build");
}
