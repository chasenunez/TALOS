# TALOS
a terminal-based repository sentinal, written in Rust

## Tech stack

| Concern | Crate | Why |
|---|---|---|
| TUI | `ratatui` + `crossterm` | de facto standard pair; gives Table widget, event loop, SIGWINCH, and double-buffered render for free |
| Git ops | shell out via `std::process::Command` to `git` | matches bash semantics exactly; auth/SSH "just works" using the user's existing config. `git2` (libgit2) would be ~2x faster but adds a C dep and credential callbacks that break for SSH agent users |
| Concurrency | `std::thread` + `mpsc` channel | parallel fetch fan-out is the only concurrency we need; `tokio` is overkill for ~45 subprocess calls |
| Cache | `directories` + plain files in `$XDG_CACHE_HOME/talos/` | per-repo last-fetch timestamps so refresh ticks can be tight without re-fetching |
| Errors | `anyhow` | one-file binary, no need for typed error hierarchy |
| Args | `clap` (derive) | target dir, fetch TTL, refresh interval flags |

## Module layout

```
talos/
├── Cargo.toml
└── src/
    └── main.rs        # App state, scan, fetch pool, ratatui render, event loop
```