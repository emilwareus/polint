# Quick Task 260505-ffu Summary

**Date:** 2026-05-05
**Status:** Complete

## Completed

- Changed `polint check` to discover repo-local Cargo rule hosts under
  configured `rules.paths`.
- Added internal delegation so the user runs `polint check` while the CLI runs
  local Rust rule hosts behind the scenes, collects JSON diagnostics, and
  renders normal human/JSON/SARIF-like output.
- Added positional path support to `polint check`, including local rule-host
  execution.
- Updated `polint-runner` so local rule hosts honor positional paths too.
- Fixed missing `[rules]` config defaults so `.polint/rules` is discovered even
  when a project config omits the section.
- Removed implicit `fast`/`full` profiles. No selected profile now means all
  discovered rules run; a selected profile must be defined exactly in
  `.polint.toml`.
- Changed `polint new-rule` to scaffold runnable `src/main.rs` rule hosts that
  register through `polint_runner::run_cli`.
- Updated root and example README commands to use `polint check` instead of
  user-facing `cargo run --manifest-path ...`.
- Added a CLI regression test that rejects `polint check --config` as an
  unknown flag.

## Verification

- `cargo test -p polint-cli`
- `cargo test -p polint-config`
- source install into a temporary `CARGO_INSTALL_ROOT`
- installed `polint check` against `examples/config-denied-literal`
- `cargo fmt -- --check`
- `git diff --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
