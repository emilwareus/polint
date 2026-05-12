# Quick Task 260511-gyu: Compact YAML Baseline And Central Ignore Ratchet - Summary

**Date:** 2026-05-11
**Status:** Complete

## Implemented

- Added a crate-private baseline engine for compact `.polint-baseline.yaml`
  files with `version`, `baseline`, and `ignore` string arrays.
- Added `polint baseline create`, `polint baseline update`, `polint check
  --baseline`, and `polint check --baseline --new-only`.
- Applied central `ignore` entries as suppressions and `baseline` entries as
  non-failing existing debt, with human summaries for new/existing/fixed/ignored
  and stale-path counts.
- Documented the workflow in README, the agent playbook, generated skill text,
  and the checked-in Claude skill.
- Added unit and temp-repo CLI tests for create, new-only ratcheting, central
  ignore suppression, update pruning, malformed entries, and `--new-only`
  validation.

## Verification

- `cargo fmt --all`
- `cargo test -p polint baseline --locked`
- `cargo check -p polint --locked`
- `cargo test -p polint --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
