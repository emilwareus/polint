# Go Complexity Example

This example models a simple maintainability policy for Go functions.

The policy is `local/go-cyclomatic-complexity`. It reports Go functions whose
branch count exceeds the configured `max`.

## Run It

From this directory:

```bash
cargo run --manifest-path .polint/rules/go-complexity/Cargo.toml -- check --profile fast --format json --fail-on none
```

## What It Finds

`router.go` intentionally has two branches while the example config sets
`max = 1`:

```go
if kind == "admin" {
	return "admin"
}
if ready {
	return "ready"
}
```

The expected finding is `local/go-cyclomatic-complexity`. A real fix would split
the branching behavior into smaller helpers or raise the threshold if the team
decides the function is acceptable.
