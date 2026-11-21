//! SigmaOS repro-status component.
//!
//! Rust implementation of [arch-repro-status](https://github.com/archlinux/arch-repro-status),
//! a CLI for querying Arch Linux package reproducibility status from a
//! [rebuilderd](https://reproducible.archlinux.org/) instance.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod archweb;
pub mod error;
pub mod fetch;
pub mod localdb;
pub mod ops;
pub mod package;
pub mod rebuilderd;
pub mod syncdb;

pub use archweb::{ARCHWEB_ENDPOINT, ArchwebPackage, SearchResult};
pub use error::ReproError;
pub use ops::{Summary, filter_packages, maintainer_packages, print_results, summarize, user_packages};
pub use package::{LogType, Package};
pub use rebuilderd::{RebuilderdPackage, Status};
