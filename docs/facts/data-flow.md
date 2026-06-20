# Data-Flow Facts

`DataFlow<'_>` is a v1.4 preview SDK view for source-to-sink policy queries.
Requesting it derives the supported `dataflow` capability.

The public surface is intentionally policy-level. Rules construct one
`FlowQuery`, run `flow.forbidden(query)`, and report returned
`PolicyViolation` values. polint does not expose raw data-flow nodes, graph
edges, solver IDs, provider rows, MIR IDs, or `AnalysisDb` to rule authors.

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

## Query Shape

- `FlowQuery::new(source, sink)` requires one `SourcePattern` and one
  `SinkPattern`.
- `query.barriers` accepts `BarrierPattern::none()` or
  `BarrierPattern::call_any(["redact", "mask_secret"])`.
- `query.max_depth` and `query.max_paths` cap the private path search.
- `query.minimum_precision` is retained as the public precision knob. Phase 58
  reports precision evidence and keeps stricter filtering conservative until the
  shared evidence semantics are normalized in Phase 59.

There is no alternate fluent builder, string query language, closure filter, or
public graph traversal API.

## Supported Patterns

Phase 58 backs these patterns:

- `SourcePattern::http_request()` matches trust-boundary source models for HTTP
  route params, query strings, request bodies, request headers, and cookies. The
  private provider introduces these source models into matching handler
  parameter places before bounded path search.
- `SourcePattern::secret_like([...])` matches explicit source names supplied by
  the rule author against source labels and MIR place names/projections. This is
  heuristic name matching, not exact secret detection.
- `SinkPattern::call("target")` matches exact call target candidates from
  existing call/refined-call facts and checks whether a source reaches an
  argument or receiver place for that call.
- `SinkPattern::logger()` matches a small heuristic logger target family such as
  `console.log`, `log.Print`, `log.Printf`, `log.Println`, `logger.info`,
  `logger.warn`, and `logger.error`.
- `BarrierPattern::call_any([...])` suppresses a found violation when the found
  path crosses a matching sanitizer/barrier call. If any found path reaches the
  sink without such a call, that uncovered path is reported.

## Result Evidence

Diagnostics built through `violation.diagnostic(...)` include standard
`policy_status` and `policy_precision` fields plus data-flow evidence such as:

- `policy=forbidden_flow`
- `source`
- `sink`
- `path_status`
- `path`
- `path_edge_count`
- `barrier_status`
- `required_barrier` when configured
- `supported_scope=bounded_private_data_flow`
- `requested_max_depth`
- `requested_max_paths`
- `budget_reason` when a cap prevents a complete answer

Found flows that depend on heuristic source or sink patterns report heuristic
status/precision honestly. Unknown paths and budget-exceeded paths produce
visible policy results instead of silently passing.

## Limits

Phase 58 is useful for repo-local policies such as secret-to-log and
request-to-dangerous-call checks, but it is still preview:

- It does not prove perfect sanitizer semantics or taint-killing transfer
  functions.
- It does not yet cover the full planned sink taxonomy for SQL, raw HTML/JSX,
  SSRF URLs, file paths, analytics, PII, and outbound network clients.
- It does not expose context-sensitivity controls.
- Extension/model-pack authoring remains internal.
- Raw `Cfg<'_>`, raw `CallGraph<'_>`, and raw data-flow graph APIs remain
  reserved.
