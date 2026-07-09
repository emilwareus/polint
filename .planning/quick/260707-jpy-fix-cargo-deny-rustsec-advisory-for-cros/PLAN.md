---
status: completed
---

# Quick Task: Fix cargo-deny RustSec Advisory for crossbeam-epoch

Goal: make the PR pass the `cargo deny (advisories + licenses + bans)` CI check.

Scope:
- Investigate the failing GitHub Actions job for PR #79.
- Update vulnerable dependency resolutions to patched versions.
- Run the same cargo-deny policy locally.
- Run a workspace compile sanity check.
