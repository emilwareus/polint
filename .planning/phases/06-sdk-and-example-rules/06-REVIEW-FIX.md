---
phase: 06-sdk-and-example-rules
fixed_at: 2026-05-01T06:31:01Z
review_path: /Users/emilwareus/Development/exlint/.planning/phases/06-sdk-and-example-rules/06-REVIEW.md
iteration: 2
findings_in_scope: 2
fixed: 2
skipped: 0
status: all_fixed
---

# Phase 06: Code Review Fix Report

**Fixed at:** 2026-05-01T06:31:01Z
**Source review:** /Users/emilwareus/Development/exlint/.planning/phases/06-sdk-and-example-rules/06-REVIEW.md
**Iteration:** 2

**Summary:**
- Findings in scope: 2
- Fixed: 2
- Skipped: 0

## Fixed Issues

### CR-01: `new-rule` accepts path traversal in rule names

**Status:** fixed
**Files modified:** `crates/polint-cli/src/main.rs`, `crates/polint-cli/tests/cli.rs`
**Commit:** b14f049
**Applied fix:** Added rule-name validation before any scaffold directories are created, rejecting empty names, path components, slashes, and names that would sanitize differently. Added a CLI regression that verifies traversal and unsafe names fail without writing `Cargo.toml`, `src/lib.rs`, nested rule directories, or unsanitized rule directories outside the intended `.polint/rules/<name>` layout.

### WR-01: Some built-in rules ignore `allow_files`

**Status:** fixed
**Files modified:** `crates/polint-rules/src/lib.rs`
**Commit:** d190784
**Applied fix:** Switched Go cyclomatic complexity, TS cyclomatic complexity, and Go branch obligations to the shared `file_in_rule_scope` helper so `allow_files` suppresses diagnostics after positive `files` matching. Added focused regressions covering both non-matching `files` and matching `allow_files` for all three rules.

## Verification

- `cargo test -p polint-cli new_rule_rejects_unsafe_rule_names_without_writing_outside_rules_dir`
- `cargo test -p polint-rules respects_files_and_allow_files`
- `cargo test -p polint-cli -p polint-rules`
- `cargo fmt --check`
- `cargo test --workspace`

---

_Fixed: 2026-05-01T06:31:01Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 2_
