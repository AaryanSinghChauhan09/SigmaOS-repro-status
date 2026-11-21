//! Pacman sync database reader.
//!
//! Upstream uses libalpm to check whether a local package belongs to a sync
//! repo. This implementation reads pacman's sync databases directly: each
//! `<dbpath>/sync/<repo>.db*` file is a tar archive (plain, gzip, or zstd)
//! containing a `desc` entry per package.

use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
};

use crate::error::ReproError;

/// Set of package base names across the given sync databases.
#[derive(Debug, Default)]
pub struct SyncDbIndex {
    pkgbases: HashSet<String>,
}

impl SyncDbIndex {
    /// Build an index from all sync db files in `<db_path>/sync`.
    ///
    /// Only files whose name matches one of `repos` (e.g. `core`, `extra`)
    /// are considered. Missing repos are skipped silently, matching pacman's
    /// tolerant behaviour for optional sync dbs.
    pub fn from_db_path(db_path: &Path, repos: &[String]) -> Result<Self, ReproError> {
        let sync_dir = db_path.join("sync");
        let mut index = SyncDbIndex::default();
        if !sync_dir.is_dir() {
            return Ok(index);
        }
        for repo in repos {
            for candidate in sync_db_candidates(&sync_dir, repo) {
                index.load_file(&candidate)?;
            }
        }
        Ok(index)
    }

    /// Build an index from explicit db file paths.
    pub fn from_files(files: &[PathBuf]) -> Result<Self, ReproError> {
        let mut index = SyncDbIndex::default();
        for f in files {
            index.load_file(f)?;
        }
        Ok(index)
    }

    /// Whether `pkgbase` exists in any loaded sync database.
    pub fn contains(&self, pkgbase: &str) -> bool {
        self.pkgbases.contains(pkgbase)
    }

    /// Number of indexed packages.
    pub fn len(&self) -> usize {
        self.pkgbases.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.pkgbases.is_empty()
    }

    fn load_file(&mut self, path: &Path) -> Result<(), ReproError> {
        let data = std::fs::read(path)
            .map_err(|e| ReproError::PacmanDb(format!("{}: {e}", path.display())))?;
        let tar_bytes = decompress(&data).ok_or_else(|| {
            ReproError::PacmanDb(format!("{}: unsupported compression", path.display()))
        })?;
        let mut archive = tar::Archive::new(tar_bytes.as_slice());
        for entry in archive.entries()? {
            let mut entry = entry?;
            if !entry.path()?.ends_with("desc") {
                continue;
            }
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            if let Some(base) = extract_base(&text) {
                self.pkgbases.insert(base);
            }
        }
        Ok(())
    }
}

/// Candidate sync db paths for a repo, checking common compression suffixes.
fn sync_db_candidates(sync_dir: &Path, repo: &str) -> Vec<PathBuf> {
    ["", ".tar.gz", ".tar.zst", ".tar.xz", ".gz"]
        .iter()
        .map(|suffix| sync_dir.join(format!("{repo}.db{suffix}")))
        .filter(|p| p.is_file())
        .collect()
}

/// Extract `%BASE%` (or `%NAME%` fallback) from a sync db `desc` entry.
fn extract_base(desc: &str) -> Option<String> {
    let mut lines = desc.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line == "%BASE%" {
            return lines.next().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        }
        if line == "%NAME%" {
            // NAME may precede BASE; keep scanning for BASE but remember NAME.
        }
    }
    // Fallback: first %NAME% value.
    let mut lines = desc.lines();
    while let Some(line) = lines.next() {
        if line.trim() == "%NAME%" {
            return lines.next().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        }
    }
    None
}

/// Decompress sync db bytes based on magic bytes.
fn decompress(data: &[u8]) -> Option<Vec<u8>> {
    // zstd magic: 0x28 0xB5 0x2F 0xFD
    if data.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        return zstd::stream::decode_all(data).ok().map(|v| v.to_vec());
    }
    // gzip magic: 0x1F 0x8B
    if data.starts_with(&[0x1F, 0x8B]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(data)
            .read_to_end(&mut out)
            .ok()?;
        return Some(out);
    }
    // xz magic: 0xFD 0x37 0x7A 0x58 0x5A 0x00
    if data.starts_with(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        // xz support omitted; return None so caller reports unsupported.
        return None;
    }
    // Assume plain tar.
    Some(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn build_test_db(dir: &Path, entries: &[(&str, &str)]) {
        let mut tar_data = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_data);
            for (name, content) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, name, content.as_bytes())
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        std::fs::create_dir_all(dir.join("sync")).unwrap();
        std::fs::write(dir.join("sync/core.db"), &tar_data).unwrap();
    }

    #[test]
    fn test_sync_db_index_finds_pkgbase() {
        let dir = tempfile::tempdir().unwrap();
        build_test_db(
            dir.path(),
            &[(
                "core-desc/desc",
                "%NAME%\nlinux\n%BASE%\nlinux\n%VERSION%\n6.1-1\n",
            )],
        );
        let index = SyncDbIndex::from_db_path(dir.path(), &["core".to_string()]).unwrap();
        assert!(index.contains("linux"));
        assert!(!index.contains("nonexistent"));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_extract_base_fallback_to_name() {
        assert_eq!(
            extract_base("%NAME%\nfoo\n%VERSION%\n1-1\n"),
            Some("foo".to_string())
        );
    }

    #[test]
    fn test_empty_index_for_missing_sync_dir() {
        let dir = tempfile::tempdir().unwrap();
        let index = SyncDbIndex::from_db_path(dir.path(), &[]).unwrap();
        assert!(index.is_empty());
    }
}
