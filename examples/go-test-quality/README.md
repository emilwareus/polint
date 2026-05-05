# Go Test Quality Example

This example models a lightweight quality gate for Go tests.

The policy is `local/go-test-quality`. It flags tests with no obvious assertion
or error check, and it can also score tests that are growing too large.

## Run It

From this directory:

```bash
polint check --format json --fail-on none
```

## What It Finds

`payment_test.go` intentionally calls production-looking code without an
assertion:

```go
func TestAuthorize(t *testing.T) {
	Authorize()
}
```

The expected finding is `local/go-test-quality`. In this example the threshold is
set to `0`, so the same rule also emits its maintainability-score diagnostic. A
real fix would assert the expected result, check an error, or call `t.Fatal` on
failure.
