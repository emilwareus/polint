# Function Facts

`FunctionFact` describes a function-like declaration found by the Go or TS/JS
adapter. Rules read these through `RuleCtx::functions()` or
`RuleCtx::functions_for_file(file_id)`.

## Fields

| Field | Meaning |
|-------|---------|
| `id` | Stable ID for this analysis run. Use it to relate function-owned facts such as branch obligations. |
| `file` | Stable `FileId` for the source file. |
| `name` | Extracted function, method, arrow/function variable, or method name. Go methods are normalized as `Receiver.Method`. |
| `span` | Source span for the function declaration or best available function syntax. |
| `language` | Source language family. |
| `is_test` | `true` for Go top-level test-like functions harvested from `_test.go` files. |
| `is_exported` | Syntax-level exported/public signal where available. |
| `cyclomatic_complexity` | Heuristic syntax-level complexity count. It does not use type or semantic analysis. |
| `calls` | Direct syntactic call names found in the function body. This is not a resolved call graph. |

## Limits

- Calls are names from syntax, not resolved symbols.
- Complexity is syntax-level and intentionally conservative.
- Invalid parse trees may reduce or omit facts for a file.
- TS/JS facts do not use TypeScript type checking.

## Small Rule Shape

```rust
use polint::sdk::prelude::*;

fn check_complexity(ctx: &mut RuleCtx<'_>, max: u32) {
    let mut diagnostics = Vec::new();
    for function in ctx.functions() {
        let file = ctx.file_path(function.file);
        if file_in_scope(ctx.options(), &file) && function.cyclomatic_complexity > max {
            diagnostics.push(
                Diagnostic::warning(
                    "local/high-complexity",
                    file,
                    function.span.diagnostic_range(),
                    "Function is over the configured complexity threshold.",
                )
                .with_evidence("function", function.name.clone())
                .with_evidence("complexity", function.cyclomatic_complexity.to_string()),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
}
```
