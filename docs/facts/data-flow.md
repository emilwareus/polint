# Data-Flow Facts

`DataFlow<'_>` is a Phase 55 preview SDK view for source-to-sink policy queries.
Requesting it derives the `dataflow` capability.

Phase 55 only exposes the rule-authoring vocabulary. `polint check` currently
reports `polint/capability` for `dataflow` and does not execute the requesting
rule. Provider-backed query results, path evidence, unknowns, budgets, and cache
semantics are deferred to Phase 58 and Phase 59.

```rust
#[polint::rule(id = "local/no-secret-logs", description = "Secret logs", severity = "error")]
pub(crate) fn no_secret_logs(ctx: &mut RuleCtx<'_>, flow: DataFlow<'_>) -> RuleResult {
    let mut query = FlowQuery::new(
        SourcePattern::secret_like(["token", "password", "apiKey"]),
        SinkPattern::logger(),
    );
    query.barriers = BarrierPattern::call_any(["redact", "mask_secret"]);
    query.max_depth = 8;
    query.max_paths = 20;

    for violation in flow.forbidden(query) {
        ctx.report(violation.diagnostic(
            ctx.rule_id(),
            "secret-like value reaches logging without redaction",
        ));
    }

    Ok(())
}
```

## Query Vocabulary

- `FlowQuery::new(source, sink)` requires one `SourcePattern` and one
  `SinkPattern`.
- `query.barriers` accepts a `BarrierPattern` such as
  `BarrierPattern::call_any(["redact", "mask_secret"])`.
- `query.max_depth`, `query.max_paths`, and `query.minimum_precision` are
  explicit option fields with deterministic defaults.

Phase 55 source patterns are named constructors. `SourcePattern::http_request()`
matches request input sources once Phase 58 wires behavior.
`SourcePattern::secret_like([...])` is an explicit list of canonical strings and
is heuristic by design. It must not be described as exact secret detection.

Phase 55 sink patterns are also named constructors. `SinkPattern::call("...")`
matches one exact canonical call target. `SinkPattern::logger()` is a named
logger sink family whose final behavior is deferred to Phase 58.

The public API does not expose raw data-flow nodes, graph edges, solver IDs,
provider rows, or `AnalysisDb`.
