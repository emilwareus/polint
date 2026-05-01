---
phase: 06-sdk-and-example-rules
fixed_at: 2026-05-01T06:20:58Z
review_path: /Users/emilwareus/Development/exlint/.planning/phases/06-sdk-and-example-rules/06-REVIEW.md
iteration: 1
findings_in_scope: 2
fixed: 2
skipped: 0
status: all_fixed
---

# Phase 06: Code Review Fix Report

**Fixed at:** 2026-05-01T06:20:58Z
**Source review:** /Users/emilwareus/Development/exlint/.planning/phases/06-sdk-and-example-rules/06-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 2
- Fixed: 2
- Skipped: 0

## Fixed Issues

### WR-01: JSX raw-color attributes can be reported twice

**Status:** fixed: requires human verification
**Files modified:** `crates/polint-rules/src/lib.rs`, `crates/polint-cli/tests/cli.rs`
**Commit:** dd51952
**Applied fix:** Replaced exact-span raw-color dedupe with overlap-aware same-file/same-literal dedupe and added regressions for overlapping SDK facts plus a real parser-backed TSX JSX attribute.

### WR-02: Some Go rules ignore configured file filters

**Status:** fixed: requires human verification
**Files modified:** `crates/polint-rules/src/lib.rs`
**Commit:** b7a93b1
**Applied fix:** Added a shared rule-scope file check for Go import boundaries, Go test suite size, and Go assertion-after-action, with regressions for non-matching `files` and matching `allow_files` filters.

## Verification

- `cargo test -p polint-rules ts_raw_colors_dedupes_string_and_jsx_attribute_facts`
- `cargo test -p polint-cli check_ts_no_raw_colors_dedupes_real_jsx_attribute_literal`
- `cargo test -p polint-rules file_filters`
- `cargo fmt --check`
- `cargo test -p polint-rules -p polint-cli`

---

_Fixed: 2026-05-01T06:20:58Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
