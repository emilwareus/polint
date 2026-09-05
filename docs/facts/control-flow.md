# Control-Flow Facts

`ControlFlow<'_>` is a preview SDK view for guard and lifecycle policies.
Requesting it derives the supported `control_flow` capability.

See [policy-queries.md](policy-queries.md) for the shared query-object style,
evidence header, precision/status vocabulary, unknown semantics, and template
starter workflow.

Same-function call-event guard and lifecycle queries use refined call facts and
CFG dominance or post-dominance when those relations are available. MIR
operation order and source spans remain fallback ordering sources when CFG rows
are absent. The API remains preview because interprocedural proof, write-field
events, and resource identity pairing are deferred.

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
- `EventPattern::call(...)` is supported for guard and lifecycle queries.
  `EventPattern::write_field(...)` is still preview vocabulary and returns no
  control-flow results.
- `max_paths` caps returned violations and reports budget evidence when
  truncated.
- `minimum_precision` filters the private call facts considered by the query.
- `max_depth` is present for a stable query shape, but the current engine only evaluates
  same-function depth. Values above `1` do not enable interprocedural search yet.
- `require_error_cleanup` is surfaced as evidence. Cleanup calls must
  post-dominate the matching start when CFG relation facts are available.

Returned diagnostics include the common policy evidence header documented in
[evidence.md](evidence.md), plus target, function, control scope, required
guard or cleanup, uncovered path, order-source, call status, call precision,
confidence when available, and budget evidence when truncation occurs. Results
remain conservative and heuristic because matching calls and lifecycle identity
are not semantic resource proofs. The `order_source` evidence is
`cfg_operation_order` when CFG rows are available, otherwise
`mir_operation_order` or `source_span`.

`ControlFlow<'_>` is not the raw `Cfg<'_>` view. `Cfg<'_>` remains a reserved
raw capability and is not the supported rule-authoring path for guard or
lifecycle policies.

## Template Starters

`polint new-rule go require-sensitive-write-guard --template
sensitive-write-guard` scaffolds a guard-before-sensitive-call policy.
`polint new-rule go require-transaction-cleanup --template transaction-cleanup`
scaffolds a same-function cleanup policy. Generated templates use
`ControlFlow<'_>` and the query-object style shown above, with fixtures that
users can edit to their local guard, write, begin, and cleanup APIs.
