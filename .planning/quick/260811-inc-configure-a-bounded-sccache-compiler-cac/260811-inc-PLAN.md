---
quick_id: 260811-inc
task: configure-a-bounded-sccache-compiler-cache
status: completed
---

# Plan

Configure bounded Rust compiler caching that is shared safely across Git
worktrees without sharing their raw Cargo target directories.

## Tasks

1. Disable per-worktree incremental compilation and set a 10 GiB sccache cap.
2. Document the portable contributor setup and why target directories remain
   worktree-local.
3. Install and configure sccache for the current user so existing worktrees use
   the shared cache immediately.
4. Verify Cargo configuration, cache bounds, compiler wrapping, and disk usage.
