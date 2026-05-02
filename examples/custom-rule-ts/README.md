# Custom TypeScript Rule

This example shows the kind of TSX code a repo-local frontend policy can
inspect. `Button.tsx` intentionally uses a raw color literal:

```tsx
const danger = "#ff00aa";
```

This directory is self-contained: the local rule implementation lives at
`.polint/rules/no-product-hex-colors/src/main.rs`.

Run the checked-in fixture from this directory:

```bash
cargo run --manifest-path .polint/rules/no-product-hex-colors/Cargo.toml -- check --profile fast --format json --fail-on none
```

The fixture uses its own `local/no-product-hex-colors` rule. To start authoring
another repo-local TypeScript/JavaScript policy in a real project, scaffold a
rule:

```bash
polint new-rule ts no-product-hex-colors
```

Use `ctx.string_literals()` and `ctx.jsx_attributes()` to enforce
project-specific frontend policies such as design-token usage:

```rust
for literal in ctx.string_literals() {
    if literal.value.starts_with('#') {
        ctx.warn(&literal.span, "Use a design token instead of a raw color");
    }
}

for attr in ctx.jsx_attributes() {
    if attr.value.as_deref().is_some_and(|value| value.starts_with('#')) {
        ctx.warn(&attr.span, "Use a design token instead of a raw color");
    }
}
```

The checked-in rule crate is the executable example. Product `polint check`
does not automatically compile repo-local Rust rules in v1, so this example
uses a tiny native rule host under `.polint/rules/no-product-hex-colors`.

Test the product fixture path without local rule registration:

```bash
polint test-rules --format json
```
