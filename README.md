# talos

a terminal dashboard for a directory full of git repos. it tells you which
ones need pulling, which ones need pushing, which ones have uncommitted work,
and which ones aren't actually git repos at all.

originally a 350-line bash script (still living at `../talos.sh`); this is
the rust rewrite. the bash version had a small but load-bearing bug where it
never actually ran `git fetch`, so the "pull" column was silently always
zero. that's fixed. it's also about 7× faster.

part of the [GAIA](../README.md) collection.

## install

needs a rust toolchain. from this directory:

```sh
cargo build --release
```

the binary lands at `target/release/talos`. drop it on your PATH (`cp
target/release/talos ~/.local/bin/`) or call it directly.

## use

```sh
talos                       # scan ~/PANTHEON, fetch + status
talos --target ~/code       # scan somewhere else
talos --no-fetch            # skip the network round-trip (fast, stale data)
talos --force-fetch         # ignore the fetch cache and refetch everything
talos --fetch-ttl 300       # only refetch a repo if last fetch was >5min ago
talos -j 4                  # fewer parallel workers (default 16)
```

example output:

```
╺┳╸┏━┓╻  ┏━┓┏━┓
 ┃ ┣━┫┃  ┃ ┃┗━┓
 ╹ ╹ ╹┗━╸┗━┛┗━┛

Pull: 2/44  Push: 0/44  Dirty: 3/44  No upstream: 0/44

PATH: /Users/you/PANTHEON

#    repo               state       *  +/-    last commit       Σ
---- ------------------ ----------- -  ------ ----------- -------
1    AEMLAP             synced      *  0/0    08/06/26          2
2    APOLLO             synced         0/0    20/01/26          5
...
22   bookbot            pull           0/1    29/04/26         17
23   chasenunez         pull           0/44   29/04/26        922
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
*and* dirty), etc. the bash original collapsed dirty into state, so a dirty
repo silently hid the fact that it was also behind upstream.

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

the bash version did all of this serially with broken fetching. the rust
version's parallel pool plus cache turns a real ~25s refresh into ~1–3s
depending on cache state.

### fetch cache

each successful fetch touches a zero-byte marker file at
`~/.cache/talos/<repo>.fetch`. on the next run, if the marker's mtime is
younger than `--fetch-ttl` (default 60s), the fetch is skipped. clear it
with `rm -rf ~/.cache/talos` or override with `--force-fetch`.

this matters once you start running talos on a tight refresh loop — the TUI
version (coming soon) ticks every few seconds and would otherwise hammer
every remote constantly.
