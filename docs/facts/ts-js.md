# TypeScript / JavaScript Facts

TS/JS facts are produced by the Oxc adapter. Rules usually combine function,
import, literal, JSX attribute, component, and class facts.
Rule functions request these facts through typed views such as
`TsComponents<'_>`, `TsClasses<'_>`, `StringLiterals<'_>`, and
`JsxAttributes<'_>`.

## Component Facts

`TsComponentFact` describes a syntax-level component-like function.

| Field | Meaning |
|-------|---------|
| `file` | Stable `FileId` for the source file. |
| `function` | Owning function ID when known. |
| `name` | Component-like name. |
| `span` | Source span for the component-like syntax. |

## Class Facts

`TsClassFact` describes a class declaration or expression where the adapter can
extract a name.

| Field | Meaning |
|-------|---------|
| `file` | Stable `FileId` for the source file. |
| `name` | Class name. |
| `span` | Source span for the class. |
| `is_exported` | Syntax-level exported signal. |
| `is_component_like` | Heuristic component-like signal, not React type analysis. |

## Limits

- These facts do not use TypeScript type checking.
- Component detection is heuristic.
- Export detection is syntactic.
- Invalid syntax can produce parser diagnostics and fewer facts.

## Small Rule Shape

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/export-components",
    description = "Require component-like classes to be exported.",
    severity = "warn"
)]
fn require_exported_component_name(
    ctx: &mut RuleCtx<'_>,
    classes: TsClasses<'_>,
) -> RuleResult {
    let mut diagnostics = Vec::new();
    for class in classes.iter() {
        let file = ctx.file_path(class.file);
        if class.is_component_like && !class.is_exported && file_in_scope(ctx.options(), &file) {
            diagnostics.push(Diagnostic::warning(
                ctx.rule_id(),
                file,
                class.span.diagnostic_range(),
                format!("Component-like class `{}` is not exported.", class.name),
            ));
        }
    }
    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
```
