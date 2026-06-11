use std::env;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

mod repo;
mod scan;
mod ui;

use ui::AppOpts;

#[derive(Parser)]
#[command(version, about = "git status dashboard for a directory of repos")]
struct Cli {
    /// Directory containing git repositories (default: $HOME/PANTHEON)
    #[arg(short, long)]
    target: Option<String>,

    /// Skip `git fetch`. Ahead/behind counts will be stale.
    #[arg(long)]
    no_fetch: bool,

    /// Skip per-repo fetch if it was last run less than N seconds ago.
    #[arg(long, default_value_t = 60)]
    fetch_ttl: u64,

    /// Seconds between automatic background rescans.
    #[arg(long, default_value_t = 30)]
    refresh: u64,

    /// Worker threads for parallel git ops.
    #[arg(short = 'j', long, default_value_t = 16)]
    jobs: usize,
}

fn default_target() -> String {
    let home = env::var("HOME").unwrap_or_else(|_| "/".into());
    format!("{home}/PANTHEON")
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let target = PathBuf::from(cli.target.unwrap_or_else(default_target));
    if !target.is_dir() {
        eprintln!("error: target not a directory: {}", target.display());
        std::process::exit(2);
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(cli.jobs.max(1))
        .build_global()
        .ok();

    ui::run(AppOpts {
        target,
        no_fetch: cli.no_fetch,
        fetch_ttl: Duration::from_secs(cli.fetch_ttl),
        refresh_interval: Duration::from_secs(cli.refresh),
    })
}
