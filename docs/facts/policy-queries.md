# Policy Query Preview

The v1.4 policy-query surface is a preview SDK for repo-local semantic
policies. It promotes useful call, control-flow, and data-flow behavior without
exposing raw CFGs, call graphs, data-flow graphs, solver internals, provider
rows, dense IDs, or `AnalysisDb`.

The public style has one shape:

1. Request a typed preview view in the `#[polint::rule]` function signature.
2. Construct one plain query object with `Query::new(required...)`.
3. Set explicit option fields when needed.
4. Run one method on the view.
5. Report each returned `PolicyViolation` through
   `violation.diagnostic(ctx.rule_id(), "...")`.

There is no string query language, fluent builder DSL, closure filter DSL, or
public graph traversal API for this preview surface.

## Views And Capabilities

| View | Capability | Backed Query |
|------|------------|--------------|
| `Events<'_>` | `events` | `events.matching(EventPattern::call(...))` |
| `Calls<'_>` | `calls` | `calls.forbidden_reachable(ReachQuery)` |
| `ControlFlow<'_>` | `control_flow` | `control.missing_guard(GuardQuery)` and `control.missing_cleanup(LifecycleQuery)` |
| `DataFlow<'_>` | `dataflow` | `flow.forbidden(FlowQuery)` |

Rules request these views exactly like stable fact views:

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-secret-logs",
    description = "Secret-like values must not reach logs.",
    severity = "error"
)]
pub(crate) fn no_secret_logs(ctx: &mut RuleCtx<'_>, flow: DataFlow<'_>) -> RuleResult {
    let mut query = FlowQuery::new(
        SourcePattern::secret_like(["token", "password", "apiKey"]),
        SinkPattern::logger(),
    );
    query.barriers = BarrierPattern::call_any(["redact", "mask_secret"]);
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

## Query Objects

- `ReachQuery::new(EventPattern::call("target"))` checks whether a forbidden
  call/event is reachable. Options include `roots`, `include_tests`,
  `max_depth`, `max_paths`, `minimum_precision`, and `minimum_confidence`.
- `GuardQuery::new(event, guard)` checks whether a sensitive call event is
  missing a prior guard in the same function. Options include `max_depth`,
  `max_paths`, and `minimum_precision`.
- `LifecycleQuery::new(start, cleanup)` checks whether a start/acquire call is
  missing later cleanup in the same function. Options include
  `require_error_cleanup`, `max_depth`, `max_paths`, and `minimum_precision`.
- `FlowQuery::new(source, sink)` checks whether a source can reach a sink.
  Options include `barriers`, `max_depth`, `max_paths`, and
  `minimum_precision`.

## Pattern Vocabulary

- `EventPattern::call("target")` matches exact call target candidates from the
  call/refined-call facts. `EventPattern::write_field("field")` is reserved
  preview vocabulary and currently returns no backed matches.
- `SourcePattern::http_request()` matches supported HTTP trust-boundary source
  models such as path params, query strings, request bodies, request headers,
  and cookies.
- `SourcePattern::secret_like([...])` heuristically matches explicit names
  against source labels and local place names/projections.
- `SinkPattern::call("target")` matches exact call target candidates and checks
  whether a source reaches a call argument or receiver.
- `SinkPattern::logger()` matches a small heuristic logger family such as
  `console.log`, `log.Print`, `logger.info`, and similar names.
- `GuardPattern::call_any([...])` and `BarrierPattern::call_any([...])` match
  explicit call-name lists. Barrier matching is conservative; generated
  templates should still be reviewed against local sanitizer behavior.

## Evidence

`PolicyViolation::diagnostic(...)` emits a normalized scalar evidence header:

- `policy_query`
- `policy_query_version`
- `query_digest`
- `policy_status`
- `policy_precision`

Query-specific evidence is flat and family-specific. Reachability diagnostics
include `root`, `target`, `path`, and `depth`. Control-flow diagnostics include
`required_guard` or `required_cleanup`, `control_scope`, `uncovered_path`, and
budget fields. Data-flow diagnostics include `source`, `sink`, `path_status`,
`path`, `barrier_status`, `required_barrier`, and budget fields.

## Precision, Status, And Unknowns

Policy results must be read with their evidence:

- `policy_status` can be `exact`, `heuristic`, `unknown`, `unsupported`, or
  `budget_exceeded`.
- `policy_precision` can be `exact`, `setup_aware`, `syntax`,
  `conservative`, `heuristic`, or `unknown`.
- `PolicyConfidence` is currently surfaced by reachable-call evidence where
  applicable.
- Budget limits are visible through policy evidence instead of being treated as
  complete absence proofs.
- Cap-filtered unknown reports are available through
  `polint unknowns --cap events|calls|control_flow|dataflow --format json`.

Unsupported preview vocabulary returns no backed policy matches until a later
phase promotes real facts. Setup gaps should produce `polint/capability`
diagnostics rather than running rules with placeholder facts.

## Template Starters

`polint new-rule <lang> <name> --template <id>` creates repo-local policy
scaffolds using the same query-object style. Template IDs are:

- `request-to-shell`
- `secret-to-log`
- `pii-to-analytics`
- `sensitive-write-guard`
- `transaction-cleanup`
- `raw-reachable-api`
- `ssrf`
- `dangerous-html`
- `unsafe-deserialization`
- `user-file-path`

Templates include positive and negative fixture cases under `.polint/tests/`.
They are starting points to edit for local APIs, not built-in rules enabled by
polint.

## Limits

The preview surface is intentionally narrower than the private analysis engine:

- Raw CFG, call graph, semantic graph, data-flow graph, provider, solver, MIR,
  and evidence-store APIs are not public rule-authoring APIs.
- Current control-flow queries are same-function call-event checks.
- Current data-flow queries are bounded and conservative.
- Name-based source, sink, guard, and barrier patterns are heuristic unless the
  underlying evidence says otherwise.
- Broader domain taxonomies such as SQL, SSRF, HTML, deserialization, analytics,
  PII, and file paths are represented by editable templates over current backed
  primitives, not by complete built-in detection.
