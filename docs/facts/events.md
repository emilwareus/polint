# Events Facts

`Events<'_>` is a Phase 55 preview SDK view for matching semantic events.
Requesting it derives the `events` capability.

Phase 55 only exposes vocabulary. `polint check` reports `polint/capability` for
`events` and does not execute the requesting rule until Phase 56 provides
provider-backed event facts.

```rust
#[polint::rule(id = "local/no-raw-dangerous-call", description = "Dangerous calls", severity = "error")]
pub(crate) fn no_raw_dangerous_call(ctx: &mut RuleCtx<'_>, events: Events<'_>) -> RuleResult {
    for violation in events.matching(EventPattern::call("dangerous_exec")) {
        ctx.report(violation.diagnostic(ctx.rule_id(), "dangerous call is not allowed"));
    }

    Ok(())
}
```

## Pattern Vocabulary

- `EventPattern::call("target")` matches one exact canonical call target.
- `EventPattern::write_field("field")` matches one exact canonical field or
  property write.

Phase 55 intentionally starts with exact strings and explicit lists. It does not
include regex matching, AST-node selectors, closure filters, raw graph traversal,
or parser-specific IDs. Broader matching primitives should be added later only
when they have one clear spelling and documented precision.
