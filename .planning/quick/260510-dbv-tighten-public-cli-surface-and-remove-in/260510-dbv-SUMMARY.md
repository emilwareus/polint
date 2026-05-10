# Quick Task 260510-dbv Summary

**Task:** Tighten public CLI surface and remove internal debug commands
**Date:** 2026-05-10
**Status:** Complete

## Changes

- Added `AGENTS.md` guidance that visible commands, flags, output formats,
  generated skill text, README workflows, and example commands are public
  contracts.
- Removed top-level `polint explain`, `polint test-rules`,
  `polint profile-rules`, and `polint graph` commands.
- Removed the unused rule-host `explain` path as well, so generated rule hosts
  expose only `check`.
- Gated the graph helper module to tests so graph internals do not compile into
  normal builds.
- Updated generated skill text, docs, roadmap notes, examples, and CLI tests to
  use supported workflows only.
- Marked `docs/INITIAL_PROMPT.md` as a historical archive so proposed commands
  there are not mistaken for current product behavior.

## Verification

- `cargo fmt --all`
- `cargo check -p polint --locked`
- `cargo test -p polint --lib --locked`
- `cargo test -p polint --test cli --locked`
- `cargo run -q -p polint -- --help`
- `cargo run -q -p polint -- graph --help` exits with unknown subcommand
- `cargo run -q -p polint -- explain --help` exits with unknown subcommand
- `cargo run --quiet --manifest-path examples/multiple-rules/.polint/rules/Cargo.toml -- --help`
