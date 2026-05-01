# Custom Go Rule

Scaffold a repo-local Go rule:

```bash
polint new-rule go require-payment-error-tests
```

Edit `.polint/rules/require-payment-error-tests/src/lib.rs` and use
`ctx.go_tests()` plus `ctx.branch_obligations(function.id)` to connect error
paths to nearby test evidence:

```rust
for function in ctx.functions() {
    let tests = ctx.go_tests();
    for obligation in ctx.branch_obligations(function.id) {
        if tests.is_empty() {
            ctx.warn(&obligation.decision_span, "Add companion test evidence");
        }
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
