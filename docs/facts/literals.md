# Literal And JSX Attribute Facts

Literal facts let rules inspect string-like syntax without parsing source text
by hand. Rules request `StringLiterals<'_>` and `JsxAttributes<'_>` as typed
fact-view parameters on a `#[polint::rule]` function. Those parameters are also
how polint derives the rule's required capabilities.

## String Literal Fields

| Field | Meaning |
|-------|---------|
| `file` | Stable `FileId` for the source file. |
| `value` | Extracted literal value or source-like text for syntax such as regex literals. |
| `span` | Source span for the literal. |
| `language` | Source language family. |

## JSX Attribute Fields

| Field | Meaning |
|-------|---------|
| `file` | Stable `FileId` for the source file. |
| `name` | Attribute name. |
| `value` | Quoted/static attribute value when available. Boolean or dynamic attributes may be `None`. |
| `span` | Source span for the attribute. |

## Limits

- Dynamic template literals are not evaluated.
- Regex literals are syntax-level text, not compiled regex behavior.
- JSX expression values are only available when the adapter can harvest a static value.
- Import paths are not duplicated as general Go string literals.

## Small Rule Shape

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-configured-literal",
    description = "Reject a configured literal value.",
    severity = "warn"
)]
fn reject_configured_literal(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
) -> RuleResult {
    let denied = ctx
        .options()
        .settings
        .get("literal")
        .and_then(|value| value.as_str())
        .unwrap_or("TODO")
        .to_string();

    let mut diagnostics = Vec::new();
    for literal in literals.iter() {
        let file = ctx.file_path(literal.file);
        if file_in_scope(ctx.options(), &file) && literal.value == denied {
            diagnostics.push(
                Diagnostic::warning(
                    ctx.rule_id(),
                    file,
                    literal.span.diagnostic_range(),
                    "Configured literal is not allowed here.",
                )
                .with_evidence("literal", literal.value.clone()),
            );
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
```
