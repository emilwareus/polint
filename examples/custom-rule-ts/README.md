# Custom TypeScript Rule

Scaffold a repo-local TypeScript/JavaScript rule:

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
