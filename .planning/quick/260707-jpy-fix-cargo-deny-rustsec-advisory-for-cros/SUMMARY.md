---
status: complete
---

# Summary: Fix cargo-deny RustSec Advisory for crossbeam-epoch

The CI failure was caused by `crossbeam-epoch 0.9.18`, pulled transitively
through `crossbeam-deque` by `ignore` and `rayon`, matching
`RUSTSEC-2026-0204`. Updating the lockfile resolved it to `crossbeam-epoch
0.9.20`.

Local cargo-deny then surfaced `RUSTSEC-2026-0190` for direct dependency
`anyhow 1.0.102`, so the workspace dependency floor and lockfile were updated
to `anyhow 1.0.103`.

Verification:
- `cargo deny check advisories licenses bans sources`
- `cargo check --workspace --all-targets`
- `git diff --check`
