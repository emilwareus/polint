# Custom TypeScript Rule

This example shows how a frontend policy can inspect TypeScript/TSX literals and
JSX attributes.

The policy is `local/no-product-hex-colors`. It catches raw product colors so
contributors use design tokens instead of embedding ad hoc color values.

## Run It

From this directory:

```bash
polint check --format json --fail-on none
```

## What It Finds

`Button.tsx` intentionally stores a raw hex value:

```tsx
const danger = "#ff00aa";
```

The expected finding is `local/no-product-hex-colors`. A real fix would replace
`"#ff00aa"` with an approved token such as `tokens.color.danger`.

## Writing A Similar Rule

Start a TypeScript/JavaScript rule in a real project with:

```bash
polint new-rule ts no-product-hex-colors
```

The useful SDK views for this kind of policy are `StringLiterals<'_>` and
`JsxAttributes<'_>`. Requesting those parameters is also how polint derives the
rule's capabilities:

```rust
#[polint::rule(
    id = "local/no-product-hex-colors",
    description = "Reject raw product colors.",
    severity = "warn"
)]
fn no_product_hex_colors(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
) -> RuleResult {
    for literal in literals.iter() {
        if !literal.value.starts_with('#') {
            continue;
        }

        ctx.report(
            Diagnostic::warning(
                ctx.rule_id(),
                ctx.file_path(literal.file),
                literal.span.diagnostic_range(),
                "Use a design token instead of a raw color.",
            )
            .with_evidence("literal", literal.value.clone()),
        );
    }
    Ok(())
}
```

Use `polint check` to verify product fixture wiring while you iterate:

```bash
polint check --format json --fail-on none
```
