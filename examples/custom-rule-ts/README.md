# Custom TypeScript Rule

This example shows the kind of TSX code a repo-local frontend policy can
inspect. `Button.tsx` intentionally uses a raw color literal:

```tsx
const danger = "#ff00aa";
```

Run the checked-in fixture from this directory:

```bash
polint check --profile fast --format json --fail-on none
```

The fixture uses the built-in `examples/ts-no-raw-colors` rule so the example is
executable in v1. To start authoring a repo-local version of that policy,
scaffold a TypeScript/JavaScript rule:

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

Test the rule fixture path:

```bash
polint test-rules --format json
```

Generated repo-local Rust rules are scaffolded for authoring/testing and are not
automatically compiled or dynamically loaded by `polint check` in v1. Native
registration and the built-in example rules are the current executable path.
