# Custom Go Rule

This example shows the kind of Go code a repo-local policy rule can inspect.
`authorize.go` intentionally has an untested error branch:

```go
func Authorize(err error) error {
	if err != nil {
		return err
	}
	return nil
}
```

Run the checked-in fixture from this directory:

```bash
polint check --profile fast --format json --fail-on none
```

The fixture uses the built-in `examples/go-branch-obligations` rule so the
example is executable in v1. To start authoring a repo-local version of that
policy, scaffold a Go rule:

```bash
polint new-rule go require-payment-error-tests
```

Then edit `.polint/rules/require-payment-error-tests/src/lib.rs` and use
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
