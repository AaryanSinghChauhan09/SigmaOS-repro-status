//! `sigmaos-repro-status` CLI — Arch package reproducibility status.
//!
//! Mirrors upstream `arch-repro-status` options:
//!   -m, --maintainer USER   packages of a maintainer
//!   -a, --all               include all local packages (not just sync repos)
//!   -r, --rebuilderd URL    rebuilderd instance (default reproducible.archlinux.org)
//!   -b, --dbpath PATH       pacman db path (default /var/lib/pacman)
//!       --repos REPOS       comma-separated repos (default core,extra,community,multilib)
//!   -n, --pkgnames NAMES    comma-separated package names
//!   -f, --filter STATUS     GOOD | BAD | UNKWN
//!   -i, --inspect           fetch and print build log/diffoscope for a package
//!   -q, --quiet             less logging
//!   -p, --pager CMD         pager for -i (default: cat when non-tty)
//!   -c, --cache-dir DIR     log cache dir

use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use sigmaos_repro_status::{
    LogType, Package, Status, filter_packages, maintainer_packages, print_results,
    user_packages,
};

struct Cli {
    maintainer: Option<String>,
    all: bool,
    inspect: bool,
    quiet: bool,
    rebuilderd: String,
    dbpath: PathBuf,
    repos: Vec<String>,
    pkgnames: Vec<String>,
    filter: Option<Status>,
    pager: String,
    cache_dir: Option<PathBuf>,
}

const DEFAULT_REPOS: &str = "core,extra,community,multilib";

fn parse_args() -> Result<Cli, String> {
    let mut cli = Cli {
        maintainer: std::env::var("MAINTAINER").ok().filter(|s| !s.is_empty()),
        all: false,
        inspect: false,
        quiet: false,
        rebuilderd: std::env::var("REBUILDERD")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "https://reproducible.archlinux.org".into()),
        dbpath: std::env::var("DBPATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/var/lib/pacman")),
        repos: DEFAULT_REPOS.split(',').map(str::to_string).collect(),
        pkgnames: std::env::var("PKGNAMES")
            .map(|s| s.split(',').map(str::to_string).collect())
            .unwrap_or_default(),
        filter: std::env::var("FILTER")
            .ok()
            .and_then(|s| Status::from_str(&s).ok()),
        pager: std::env::var("PAGER").unwrap_or_else(|_| "cat".into()),
        cache_dir: std::env::var("CACHE_DIR").map(PathBuf::from).ok(),
    };

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut take = |name: &str| -> Result<String, String> {
            args.next().ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "-a" | "--all" => cli.all = true,
            "-i" | "--inspect" => cli.inspect = true,
            "-q" | "--quiet" => cli.quiet = true,
            "-m" | "--maintainer" => cli.maintainer = Some(take("--maintainer")?),
            "-r" | "--rebuilderd" => cli.rebuilderd = take("--rebuilderd")?,
            "-b" | "--dbpath" => cli.dbpath = PathBuf::from(take("--dbpath")?),
            "--repos" => {
                cli.repos = take("--repos")?.split(',').map(str::to_string).collect()
            }
            "-n" | "--pkgnames" => {
                cli.pkgnames = take("--pkgnames")?.split(',').map(str::to_string).collect()
            }
            "-f" | "--filter" => {
                let v = take("--filter")?;
                cli.filter = Some(
                    Status::from_str(&v).map_err(|_| format!("invalid filter {v}"))?,
                );
            }
            "-p" | "--pager" => cli.pager = take("--pager")?,
            "-c" | "--cache-dir" => cli.cache_dir = Some(PathBuf::from(take("--cache-dir")?)),
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}")),
            _ => return Err(format!("unexpected argument: {arg}")),
        }
    }
    Ok(cli)
}

fn print_usage() {
    eprintln!(
        "sigmaos-repro-status — Arch package reproducibility status\n\
         \n\
         USAGE: sigmaos-repro-status [OPTIONS]\n\
         \n\
         OPTIONS:\n    -m, --maintainer USER   maintainer packages (env MAINTAINER)\n    \
             -a, --all               all local packages (env: default sync repos only)\n    -r, \
             --rebuilderd URL        rebuilderd instance [default: https://reproducible.archlinux.org]\n    \
             -b, --dbpath PATH        pacman db path [default: /var/lib/pacman]\n        \
             --repos REPOS           repos csv [default: core,extra,community,multilib]\n    -n, \
             --pkgnames NAMES        package names csv\n    -f, --filter STATUS     GOOD|BAD|UNKWN\n    \
             -i, --inspect            print build log/diffoscope\n    -p, --pager CMD         pager \
             [default: cat]\n    -c, --cache-dir DIR     log cache dir\n    -q, --quiet             \
             less logging"
    );
}

fn run(cli: &Cli) -> Result<(), String> {
    let mut packages = if let Some(m) = &cli.maintainer {
        maintainer_packages(m, &cli.rebuilderd)
    } else {
        user_packages(&cli.dbpath, &cli.repos, cli.all, &cli.rebuilderd)
    }
    .map_err(|e| e.to_string())?;

    filter_packages(&mut packages, cli.filter, &cli.pkgnames);

    if cli.inspect {
        inspect(&packages, cli)?;
        return Ok(());
    }

    let color = !cli.quiet
        && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(false);
    let is_local = cli.maintainer.is_none();
    print_results(&packages, is_local, color, &mut std::io::stdout())
        .map_err(|e| e.to_string())
}

/// Interactive-ish inspection: pick a package by index, fetch and print its
/// build log or diffoscope through the pager.
fn inspect(packages: &[Package], cli: &Cli) -> Result<(), String> {
    if packages.is_empty() {
        return Err("no packages to inspect".into());
    }
    for (i, p) in packages.iter().enumerate() {
        eprintln!("[{i}] {p}");
    }
    eprint!("select package index: ");
    use std::io::Write;
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    let idx: usize = line.trim().parse().map_err(|_| "invalid index".to_string())?;
    let pkg = packages.get(idx).ok_or("index out of range")?;

    let log_type = if pkg.has_diffoscope {
        eprintln!("package has diffoscope; fetching diffoscope (build log also available)");
        LogType::Diffoscope
    } else {
        LogType::Build
    };
    let client = sigmaos_repro_status::fetch::client().map_err(|e| e.to_string())?;
    let logs = sigmaos_repro_status::fetch::fetch_rebuilderd_logs(
        &client,
        &cli.rebuilderd,
        pkg.build_id,
        log_type,
    )
    .map_err(|e| e.to_string())?;

    if let Some(dir) = &cli.cache_dir {
        let path = dir.join(format!("{}_{}.log", pkg.build_id, log_type));
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        std::fs::write(&path, &logs).map_err(|e| e.to_string())?;
        eprintln!("cached at {}", path.display());
    }
    print!("{logs}");
    Ok(())
}

fn main() -> ExitCode {
    let cli = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

// Anchor: LogType used in inspect.
#[allow(dead_code)]
fn _anchor(_: LogType, _: Package) {}
