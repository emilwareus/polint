# Quick Task 260505-e2y Summary

**Date:** 2026-05-05
**Status:** Complete

## Completed

- Added a README "Try to use it!" section.
- Documented the private install one-liner, repository clone command, example
  directory, exact example rule command, and expected output.
- Clarified that the example command runs its local rule host because polint
  ships no built-in policy rules.

## Verification

- Ran the documented install flow in a temporary directory with
  `POLINT_INSTALL_DIR` pointed at a temp bin path.
- Cloned `emilwareus/exlint` into the temp directory.
- Ran `polint --version`.
- Ran `cargo run --quiet --manifest-path .polint/rules/no-denied-literals/Cargo.toml -- check --profile fast --fail-on none` from
  `examples/config-denied-literal`.
