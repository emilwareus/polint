# Calls Facts

`Calls<'_>` is a Phase 55 preview SDK view for policy-level reachability checks.
Requesting it derives the `calls` capability.

Phase 55 only exposes vocabulary. `polint check` reports `polint/capability` for
`calls` and does not execute the requesting rule until Phase 56 provides
provider-backed call-query facts.

```rust
#[polint::rule(id = "local/no-dangerous-reachable", description = "Reachability policy", severity = "error")]
pub(crate) fn no_dangerous_reachable(ctx: &mut RuleCtx<'_>, calls: Calls<'_>) -> RuleResult {
    let mut query = ReachQuery::new(EventPattern::call("dangerous_exec"));
    query.roots = vec![EventPattern::call("http_handler")];
    query.include_tests = false;
    query.max_depth = 20;
    query.max_paths = 20;

    for violation in calls.forbidden_reachable(query) {
        ctx.report(violation.diagnostic(ctx.rule_id(), "dangerous call is reachable"));
    }

    Ok(())
}
```

## Query Vocabulary

- `ReachQuery::new(target)` requires one target `EventPattern`.
- `query.roots` constrains the roots when non-empty.
- `query.include_tests`, `query.max_depth`, `query.max_paths`, and
  `query.minimum_precision` are explicit option fields with deterministic
  defaults.

`Calls<'_>` is not the raw `CallGraph<'_>` view. `CallGraph<'_>` remains a
reserved raw capability and is not the supported rule-authoring path for
reachability policies.
