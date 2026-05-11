# Quick Task 260511-i7m Summary

**Task:** Make the baseline file live only at `.polint/baseline.yaml` and remove user-selectable baseline paths
**Date:** 2026-05-11
**Status:** Complete

## Changes

- Made `.polint/baseline.yaml` the only baseline file path.
- Removed `baseline create --output` and `baseline update --baseline`.
- Changed `polint check --baseline` to a boolean flag that always reads `.polint/baseline.yaml`.
- Updated README, agent playbook, generated skill text, and the checked-in Claude skill examples.
- Updated CLI tests for canonical-path create/check/update/ignore behavior and rejection of removed path flags.

## Verification

- `cargo fmt --all`
- `cargo test -p polint baseline --locked`
- `cargo check -p polint --locked`
- `cargo test -p polint --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `git diff --check`
