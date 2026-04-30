---
phase: 05-typescript-adapter
source_review: .planning/phases/05-typescript-adapter/05-REVIEW.md
status: fixed
fixed: 7
remaining: 0
completed: 2026-04-30
---

# Phase 5 Code Review Fix Summary

## Fixes Applied

### First Review Pass

1. **WR-01: Quoted JSX Attribute Literals Bypass Literal Rules**
   - Added a regression test proving `<button data-color="#00ff00" />` creates a `StringLiteralFact`.
   - Updated `walk_jsx_attribute_value_for_literals` so quoted JSX attributes are also pushed as string literals.

2. **WR-02: Export Specifier Declarations Are Not Marked Exported**
   - Added a regression test for `const Button = () => null; class Panel {}; export { Button, Panel };`.
   - Added program-level exported-name collection and applied it to top-level function, variable, class, arrow, function-expression, and class-expression facts.

3. **WR-03: Nested Calls In Normal Arguments Are Omitted**
   - Added a regression test for `outer(inner())` and `track(String(error))`.
   - Expanded call collection through ordinary arguments, arrays, objects, conditionals, logical expressions, JSX containers, and TS wrapper expressions.

### Second Review Pass

4. **WR-01: Referenced Default Exports Are Not Marked Exported**
   - Added a regression test for `const Button = () => null; export default Button;` and `class Panel {}; export default Panel;`.
   - Extended exported-name collection to include identifier default exports.

5. **WR-02: Call Collection Still Skips Calls Inside Expression Containers**
   - Added a regression test covering calls in arrays, objects, `new` arguments, and JSX expression containers.
   - Expanded central expression call collection to recurse through those containers.

6. **WR-03: CommonJS Require Imports No Longer Produce Import Facts**
   - Added a regression test for `const config = require("./config")` in `.js` source.
   - Added parser-backed CommonJS `require("...")` extraction into `ImportFact`.

### Third Review Pass

7. **WR-01: Body-Scoped CommonJS Requires Are Still Skipped**
   - Added a regression test covering `require(...)` calls inside function, arrow function, class method, class field, and static block bodies.
   - Extended CommonJS require traversal through function and class bodies, default-exported declarations, expression-bodied arrows, class expressions, and class heritage.

## Commits

- `9bfd5d2` test(05-review): cover TS fact review gaps
- `2242292` fix(05-review): close TS adapter fact gaps
- `75ee5ba` test(05-review): cover remaining TS adapter gaps
- `80f0758` fix(05-review): close remaining TS adapter gaps
- `de20315` test(05-review): cover body scoped require imports
- `acdfa28` fix(05-review): collect body scoped require imports

## Verification

- `cargo test -p polint-ts --lib quoted_jsx_attributes_are_available_as_string_literals` - passed
- `cargo test -p polint-ts --lib export_specifiers_mark_ts_facts_as_exported` - passed
- `cargo test -p polint-ts --lib nested_calls_inside_regular_arguments_are_collected` - passed
- `cargo test -p polint-ts --lib referenced_default_exports_mark_ts_facts_as_exported` - passed
- `cargo test -p polint-ts --lib calls_inside_expression_containers_are_collected` - passed
- `cargo test -p polint-ts --lib commonjs_require_calls_emit_import_facts` - passed
- `cargo test -p polint-ts --lib commonjs_require_calls_inside_function_and_class_bodies_emit_import_facts` - passed
- `cargo fmt -- --check` - passed
- `cargo test -p polint-ts --lib` - passed, 22 tests
- `cargo test -p polint-cli --test cli check_ts` - passed, 2 tests
- `cargo clippy -p polint-ts --all-targets -- -D warnings` - passed

## Notes

- Fixes are limited to `crates/polint-ts/src/lib.rs`.
- No production resolver, CSS parser, graph command, or SARIF behavior was added.
