# Phase 58: Data-Flow Source/Sink/Barrier Queries - Context

**Gathered:** 2026-06-20
**Status:** Ready for planning
**Mode:** Autonomous, inline

<domain>
## Phase Boundary

Phase 58 promotes `DataFlow<'_>` from preview vocabulary to a backed policy
query surface for `FlowQuery` source-to-sink policies with optional
barrier/sanitizer calls. The public rule-authoring style remains the v1.4
"one good way" shape:

```rust
let mut query = FlowQuery::new(
    SourcePattern::http_request(),
    SinkPattern::call("exec.Command"),
);
query.barriers = BarrierPattern::call_any(["validate_command"]);

for violation in flow.forbidden(query) {
    ctx.report(violation.diagnostic(ctx.rule_id(), "Request data reaches shell execution."));
}
```

The first supported scope is provider-backed path search over private
data-flow facts. It should connect supported source models to MIR parameter
places, match call sinks through existing call/refined-call facts and call-site
argument places, and use the private bounded data-flow path search. It must not
expose raw data-flow nodes, edge IDs, solver internals, MIR IDs, call graph IDs,
extension facts, or graph traversal APIs to rule authors.

</domain>

<decisions>
## Implementation Decisions

- Keep the public API simple: `DataFlow<'_>` plus one `FlowQuery`, no fluent
  builder, string DSL, closure filter, callback traversal, or public graph API.
- Promote only backed behavior in this phase. `SourcePattern::http_request`,
  `SourcePattern::secret_like`, `SinkPattern::call`, `SinkPattern::logger`, and
  `BarrierPattern::call_any` should produce deterministic results where the
  private facts support them.
- Add source-introduction edges inside the private data-flow provider so trust
  boundary source nodes can reach matching parameter places. Do not make source
  nodes or place IDs public.
- Treat built-in name patterns such as `secret_like` and `logger` as heuristic
  pattern matching, and surface that through policy status/precision/evidence.
- Use existing `analysis::data_flow::query::find_paths` for bounded search.
  `max_depth` and `max_paths` are real caps; budget and unknown outcomes must be
  visible in violation evidence instead of silently passing.
- Barriers are path filters: if every found source-to-sink path crosses a
  matching barrier call, no violation is reported. If any found path does not
  cross a barrier, report that uncovered path.

</decisions>

<code_context>
## Existing Code Insights

- `crates/polint/src/sdk/facts.rs` currently leaves `DataFlow<'_>` in
  fail-closed preview mode through `preview_query_unavailable`.
- `crates/polint/src/sdk/policy.rs` already defines `FlowQuery`,
  `SourcePattern`, `SinkPattern`, and `BarrierPattern`, but source/sink/barrier
  private accessors are not yet exposed to internal query code.
- `crates/polint/src/policy_queries.rs` is the existing private bridge from SDK
  preview views to backed query behavior for events, calls, and control flow.
- `analysis/data_flow/provider.rs` derives local place nodes, local value-flow
  edges, direct call edges, summary-projected edges, source models from trust
  boundaries, and extension models.
- Trust-boundary source nodes are currently standalone. Phase 58 needs private
  `SourceIntroduction` edges from source nodes to matching parameter place nodes
  before HTTP request source policies can produce real paths.
- `analysis/data_flow/query.rs` already provides deterministic bounded path
  search with found, unknown, not found, and budget-exceeded status rows.
- `analysis_plan.rs` and `analysis_kernel/mod.rs` still treat `dataflow` as an
  unsupported preview capability; promoting Phase 58 requires moving it to the
  semantic pipeline.

</code_context>

<specifics>
## Specific Ideas

- Match HTTP request sources from `DataFlowNodeKind::Source` nodes backed by
  trust-boundary models whose payload labels include request source kinds:
  `PathParam`, `QueryString`, `RequestBody`, `RequestHeader`, or `Cookie`.
- Match secret-like sources from source model labels or place/root names using
  the explicit user-supplied strings from `SourcePattern::secret_like`.
- Match call sinks by joining refined call edges, call sites, and call-site
  argument places. Sink nodes can be the argument/receiver places that reach the
  call boundary rather than a public sink-node abstraction.
- Match logger sinks as a small heuristic target-name set such as `log.Print`,
  `log.Printf`, `log.Println`, `console.log`, `logger.info`, and similar
  canonical labels already emitted by call facts.
- Detect barrier calls by finding whether any edge or endpoint on a found path
  is associated with a call site whose refined target matches
  `BarrierPattern::call_any`.
- Emit `PolicyViolation` rows at the sink call span with evidence for source,
  sink, path, path status, barrier status, precision, confidence, budget reason,
  and supported scope.

</specifics>

<deferred>
## Deferred Ideas

- Exact sanitizer semantics and taint-killing transfer functions.
- Full source/sink taxonomy for SQL, raw HTML/JSX, SSRF URLs, file paths,
  analytics, PII, and outbound network clients.
- Context-sensitive whole-program taint precision controls.
- Public model-pack or extension authoring APIs.
- Public raw `DataFlowGraph`, solver, path, node, or edge APIs.

</deferred>
