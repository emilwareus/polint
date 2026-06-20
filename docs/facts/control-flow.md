# Control-Flow Facts

`ControlFlow<'_>` is a Phase 55 preview SDK view for guard and lifecycle
policies. Requesting it derives the `control_flow` capability.

Phase 55 only exposes vocabulary. `polint check` reports `polint/capability` for
`control_flow` and does not execute the requesting rule until Phase 57 provides
provider-backed guard and lifecycle facts.

```rust
#[polint::rule(id = "local/require-auth-before-balance-write", description = "Auth guard", severity = "error")]
pub(crate) fn require_auth_before_balance_write(
    ctx: &mut RuleCtx<'_>,
    control: ControlFlow<'_>,
) -> RuleResult {
    let query = GuardQuery::new(
        EventPattern::write_field("account.balance"),
        GuardPattern::call_any(["authorize", "require_admin"]),
    );

    for violation in control.missing_guard(query) {
        ctx.report(violation.diagnostic(ctx.rule_id(), "balance writes require authorization"));
    }

    Ok(())
}
```

```rust
#[polint::rule(id = "local/transaction-cleanup", description = "Transaction lifecycle", severity = "error")]
pub(crate) fn transaction_cleanup(ctx: &mut RuleCtx<'_>, control: ControlFlow<'_>) -> RuleResult {
    let mut query = LifecycleQuery::new(
        EventPattern::call("begin_transaction"),
        EventPattern::call("rollback"),
    );
    query.require_error_cleanup = true;

    for violation in control.missing_cleanup(query) {
        ctx.report(violation.diagnostic(ctx.rule_id(), "transaction requires cleanup"));
    }

    Ok(())
}
```

## Query Vocabulary

- `GuardQuery::new(event, guard)` requires an event and one guard pattern.
- `LifecycleQuery::new(start, cleanup)` requires a start event and cleanup
  event.
- `GuardPattern::call_any([...])` is an explicit list of canonical call names.
- `max_depth`, `max_paths`, `minimum_precision`, and
  `require_error_cleanup` are explicit option fields with deterministic
  defaults.

`ControlFlow<'_>` is not the raw `Cfg<'_>` view. `Cfg<'_>` remains a reserved
raw capability and is not the supported rule-authoring path for guard or
lifecycle policies.
