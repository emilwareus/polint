# Go Branch Obligations Example

This example models a team rule for important error paths.

The policy is `local/go-branch-obligations`. It finds Go error branches, then
looks for nearby test evidence that appears to exercise the same condition.

## Run It

From this directory:

```bash
cargo run --manifest-path .polint/rules/go-branch-obligations/Cargo.toml -- check --profile fast --format json --fail-on none
```

## What It Finds

`authorize.go` intentionally has error branches without companion tests:

```go
if err := charge(amount); err != nil {
	return err
}
```

The expected finding is `local/go-branch-obligations`. The rule is heuristic: it
does not prove exact coverage. It gives teams an executable reminder to add test
cases around branches they care about.
