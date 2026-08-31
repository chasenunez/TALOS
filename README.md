[![DOI](https://zenodo.org/badge/1265871816.svg)](https://doi.org/10.5281/zenodo.22206974)

# TALOS

A terminal dashboard for a directory full of git repos. it tells you which ones need pulling, which ones need pushing, which ones have uncommitted work, and which ones aren't actually git repos at all. Originally a bash script, now in Rust. 

## layout

- `src/main.rs` — clap CLI, rayon thread-pool setup, hands off to the UI.
- `src/ui.rs` — ratatui dashboard, key handling, background scan thread.
- `src/scan.rs` — parallel directory walk, git plumbing, fetch cache.
- `src/repo.rs` — `Repo` and `State` types and their colours.

## install

You will need:

- a rust toolchain on edition 2024 (rustc 1.85+). easiest path is
  [rustup](https://rustup.rs).
- `git` on `$PATH` — talos shells out to git for every operation.
- macOS or Linux. the fetch shim points `GIT_ASKPASS` at `/usr/bin/true` and
  the cache lives under `$HOME/.cache/talos`.

from this directory:

```sh
cargo build --release
```

the binary lands at `target/release/talos`. drop it on your PATH (`cp
target/release/talos ~/.local/bin/`) or call it directly.

## first run

```sh
git clone https://github.com/<you>/talos.git
cd talos
cargo run --release -- --target ~/path/to/your/repos
```

If you'd like to install TALOS so it is reachable without navigating to the script folder, you can use:

```sh
cargo install --path .
```

The default target (the directory that holds all your git repositories) is `~/PANTHEON`.
To make it your own, pass `--target` and point at any directory whose immediate children are git clones. talos only walks one level deep and hidden subdirectories are skipped, so nesting like `~/code/work/foo` won't get picked up unless you point at `~/code/work` directly.

## use

```sh
talos                       # scan ~/PANTHEON, fetch + status
talos --target ~/code       # scan somewhere else
talos --no-fetch            # skip the network round-trip (fast, stale data)
talos --force-fetch         # ignore the fetch cache and refetch everything
talos --fetch-ttl 300       # only refetch a repo if last fetch was >5min ago
talos --refresh 60          # seconds between background auto-rescans (default 30)
talos -j 4                  # fewer parallel workers (default 16)
```

keys, once the dashboard is up:

- `r` — rescan now (respects the fetch cache).
- `f` — force-fetch every repo on the next rescan.
- `q` / Esc / Ctrl-C — quit.

example output:

```
╺┳╸┏━┓╻  ┏━┓┏━┓
 ┃ ┣━┫┃  ┃ ┃┗━┓
 ╹ ╹ ╹┗━╸┗━┛┗━┛

Pull: 2/23  Push: 0/23  Dirty: 1/23  No upstream: 0/23

PATH: /Users/you/VAULT

#    repo               state       *  +/-    last commit       Σ
---- ------------------ ----------- -  ------ ----------- -------
1    REPO_01             synced      *  0/0    08/06/26          2
2    REPO_02             synced         0/0    20/01/26          5
...
22   REPO_22             pull           0/1    29/04/26         17
23   REPO_23             pull           0/44   29/04/26        922
```

## what the columns mean

| column | meaning |
| --- | --- |
| `#` | row index |
| `repo` | directory name |
| `state` | upstream relationship (see below) |
| `*` | magenta `*` when the working tree has uncommitted changes |
| `+/-` | commits ahead / commits behind upstream |
| `last commit` | committer date of `HEAD` |
| `Σ` | total commits on the current branch |

states:

- **synced** (green) — matches upstream, nothing to do
- **pull** (yellow) — behind upstream, run `git pull`
- **push** (red) — ahead of upstream, run `git push`
- **diverged** (yellow) — both ahead and behind; merge or rebase required
- **no-upstream** (cyan) — branch has no remote tracking
- **not-repo** (default) — directory isn't a git repository

dirty (the `*`) is tracked independently of state. a repo can be `synced *`
(in sync with origin but with local edits) or `pull *` (behind upstream
*and* dirty), etc.

## how it works

per repo, in parallel across a thread pool (default 16 workers):

1. `git fetch --quiet origin` — unless `--no-fetch`, the cache entry is
   fresh, or the remote is unreachable. credential prompts are disabled
   (`GIT_TERMINAL_PROMPT=0`) and network timeouts are tight (~10s) so a dead
   remote can't hang the dashboard.
2. local `git rev-parse` / `status --porcelain` / `rev-list --left-right` to
   compute branch, dirtiness, and ahead/behind.
3. `git log -1` + `git rev-list --count` for the last-commit date and
   total-commit columns.

### fetch cache

each successful fetch touches a zero-byte marker file at
`~/.cache/talos/<repo>.fetch`. on the next run, if the marker's mtime is
younger than `--fetch-ttl` (default 60s), the fetch is skipped. clear it
with `rm -rf ~/.cache/talos` or override with `--force-fetch`.


