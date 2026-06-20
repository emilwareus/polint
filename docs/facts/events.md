# Events Facts

`Events<'_>` is a preview SDK view for matching semantic policy events.
Requesting it derives the supported `events` capability.

See [policy-queries.md](policy-queries.md) for the shared query-object style,
evidence header, precision/status vocabulary, unknown semantics, and template
starter workflow.

Phase 56 backs call-event matching with existing call and refined-call facts.
The API remains preview because additional event families, such as field writes
and lifecycle events, will be promoted in later phases.

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

- `EventPattern::call("target")` matches one exact call target string. Phase 56
  checks existing canonical labels such as symbol qualified names, symbol names,
  function names, synthetic targets, and syntactic callee labels.
- `EventPattern::write_field("field")` is preview vocabulary. It currently
  returns no provider-backed matches until write-event facts are promoted.

Results are returned as `PolicyViolation`s with the common policy evidence
header documented in [evidence.md](evidence.md), plus event-specific keys such
as `event`, `target`, `call_status`, `call_precision`, and `confidence` when
available. They do not expose raw AST, MIR, CFG, solver, call-graph node, or
provider IDs.

Matching intentionally starts with exact strings and explicit lists. It does not
include regex matching, closure filters, raw graph traversal, or parser-specific
selectors.
