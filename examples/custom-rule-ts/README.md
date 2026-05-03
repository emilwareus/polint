# Custom TypeScript Rule

This example shows how a frontend policy can inspect TypeScript/TSX literals and
JSX attributes.

The policy is `local/no-product-hex-colors`. It catches raw product colors so
contributors use design tokens instead of embedding ad hoc color values.

## Run It

From this directory:

```bash
cargo run --manifest-path .polint/rules/no-product-hex-colors/Cargo.toml -- check --profile fast --format json --fail-on none
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

The useful SDK calls for this kind of policy are `ctx.string_literals()` and
`ctx.jsx_attributes()`:

```rust
for literal in ctx.string_literals() {
    if literal.value.starts_with('#') {
        ctx.report(
            Diagnostic::warning(
                "local/no-product-hex-colors",
                ctx.file_path(literal.file),
                literal.span.diagnostic_range(),
                "Use a design token instead of a raw color.",
            )
            .with_evidence("literal", literal.value.clone()),
        );
    }
}
```

Use `polint test-rules` to verify product fixture wiring while you iterate:

```bash
polint test-rules --format json
```
