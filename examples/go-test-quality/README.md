# Go Test Quality Example

Minimal Go test fixture for two heuristic test-quality policies:

- `examples/go-test-suite-size`
- `examples/go-assertion-after-action`

Run it from this directory:

```bash
polint check --profile fast --format json --fail-on none
```

`payment_test.go` intentionally calls production-looking code without an
assertion. The suite-size threshold is set to `0` so the tiny fixture also emits
the maintainability-score diagnostic.
