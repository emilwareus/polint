---
status: complete
quick_id: 260811-inc
completed: 2026-08-11
---

# Quick Task 260811-inc Summary

Configured bounded cross-worktree Rust compiler caching while keeping Cargo
target directories isolated per worktree.

## Changes

- Disabled Cargo incremental compilation for this repository so worktrees no
  longer accumulate large `target/debug/incremental` trees.
- Set the shared sccache maximum to 10 GiB with compiler fallback on cache-server
  I/O errors.
- Documented the contributor setup and the reason raw target directories are not
  shared.
- Installed sccache 0.17.0 and configured the current user's Cargo settings so
  all existing worktrees use the same cache immediately.

## Verification

- Two clean `cargo check -p polint-macros --lib --locked` builds used separate
  target directories; the second build reported six Rust cache hits.
- `sccache --show-stats` reported a 10 GiB maximum cache size and no cache read,
  write, or timeout errors.
- Both verification target directories contained only empty incremental
  directories (0 bytes) and were removed with `cargo clean` afterward.
- `git diff --check` passed.

## Commit

- `3b1e3193` — bound Rust build cache usage
