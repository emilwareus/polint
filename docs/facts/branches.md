# Branch Facts

`BranchObligation` describes a syntax-level branch that may need policy review,
for example an error path in Go. Rules read these through `RuleCtx::branches()`,
`RuleCtx::branch_obligations_for_file(file_id)`, or
`RuleCtx::branch_obligations(function_id)`.

## Fields

| Field | Meaning |
|-------|---------|
| `id` | Stable ID for this analysis run. |
| `function` | Owning function ID when known. |
| `file` | Stable `FileId` for the source file. |
| `decision_span` | Source span for the branch decision. |
| `condition_text` | Extracted condition text. This is syntax text, not a normalized semantic expression. |
| `edge_label` | Human-readable edge label such as a branch direction or case label. |
| `is_error_path` | Heuristic signal that this branch appears to be an error path. |
| `stable_fingerprint` | Stable branch identity derived from source facts, suitable for evidence. |

## Limits

- Error-path detection is heuristic and syntax-only.
- This is not exact branch coverage.
- `condition_text` is for diagnostics and heuristics, not semantic equality.
- Coverage facts are reserved for future external coverage imports.

## Small Rule Shape

```rust
use polint::sdk::prelude::*;

fn require_test_evidence(ctx: &mut RuleCtx<'_>) {
    let mut diagnostics = Vec::new();
    for branch in ctx.branches() {
        let file = ctx.file_path(branch.file);
        if branch.is_error_path
            && file_in_scope(ctx.options(), &file)
            && ctx.go_tests_for_related_file(branch.file).is_empty()
        {
            diagnostics.push(
                Diagnostic::warning(
                    "local/error-branch-tests",
                    file,
                    branch.decision_span.diagnostic_range(),
                    "No nearby test evidence found for this error branch.",
                )
                .with_evidence("condition", branch.condition_text.clone())
                .with_help("This is heuristic and does not prove exact coverage."),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
}
```
