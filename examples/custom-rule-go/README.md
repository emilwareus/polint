# Custom Go Rule

```bash
polint new-rule go require-payment-error-tests
```

Edit `.polint/rules/require-payment-error-tests/src/lib.rs` and use `ctx.go_tests()` plus `ctx.branch_obligations(function.id)` to connect error paths to nearby test evidence.
