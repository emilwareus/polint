---
phase: 06-sdk-and-example-rules
reviewed: 2026-05-01T06:35:50Z
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
  warning: 0
  info: 0
  total: 1
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-05-01T06:35:50Z
**Depth:** standard
**Files Reviewed:** 14
**Status:** issues_found

## Summary

Re-reviewed the Phase 6 CLI, SDK, parser adapters, built-in rules, rule tests, and fixtures after the code-review fixes.

The prior findings are resolved: TS raw-color diagnostics dedupe overlapping string and JSX attribute facts, the focused built-in rules now honor `files` and `allow_files`, and `polint new-rule` rejects path traversal and unsafe rule names before writing generated files.

One scaffold data-loss issue remains: `polint new-rule` still overwrites an existing safe rule directory.

## Critical Issues

### CR-01: `new-rule` overwrites existing rule files

**File:** `crates/polint-cli/src/main.rs:159`
**Issue:** After validating a safe rule name, `new_rule` calls `fs::create_dir_all(&src_dir)` and then unconditionally writes `Cargo.toml` and `src/lib.rs`. Re-running `polint new-rule go demo` for an existing `.polint/rules/demo` succeeds and replaces the existing rule implementation, which can destroy repo-local rule code. I verified this in a temporary directory: a sentinel `src/lib.rs` was replaced with the generated SDK template.
**Fix:**
```rust
fn new_rule(root: PathBuf, args: &NewRuleArgs) -> Result<()> {
    let rule_name = validate_rule_name(&args.rule_name)?;
    let rules_dir = root.join(".polint/rules");
    let rule_dir = rules_dir.join(&rule_name);

    match fs::symlink_metadata(&rule_dir) {
        Ok(_) => anyhow::bail!("rule already exists: {}", rule_dir.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| {
            format!("failed to inspect {}", rule_dir.display())
        }),
    }

    fs::create_dir_all(&rules_dir)?;
    fs::create_dir(&rule_dir)?;
    let src_dir = rule_dir.join("src");
    fs::create_dir(&src_dir)?;

    // write Cargo.toml and src/lib.rs after the exclusive directory creation
    // succeeds.
    Ok(())
}
```

## Verification

- `cargo test -p polint-cli new_rule_rejects_unsafe_rule_names_without_writing_outside_rules_dir`
- `cargo test -p polint-cli check_ts_no_raw_colors_dedupes_real_jsx_attribute_literal`
- `cargo test -p polint-rules ts_raw_colors_dedupes_string_and_jsx_attribute_facts`
- `cargo test -p polint-rules respects`
- `cargo fmt --check`

---

_Reviewed: 2026-05-01T06:35:50Z_
_Reviewer: Codex (gsd-code-reviewer)_
_Depth: standard_
