use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use rayon::prelude::*;

use crate::repo::{Repo, State};

pub struct ScanOpts {
    pub fetch: bool,
    pub fetch_ttl: Duration,
    pub force_fetch: bool,
}

pub fn scan(target: &Path, opts: &ScanOpts) -> Vec<Repo> {
    let mut entries = visible_subdirs(target);
    entries.sort();

    let mut repos: Vec<Repo> = entries
        .par_iter()
        .map(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            scan_one(p, name, opts)
        })
        .collect();

    repos.sort_by_cached_key(|r| (r.state as u8, !r.dirty, r.name.to_lowercase()));
    repos
}

fn visible_subdirs(target: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(target) else {
        return Vec::new();
    };
    rd.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| !n.starts_with('.'))
        })
        .map(|e| e.path())
        .collect()
}

fn scan_one(path: &Path, name: String, opts: &ScanOpts) -> Repo {
    if !is_git_repo(path) {
        return Repo::placeholder(name, State::NotRepo);
    }

    if opts.fetch && !cache_is_fresh(path, opts) {
        git_fetch(path);
        mark_fetched(path);
    }

    let branch = match git(path, &["rev-parse", "--abbrev-ref", "HEAD"]).as_deref() {
        Some("HEAD") => "(detached)".into(),
        Some(b) => b.into(),
        None => "(unknown)".into(),
    };

    let dirty = git(path, &["status", "--porcelain"]).is_some_and(|s| !s.is_empty());

    let (last_commit, commits) = commit_summary(path);
    let (state, ahead, behind) = upstream_state(path);

    Repo {
        name,
        state,
        dirty,
        ahead,
        behind,
        branch,
        last_commit,
        commits,
    }
}

fn commit_summary(path: &Path) -> (String, u64) {
    if git(path, &["rev-parse", "--verify", "HEAD"]).is_none() {
        return ("N/A".into(), 0);
    }
    let last = git(path, &["log", "-1", "--date=format:%d/%m/%y", "--format=%cd"])
        .unwrap_or_else(|| "N/A".into());
    let count = git(path, &["rev-list", "--count", "HEAD"])
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (last, count)
}

fn upstream_state(path: &Path) -> (State, u32, u32) {
    if git(
        path,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
    .is_none()
    {
        return (State::NoUpstream, 0, 0);
    }
    let lr = git(path, &["rev-list", "--left-right", "--count", "HEAD...@{u}"])
        .unwrap_or_default();
    let (ahead, behind) = parse_left_right(&lr);
    (classify(ahead, behind), ahead, behind)
}

fn parse_left_right(s: &str) -> (u32, u32) {
    let mut parts = s.split_whitespace();
    let a = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let b = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (a, b)
}

fn classify(ahead: u32, behind: u32) -> State {
    match (ahead > 0, behind > 0) {
        (true, true) => State::Diverged,
        (true, false) => State::Push,
        (false, true) => State::Pull,
        (false, false) => State::Synced,
    }
}

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn is_git_repo(path: &Path) -> bool {
    git(path, &["rev-parse", "--is-inside-work-tree"]).as_deref() == Some("true")
}

// Silent on failure — the local-status pass handles whatever ref state exists.
fn git_fetch(path: &Path) {
    let _ = Command::new("git")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "/usr/bin/true")
        .arg("-C")
        .arg(path)
        .arg("-c")
        .arg("http.lowSpeedLimit=1000")
        .arg("-c")
        .arg("http.lowSpeedTime=10")
        .arg("fetch")
        .arg("--quiet")
        .arg("--no-tags")
        .arg("origin")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn cache_is_fresh(path: &Path, opts: &ScanOpts) -> bool {
    !opts.force_fetch && last_fetch_age(path).is_some_and(|age| age < opts.fetch_ttl)
}

fn cache_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env::var_os("HOME")?)
            .join(".cache")
            .join("talos");
        fs::create_dir_all(&dir).ok()?;
        Some(dir)
    })
    .as_deref()
}

fn cache_path(repo_path: &Path) -> Option<PathBuf> {
    let name = repo_path.file_name()?.to_str()?;
    Some(cache_dir()?.join(format!("{name}.fetch")))
}

fn last_fetch_age(repo_path: &Path) -> Option<Duration> {
    let mtime = fs::metadata(cache_path(repo_path)?)
        .ok()?
        .modified()
        .ok()?;
    SystemTime::now().duration_since(mtime).ok()
}

fn mark_fetched(repo_path: &Path) {
    if let Some(p) = cache_path(repo_path) {
        let _ = fs::write(&p, b"");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_left_right_count() {
        assert_eq!(parse_left_right("0\t0"), (0, 0));
        assert_eq!(parse_left_right("3\t0"), (3, 0));
        assert_eq!(parse_left_right("0\t44"), (0, 44));
        assert_eq!(parse_left_right("2\t5"), (2, 5));
    }

    #[test]
    fn parses_left_right_defaults_on_garbage() {
        assert_eq!(parse_left_right(""), (0, 0));
        assert_eq!(parse_left_right("nope"), (0, 0));
    }

    #[test]
    fn classify_states() {
        assert_eq!(classify(0, 0), State::Synced);
        assert_eq!(classify(3, 0), State::Push);
        assert_eq!(classify(0, 1), State::Pull);
        assert_eq!(classify(2, 5), State::Diverged);
    }
}
