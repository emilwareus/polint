# Go Complexity Example

Minimal Go fixture for `examples/go-cyclomatic-complexity`.

Run it from this directory:

```bash
polint check --profile fast --format json --fail-on none
```

`router.go` intentionally has two branches while the example config sets
`max = 1`, so the CLI emits one complexity diagnostic.
