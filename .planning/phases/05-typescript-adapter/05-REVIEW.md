---
phase: 05-typescript-adapter
reviewed: 2026-04-30T07:00:41Z
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

**Reviewed:** 2026-04-30T07:00:41Z
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Re-reviewed the Phase 5 TypeScript adapter, core fact exposure, CLI integration tests, and TS fixtures/examples after the fixes in `05-REVIEW-FIX.md`. The three prior warnings are closed:

- quoted JSX attributes are now emitted as `StringLiteralFact` values and covered by `quoted_jsx_attributes_are_available_as_string_literals`;
- named export specifiers now mark local function/class facts as exported and are covered by `export_specifiers_mark_ts_facts_as_exported`;
- nested calls inside ordinary arguments are now collected and covered by `nested_calls_inside_regular_arguments_are_collected`.

Verification run:

- `cargo test -p polint-ts --lib` passed, 18 tests.
- `cargo test -p polint-cli --test cli check_ts` passed, 2 tests.
- `cargo test -p polint-core --lib` passed, 17 tests.

The remaining issues are new/adjacent correctness gaps in TypeScript/JavaScript fact extraction.

## Warnings

### WR-01: Referenced Default Exports Are Not Marked Exported

**File:** `crates/polint-ts/src/lib.rs:415`
**Issue:** `exported_local_names` only collects `ExportNamedDeclaration` specifiers, while `extract_declarations` only marks inline default declarations (`export default function Button() {}` / `export default class Button {}`) as exported. Common patterns such as `const Button = () => null; export default Button;` leave the `FunctionFact` or `TsClassFact` with `is_exported: false`, so export-aware rules get incorrect facts for a valid public API.
**Fix:**
```rust
fn exported_local_names(program: &Program<'_>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for statement in &program.body {
        match statement {
            Statement::ExportNamedDeclaration(export) if export.source.is_none() => {
                for specifier in &export.specifiers {
                    if matches!(specifier.export_kind, ImportOrExportKind::Value)
                        && let Some(name) = module_export_name_text(&specifier.local)
                    {
                        names.insert(name);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::Identifier(identifier) = &export.declaration {
                    names.insert(identifier.name.to_string());
                }
            }
            _ => {}
        }
    }

    names
}
```

### WR-02: Call Collection Still Skips Calls Inside Expression Containers

**File:** `crates/polint-ts/src/lib.rs:680`
**Issue:** `collect_calls_from_argument` was expanded, but the central `collect_calls_from_expression` walker still drops several expression containers, including arrays, objects, `new`, JSX, tagged templates, and `await`. As a result, calls in normal function bodies such as `const values = [load()]`, `return { value: format() }`, `return <View label={format()} />`, or `new Client(createConfig())` are omitted from `FunctionFact.calls`, leaving `polint-graph` call edges incomplete.
**Fix:** Mirror the expression coverage already present in the complexity/literal walkers instead of relying on argument-only recursion.
```rust
fn collect_calls_from_expression(expression: &Expression<'_>, calls: &mut Vec<String>) {
    match expression {
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                collect_calls_from_array_element(element, calls);
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        collect_calls_from_property_key(&property.key, calls);
                        collect_calls_from_expression(&property.value, calls);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_calls_from_expression(&spread.argument, calls);
                    }
                }
            }
        }
        Expression::NewExpression(expression) => {
            collect_calls_from_expression(&expression.callee, calls);
            for argument in &expression.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        Expression::JSXElement(element) => collect_calls_from_jsx_element(element, calls),
        Expression::JSXFragment(fragment) => collect_calls_from_jsx_fragment(fragment, calls),
        _ => {
            // keep the existing call, conditional, logical, wrapper, and member handling
        }
    }
}
```

### WR-03: CommonJS Require Imports No Longer Produce Import Facts

**File:** `crates/polint-ts/src/lib.rs:142`
**Issue:** The previous TS adapter detected `require("...")` while extracting imports. The Oxc-based extractor now only handles `import` declarations and `export ... from` statements, and the module-record fallback only runs when no AST imports were emitted. JavaScript/CommonJS files such as `const config = require("./config")` therefore lose `ImportFact` coverage, which makes `ImportGraph::from_db` incomplete for supported `.js` files.
**Fix:** Add a regression test for CommonJS imports, then either walk call expressions for a `require` callee with a string-literal first argument or handle Oxc's TS import-equals form explicitly.
```rust
// During expression traversal:
if let Expression::CallExpression(call) = expression
    && callee_text(&call.callee).as_deref() == Some("require")
    && let Some(Argument::StringLiteral(path)) = call.arguments.first()
{
    push_module_import(
        db,
        file,
        path.value.as_str(),
        span_from_oxc(file, source, path.span),
        language,
    );
}
```

---

_Reviewed: 2026-04-30T07:00:41Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
