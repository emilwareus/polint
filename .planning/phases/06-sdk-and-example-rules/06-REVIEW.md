---
phase: 06-sdk-and-example-rules
reviewed: 2026-05-01T06:25:17Z
depth: standard
files_reviewed: 14
files_reviewed_list:
  - crates/polint-cli/src/main.rs
  - crates/polint-cli/tests/cli.rs
  - crates/polint-config/src/lib.rs
  - crates/polint-core/src/lib.rs
  - crates/polint-go/src/lib.rs
  - crates/polint-rules/Cargo.toml
  - crates/polint-rules/src/lib.rs
  - crates/polint-rules/tests/snapshots.rs
  - crates/polint-sdk/src/lib.rs
  - crates/polint-ts/src/lib.rs
  - tests/fixtures/go/clean/payment_test.go
  - tests/fixtures/go/failing/payment.go
  - tests/fixtures/go/failing/payment_test.go
  - tests/fixtures/ts/failing/component.tsx
findings:
  critical: 1
  warning: 1
  info: 0
  total: 2
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-05-01T06:25:17Z
**Depth:** standard
**Files Reviewed:** 14
**Status:** issues_found

## Summary

Re-reviewed the Phase 6 SDK, built-in rule, parser fact, CLI, fixture, and test changes after the review fixes. `Cargo.lock` was listed in the input but excluded from the reviewed-file count by the code-review lockfile filter.

The two previously reported issues are resolved: JSX raw-color diagnostics now dedupe overlapping string/JSX facts, and Go import-boundaries, test-suite-size, and assertion-after-action now honor `files` and `allow_files`. The targeted regressions and package tests pass.

Two issues remain from the broader standard pass: `new-rule` can escape `.polint/rules` and overwrite unintended files, and three built-in rules still ignore `allow_files`.

## Critical Issues

### CR-01: `new-rule` accepts path traversal in rule names

**File:** `crates/polint-cli/src/main.rs:156`
**Issue:** `new_rule` joins `args.rule_name` directly into `.polint/rules` and then writes `Cargo.toml` and `src/lib.rs`. A value like `../..` resolves outside `.polint/rules` and can overwrite repository files such as `Cargo.toml`, creating a path traversal and data-loss risk. The same unsanitized value is also embedded into the generated Rust rule id.
**Fix:**
```rust
fn validate_rule_name(name: &str) -> Result<String> {
    let sanitized = sanitize_name(name);
    let path = Path::new(name);
    if name.is_empty()
        || sanitized != name
        || path.components().any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        anyhow::bail!("rule name must be a single safe path component");
    }
    Ok(sanitized)
}

fn new_rule(root: PathBuf, args: &NewRuleArgs) -> Result<()> {
    let rule_name = validate_rule_name(&args.rule_name)?;
    let rule_dir = root.join(".polint/rules").join(&rule_name);
    if rule_dir.exists() {
        anyhow::bail!("rule already exists: {}", rule_dir.display());
    }
    // use rule_name for the package name and generated rule id
    // ...
}
```

## Warnings

### WR-01: Some built-in rules ignore `allow_files`

**File:** `crates/polint-rules/src/lib.rs:51`
**Issue:** `examples/go-cyclomatic-complexity`, `examples/ts-cyclomatic-complexity`, and `examples/go-branch-obligations` still call `file_selected(...)` without checking `file_allowed(...)`. These rules honor positive `files` filters but still emit diagnostics for paths configured in `allow_files`, causing false positives when users suppress generated or exception files.
**Fix:** Use the shared scope helper everywhere rule-level file filters are applied, and add negative tests for `allow_files` on these rules.
```rust
let file = ctx.file_path(function.file);
if function.cyclomatic_complexity > max && file_in_rule_scope(ctx.options(), &file) {
    // report diagnostic
}

let file = ctx.file_path(branch.file);
if !file_in_rule_scope(ctx.options(), &file) {
    continue;
}
```

## Verification

- `cargo test -p polint-cli check_ts_no_raw_colors_dedupes_real_jsx_attribute_literal`
- `cargo test -p polint-rules ts_raw_colors_dedupes_string_and_jsx_attribute_facts`
- `cargo test -p polint-rules file_filters`
- `cargo test -p polint-rules go_import_boundary_respects_rule_file_filters`
- `cargo test -p polint-rules -p polint-cli`
- `cargo fmt --check`

---

_Reviewed: 2026-05-01T06:25:17Z_
_Reviewer: Codex (gsd-code-reviewer)_
_Depth: standard_
