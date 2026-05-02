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

This directory is self-contained: the local rule implementation lives at
`.polint/rules/require-error-branch-tests/src/main.rs`.

Run the checked-in fixture from this directory:

```bash
cargo run --manifest-path .polint/rules/require-error-branch-tests/Cargo.toml -- check --profile fast --format json --fail-on none
```

The fixture uses its own `local/require-error-branch-tests` rule. To start
authoring another repo-local Go policy in a real project, scaffold a Go rule:

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

The checked-in rule crate is the executable example. Product `polint check`
does not automatically compile repo-local Rust rules in v1, so this example
uses a tiny native rule host under `.polint/rules/require-error-branch-tests`.

Test the product fixture path without local rule registration:

```bash
polint test-rules --format json
```
