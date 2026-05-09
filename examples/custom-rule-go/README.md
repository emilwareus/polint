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

The useful SDK views for this kind of policy are `BranchObligations<'_>` and
`GoTests<'_>`. Requesting those parameters is also how polint derives the rule's
capabilities:

```rust
#[polint::rule(
    id = "local/require-payment-error-tests",
    description = "Require test evidence for Go error branches.",
    severity = "warn"
)]
fn require_payment_error_tests(
    ctx: &mut RuleCtx<'_>,
    branches: BranchObligations<'_>,
    tests: GoTests<'_>,
) -> RuleResult {
    for branch in branches.iter() {
        if !branch.is_error_path || !tests.related_for_file(branch.file).is_empty() {
            continue;
        }

        ctx.report(
            Diagnostic::warning(
                ctx.rule_id(),
                ctx.file_path(branch.file),
                branch.decision_span.diagnostic_range(),
                "Add a test for this error branch.",
            )
            .with_evidence("condition", branch.condition_text.clone()),
        );
    }
    Ok(())
}
```

Use `polint test-rules` to verify product fixture wiring while you iterate:

```bash
polint test-rules --format json
```
