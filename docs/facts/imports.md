# Import Facts

`ImportFact` describes a syntactic Go or TS/JS import. Rules request the
`Imports<'_>` typed fact view on a `#[polint::rule]` function. The same
parameter is how polint derives the `imports` capability.

## Fields

| Field | Meaning |
|-------|---------|
| `id` | Stable ID for this analysis run. |
| `file` | Stable `FileId` for the source file. |
| `package` | Optional local package/alias/name when the adapter can extract one. |
| `path` | Imported module path or package string exactly as the adapter harvested it. |
| `span` | Source span for the import syntax. |
| `language` | Source language family. |

## Limits

- Imports are syntactic. polint does not currently resolve Node, TypeScript, or
  Go module paths to files.
- `package` is best-effort and may be `None` for common unaliased imports.
- Dynamic imports and unusual parser recovery cases may be incomplete.

## Small Rule Shape

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/forbidden-import",
    description = "Reject forbidden imports.",
    severity = "error"
)]
fn check_forbidden_import(ctx: &mut RuleCtx<'_>, imports: Imports<'_>) -> RuleResult {
    let forbidden = ctx
        .options()
        .settings
        .get("forbidden")
        .and_then(|value| value.as_str())
        .unwrap_or("internal/legacy");
    let mut diagnostics = Vec::new();
    for import in imports.iter() {
        let file = ctx.file_path(import.file);
        if file_in_scope(ctx.options(), &file) && import.path.contains(forbidden) {
            diagnostics.push(
                Diagnostic::error(
                    ctx.rule_id(),
                    file,
                    import.span.diagnostic_range(),
                    "Import violates a local boundary.",
                )
                .with_evidence("import", import.path.clone()),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
```
