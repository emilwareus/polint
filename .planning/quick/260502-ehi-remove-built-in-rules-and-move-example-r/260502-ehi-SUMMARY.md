# Quick Task 260502-ehi Summary

**Task:** Remove built-in rules and move example policies into examples
**Date:** 2026-05-02
**Status:** Complete
**Commit:** this commit

## Changes

- Removed the shipped `polint` CLI dependency on the former rule crate and made `polint check`, `explain`, `profile-rules`, and `test-rules` behave truthfully when no policy rules are registered.
- Moved the old example policy rules from `crates/polint-rules` to `examples/rules` as `polint-example-rules`.
- Added an example runner binary so checked-in examples can execute real SDK rules without making those rules part of the product CLI.
- Updated default config generation to use empty profiles by default.
- Updated README and example READMEs so examples run through `examples/rules` and the docs state that polint ships no built-in policy rules.
- Updated CLI and example-rule tests to prove the product has no bundled policy diagnostics while the example fixtures still exercise the example policies end to end.

## Verification

- `cargo fmt --all`
- `cargo fmt --all -- --check`
- `cargo test -p polint-example-rules`
- `cargo test -p polint-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
