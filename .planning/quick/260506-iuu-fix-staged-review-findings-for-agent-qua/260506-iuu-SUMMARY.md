# Quick Task 260506-iuu Summary

**Date:** 2026-05-06
**Status:** Complete

## Completed

- Moved SARIF tags under `reportingDescriptor.properties.tags`.
- Split diagnostic filtering from render-only truncation so `--fail-on` sees the
  full filtered diagnostic set.
- Stopped forwarding `--max-diagnostics` to repo-local rule hosts, preserving
  complete child JSON for parent exit-code decisions.
- Fixed the SARIF CI job binary path from the example workspace and added
  assertions for the SARIF version and tag placement.
- Updated generated `add-skill` content and tests so newly installed skills
  include the report schema and `explain go-test` guidance.
- Tightened `LoadSourcesTimings::total` to bench builds, unblocking lib tests.

## Verification

- `cargo fmt --all`
- `cargo test -p polint --lib --locked`
- `cargo test -p polint --test cli --locked check_max_diagnostics -- --nocapture`
- `cargo test -p polint --test cli --locked add_skill_installs_claude_skill_non_interactively -- --nocapture`
- `cargo test -p polint --test cli --locked`
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked`
- `cargo test --workspace --all-features --locked`
- Local SARIF command/shape check using `../../target/debug/polint` from
  `examples/ts-design-tokens`.
