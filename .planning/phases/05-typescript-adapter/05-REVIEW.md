---
phase: 05-typescript-adapter
reviewed: 2026-04-30T07:08:11Z
depth: standard
files_reviewed: 8
files_reviewed_list:
  - crates/polint-core/src/lib.rs
  - crates/polint-ts/Cargo.toml
  - crates/polint-ts/src/lib.rs
  - crates/polint-cli/tests/cli.rs
  - tests/fixtures/ts/clean/component.tsx
  - tests/fixtures/ts/failing/component.tsx
  - tests/fixtures/mixed/view.ts
  - examples/ts-design-tokens/Button.tsx
findings:
  critical: 0
  warning: 1
  info: 0
  total: 1
status: issues_found
---

# Phase 5: Code Review Report

**Reviewed:** 2026-04-30T07:08:11Z
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Re-reviewed the Phase 5 TypeScript adapter, core fact exposure, CLI tests, and TS fixtures/examples after the fixes recorded in `05-REVIEW-FIX.md`.

Both prior warning batches are closed for their covered cases:

- quoted JSX attributes are now emitted as `StringLiteralFact` values and covered by `quoted_jsx_attributes_are_available_as_string_literals`;
- named export specifiers and referenced default exports now mark local function/class facts as exported and are covered by `export_specifiers_mark_ts_facts_as_exported` plus `referenced_default_exports_mark_ts_facts_as_exported`;
- nested calls in normal arguments and expression containers are now collected and covered by `nested_calls_inside_regular_arguments_are_collected` plus `calls_inside_expression_containers_are_collected`;
- simple top-level CommonJS `require("...")` calls now emit `ImportFact` values and are covered by `commonjs_require_calls_emit_import_facts`.

Verification run:

- `cargo test -p polint-ts --lib` passed, 21 tests.
- `cargo test -p polint-cli --test cli check_ts` passed, 2 tests.
- `cargo test -p polint-core --lib` passed, 17 tests.
- `cargo clippy -p polint-ts --all-targets -- -D warnings` passed.

One remaining adjacent correctness gap exists in the new CommonJS require traversal.

## Warnings

### WR-01: CommonJS Require Traversal Skips Function Bodies

**File:** `crates/polint-ts/src/lib.rs:294`
**Issue:** `collect_require_imports_from_statement` drops ordinary `FunctionDeclaration` and `ClassDeclaration` statements through the `_ => {}` arm, and `collect_require_imports_from_expression` also drops `ArrowFunctionExpression`, `FunctionExpression`, and `ClassExpression`. This means supported `.js`/`.ts` files such as `function load() { return require("./config"); }` or `const load = () => require("./config");` still produce no `ImportFact`, leaving `ImportGraph::from_db` incomplete for common lazy CommonJS dependencies.
**Fix:** Add a regression test for require calls inside function/arrow/class bodies, then reuse the existing declaration/body/class traversal shape used by the literal walker.
```rust
match statement {
    Statement::FunctionDeclaration(function) => {
        if let Some(body) = function.body.as_deref() {
            for statement in &body.statements {
                collect_require_imports_from_statement(db, ctx, statement);
            }
        }
    }
    Statement::ClassDeclaration(class) => {
        collect_require_imports_from_class(db, ctx, class);
    }
    _ => {}
}

match expression {
    Expression::ArrowFunctionExpression(function) => {
        for statement in &function.body.statements {
            collect_require_imports_from_statement(db, ctx, statement);
        }
    }
    Expression::FunctionExpression(function) => {
        if let Some(body) = function.body.as_deref() {
            for statement in &body.statements {
                collect_require_imports_from_statement(db, ctx, statement);
            }
        }
    }
    Expression::ClassExpression(class) => {
        collect_require_imports_from_class(db, ctx, class);
    }
    _ => {}
}
```

---

_Reviewed: 2026-04-30T07:08:11Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
