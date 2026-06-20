# Control-Flow Facts

`ControlFlow<'_>` is a preview SDK view for guard and lifecycle policies.
Requesting it derives the supported `control_flow` capability.

Phase 57 backs same-function call-event guard and lifecycle queries with the
existing call/refined-call facts and CFG operation order where available. The
API remains preview because exact interprocedural path proof, write-field
events, resource identity pairing, and per-exit cleanup proof are deferred.

```rust
#[polint::rule(id = "local/require-auth-before-dangerous-call", description = "Auth guard", severity = "error")]
pub(crate) fn require_auth_before_dangerous_call(
    ctx: &mut RuleCtx<'_>,
    control: ControlFlow<'_>,
) -> RuleResult {
    let query = GuardQuery::new(
        EventPattern::call("dangerous_exec"),
        GuardPattern::call_any(["authorize", "require_admin"]),
    );

    for violation in control.missing_guard(query) {
        ctx.report(violation.diagnostic(ctx.rule_id(), "dangerous calls require authorization"));
    }

    Ok(())
}
```

```rust
#[polint::rule(id = "local/transaction-cleanup", description = "Transaction lifecycle", severity = "error")]
pub(crate) fn transaction_cleanup(ctx: &mut RuleCtx<'_>, control: ControlFlow<'_>) -> RuleResult {
    let mut query = LifecycleQuery::new(
        EventPattern::call("Begin"),
        EventPattern::call("Rollback"),
    );
    query.require_error_cleanup = true;

    for violation in control.missing_cleanup(query) {
        ctx.report(violation.diagnostic(ctx.rule_id(), "transaction begin requires cleanup"));
    }

    Ok(())
}
```

## Query Vocabulary

- `GuardQuery::new(event, guard)` requires an event and one guard pattern.
- `LifecycleQuery::new(start, cleanup)` requires a start event and cleanup
  event.
- `GuardPattern::call_any([...])` is an explicit list of canonical call names.
- Phase 57 supports `EventPattern::call(...)` for guard and lifecycle queries.
  `EventPattern::write_field(...)` is still preview vocabulary and returns no
  provider-backed control-flow results.
- `max_paths` caps returned violations and reports budget evidence when
  truncated.
- `minimum_precision` filters the private call facts considered by the query.
- `max_depth` is present for a stable query shape, but Phase 57 only evaluates
  same-function depth. Values above `1` do not enable interprocedural search yet.
- `require_error_cleanup` is surfaced as evidence. Phase 57 does not prove exact
  cleanup on every normal and error exit.

Returned diagnostics include the common policy evidence header documented in
[evidence.md](evidence.md), plus target, function, control scope, required
guard or cleanup, uncovered path, order-source, call status, call precision,
confidence when available, and budget evidence when truncation occurs. Because
the current implementation proves only same-function ordering, returned policy
results use conservative precision and heuristic status.

`ControlFlow<'_>` is not the raw `Cfg<'_>` view. `Cfg<'_>` remains a reserved
raw capability and is not the supported rule-authoring path for guard or
lifecycle policies.
