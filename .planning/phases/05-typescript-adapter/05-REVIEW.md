---
phase: 05-typescript-adapter
reviewed: 2026-04-30T06:52:11Z
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
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 5: Code Review Report

**Reviewed:** 2026-04-30T06:52:11Z
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Reviewed the Phase 5 TypeScript adapter, core fact exposure, CLI integration tests, and TS fixtures/examples. The adapter is broadly integrated and the targeted tests pass, but there are three correctness gaps in extracted TS facts that can produce false negatives for literal policies, export-aware rules, and call graph consumers.

Verification run:

- `cargo test -p polint-ts` passed.
- `cargo test -p polint-cli --test cli check_ts` passed.
- A manual CLI repro with only `<button data-color="#00ff00">` returned `[]`, confirming WR-01.

## Warnings

### WR-01: Quoted JSX Attribute Literals Bypass Literal Rules

**File:** `crates/polint-ts/src/lib.rs:1987`
**Issue:** `walk_jsx_attribute_value_for_literals` ignores `JSXAttributeValue::StringLiteral`. Built-in literal policies such as `examples/ts-no-raw-colors` and `examples/config-query-no-literal` consume `ctx.string_literals()`, so JSX like `<button data-color="#00ff00" />` is missed unless another raw string exists elsewhere in the file.
**Fix:**
```rust
fn walk_jsx_attribute_value_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    value: &JSXAttributeValue<'_>,
) {
    match value {
        JSXAttributeValue::ExpressionContainer(container) => {
            walk_jsx_expression_for_literals(db, ctx, &container.expression);
        }
        JSXAttributeValue::Element(element) => extract_jsx_element_attributes(db, ctx, element),
        JSXAttributeValue::Fragment(fragment) => walk_jsx_fragment_for_literals(db, ctx, fragment),
        JSXAttributeValue::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
    }
}
```

### WR-02: Export Specifier Declarations Are Not Marked Exported

**File:** `crates/polint-ts/src/lib.rs:242-245`
**Issue:** `ExportNamedDeclaration` only propagates `is_exported: true` when it contains an inline declaration, so common TS patterns like `const Button = () => null; export { Button };` leave the `FunctionFact` or `TsClassFact` marked as not exported. Rules using the exposed `is_exported` field will see incorrect SDK facts for valid exported APIs.
**Fix:** Collect local names from export specifiers before extracting declarations, then apply that set when pushing matching functions/classes.
```rust
let exported_names = exported_local_names(program);

// When pushing top-level declarations:
let is_exported = exported_names.contains(name.as_str());
```

### WR-03: Nested Calls In Normal Arguments Are Omitted

**File:** `crates/polint-ts/src/lib.rs:1275-1279`
**Issue:** `collect_calls_from_expression` delegates call arguments to `collect_calls_from_argument`, but that helper only recurses into spread arguments. Calls inside ordinary arguments, such as `outer(inner())` or `track(String(error))`, are absent from `FunctionFact.calls`, which makes `polint-graph` call edges incomplete.
**Fix:** Expand `collect_calls_from_argument` to mirror the expression recursion used by the complexity and literal walkers, including call, conditional, logical, array, object, JSX, and TS wrapper argument variants.
```rust
fn collect_calls_from_argument(argument: &Argument<'_>, calls: &mut Vec<String>) {
    match argument {
        Argument::SpreadElement(spread) => collect_calls_from_expression(&spread.argument, calls),
        Argument::CallExpression(call) => {
            if let Some(name) = callee_text(&call.callee) {
                calls.push(name);
            }
            for argument in &call.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        Argument::ConditionalExpression(expression) => {
            collect_calls_from_expression(&expression.test, calls);
            collect_calls_from_expression(&expression.consequent, calls);
            collect_calls_from_expression(&expression.alternate, calls);
        }
        _ => {}
    }
}
```

---

_Reviewed: 2026-04-30T06:52:11Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
