# Custom Go Rule

This example shows how a Go policy can combine branch facts with test facts.

The policy is `local/require-error-branch-tests`. It looks for error-return
branches and asks for nearby test evidence. This is useful for teams that want
important error paths to be covered before code is merged.

## Run It

From this directory:

```bash
polint check --format json --fail-on none
```

## What It Finds

`authorize.go` intentionally returns an error without a companion test:

```go
func Authorize(err error) error {
	if err != nil {
		return err
	}
	return nil
}
```

The expected finding is `local/require-error-branch-tests`. A real fix would add
a test that forces `Authorize` through the `err != nil` path.

## Writing A Similar Rule

Start a Go rule in a real project with:

```bash
polint new-rule go require-payment-error-tests
```

The useful SDK calls for this kind of policy are `ctx.branches()` and
`ctx.go_tests_for_related_file(...)`:

```rust
for branch in ctx.branches() {
    if branch.is_error_path && ctx.go_tests_for_related_file(branch.file).is_empty() {
        ctx.report(
            Diagnostic::warning(
                "local/require-payment-error-tests",
                ctx.file_path(branch.file),
                branch.decision_span.diagnostic_range(),
                "Add a test for this error branch.",
            )
            .with_evidence("condition", branch.condition_text.clone()),
        );
    }
}
```

Use `polint test-rules` to verify product fixture wiring while you iterate:

```bash
polint test-rules --format json
```
