---
phase: 06-sdk-and-example-rules
fixed_at: 2026-05-01T06:39:36Z
review_path: /Users/emilwareus/Development/exlint/.planning/phases/06-sdk-and-example-rules/06-REVIEW.md
iteration: 3
findings_in_scope: 1
fixed: 1
skipped: 0
status: all_fixed
---

# Phase 06: Code Review Fix Report

**Fixed at:** 2026-05-01T06:39:36Z
**Source review:** /Users/emilwareus/Development/exlint/.planning/phases/06-sdk-and-example-rules/06-REVIEW.md
**Iteration:** 3

**Summary:**
- Findings in scope: 1
- Fixed: 1
- Skipped: 0

## Fixed Issues

### CR-01: `new-rule` overwrites existing rule files

**Status:** fixed
**Files modified:** `crates/polint-cli/src/main.rs`, `crates/polint-cli/tests/cli.rs`
**Commit:** 610fe6d
**Applied fix:** Changed `polint new-rule` to validate the safe rule name, inspect `.polint/rules/<rule_name>` with `symlink_metadata`, and fail before writing generated files when the rule path already exists. The scaffold flow now creates `.polint/rules` first and then uses exclusive `create_dir` calls for the rule directory and `src` directory before writing `Cargo.toml` or `src/lib.rs`. Added a CLI regression proving rerunning `polint new-rule go demo` against an existing rule fails and preserves sentinel `Cargo.toml` and `src/lib.rs` contents.

## Verification

- `cargo test -p polint-cli new_rule_rejects_existing_rule_without_overwriting_files`
- `cargo test -p polint-cli new_rule`
- `cargo fmt --check`
- `cargo test -p polint-cli`
- `cargo test --workspace`

---

_Fixed: 2026-05-01T06:39:36Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 3_
