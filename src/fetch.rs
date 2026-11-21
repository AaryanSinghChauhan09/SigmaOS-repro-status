//! HTTP fetchers for archlinux.org and rebuilderd instances (blocking reqwest).

use crate::{
    archweb::{ARCHWEB_ENDPOINT, ArchwebPackage, SearchResult},
    error::ReproError,
    package::LogType,
    rebuilderd::RebuilderdPackage,
};

/// User agent identifying this tool.
pub const USER_AGENT: &str = concat!("sigmaos-repro-status/", env!("CARGO_PKG_VERSION"));

/// Build an HTTP client with the SigmaOS user agent.
pub fn client() -> Result<reqwest::blocking::Client, ReproError> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()?)
}

/// Fetch all packages maintained by `maintainer` from archlinux.org,
/// following pagination like upstream.
pub fn fetch_archweb_packages(
    client: &reqwest::blocking::Client,
    maintainer: &str,
) -> Result<Vec<ArchwebPackage>, ReproError> {
    let url = format!("{ARCHWEB_ENDPOINT}/?maintainer={maintainer}");
    let response: SearchResult = client.get(&url).send()?.json()?;
    let mut results = response.results;
    if let (Some(page), Some(num_pages)) = (response.page, response.num_pages) {
        for p in (page + 1)..=num_pages {
            let next: SearchResult = client
                .get(format!("{url}&page={p}"))
                .send()?
                .json()?;
            results.extend(next.results);
        }
    }
    Ok(results)
}

/// Fetch all package releases from a rebuilderd instance.
pub fn fetch_rebuilderd_packages(
    client: &reqwest::blocking::Client,
    rebuilder: &str,
) -> Result<Vec<RebuilderdPackage>, ReproError> {
    Ok(client
        .get(format!("{rebuilder}/api/v0/pkgs/list?distro=archlinux"))
        .send()?
        .json()?)
}

/// Fetch a build log or diffoscope output for a build ID.
pub fn fetch_rebuilderd_logs(
    client: &reqwest::blocking::Client,
    rebuilder: &str,
    build_id: i64,
    log_type: LogType,
) -> Result<String, ReproError> {
    let kind = match log_type {
        LogType::Build => "log",
        LogType::Diffoscope => "diffoscope",
    };
    Ok(client
        .get(format!("{rebuilder}/api/v0/builds/{build_id}/{kind}"))
        .send()?
        .text()?)
}
