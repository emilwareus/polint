# Phase 59: Violation Evidence, Unknowns, and Cache Semantics - Context

**Gathered:** 2026-06-20
**Status:** Ready for execution
**Mode:** Autonomous, inline

<domain>
## Phase Boundary

Phase 59 normalizes the result contract shared by the v1.4 policy query views:

- `Events<'_>::matching(EventPattern)`
- `Calls<'_>::forbidden_reachable(ReachQuery)`
- `ControlFlow<'_>::missing_guard(GuardQuery)`
- `ControlFlow<'_>::missing_cleanup(LifecycleQuery)`
- `DataFlow<'_>::forbidden(FlowQuery)`

The goal is not another query family. The goal is a stable, predictable
diagnostic/evidence shape for users, agents, JSON/SARIF consumers, and future
query-result caching. The public rule-authoring API remains the same: request a
typed view, build one query object, call one method, and report
`violation.diagnostic(ctx.rule_id(), "...")`.

</domain>

<decisions>
## Implementation Decisions

- Keep `PolicyViolation` as the shared result type; do not add a second public
  violation API or a generic evidence DSL.
- Do not overload `diagnostics::evidence_v1`. That field has a strict internal
  evidence-bundle schema. Phase 59 should standardize the existing public
  scalar evidence pairs unless a separate future policy evidence schema is
  intentionally introduced.
- Add common policy evidence to every policy diagnostic:
  `policy_query`, `policy_query_version`, `query_digest`, `policy_status`, and
  `policy_precision`.
- Keep query-specific evidence explicit and flat. For example, data-flow keeps
  `source`, `sink`, `path_status`, and `barrier_status`; control-flow keeps
  `required_guard` or `required_cleanup`; calls keeps `root`, `target`, and
  `path`.
- Query parameters are runtime values inside user rule code. They cannot
  honestly participate in analysis fact-cache keys today because query results
  are not cached as a separate analysis layer. They should instead participate
  in deterministic `query_digest` evidence. If query-result caching is added
  later, that digest is the key material.
- Preview policy-query schema/version must participate in analysis-plan digest
  material when policy capabilities are requested, so provider cache decisions
  are sensitive to semantics-level preview changes.
- `polint unknowns --cap ...` and `polint inspect unknowns --cap ...` should
  support policy capabilities where backed unknown rows exist. Unsupported raw
  capabilities should continue to return explicit unsupported rows.

</decisions>

<code_context>
## Existing Code Insights

- `crates/polint/src/sdk/policy.rs` defines `PolicyViolation`,
  `PolicyStatus`, `PolicyPrecision`, query structs, and pattern structs.
- `PolicyViolation::diagnostic` currently appends `policy_status` and
  `policy_precision`, then copies arbitrary evidence pairs supplied by
  `policy_queries.rs`.
- `crates/polint/src/policy_queries.rs` builds all Phase 56-58 policy results.
  Evidence shape is useful but currently assembled per query family.
- `diagnostics::Diagnostic` already exposes scalar evidence pairs publicly.
  `evidence_v1` exists, but its validator requires the internal evidence-bundle
  object shape and should not be reused for policy evidence.
- `analysis_plan.rs` computes deterministic plan digests from schema, rules,
  options, and requested capabilities. This is the right place to fold a
  policy-query preview schema/version for policy capabilities.
- `analysis/unknown_taxonomy/collect.rs` currently supports cap-filtered
  public unknowns for imports, symbols, and references, plus consolidated graph
  engine unknowns. Policy capabilities are still mostly routed to unsupported
  rows.
- Phase 58 made `dataflow` supported and provider-backed, but cap-filtered
  `dataflow` unknown inspection remains unpromoted.

</code_context>

<specifics>
## Specific Ideas

- Introduce an internal `PolicyQueryKind` or equivalent label helpers for:
  `events.matching`, `calls.forbidden_reachable`,
  `control_flow.missing_guard`, `control_flow.missing_cleanup`, and
  `data_flow.forbidden`.
- Add deterministic query digest helpers for `EventPattern`, `ReachQuery`,
  `GuardQuery`, `LifecycleQuery`, and `FlowQuery`, using length-prefixed
  encoding or the existing stable hash helper.
- Make query functions pass the query kind and query digest into
  `PolicyViolation` at construction time, so `diagnostic` emits common evidence
  without each query family hand-writing it.
- Sort and dedupe policy query results through one helper before returning them.
- Add cap-filtered unknown rows for policy capabilities:
  - `events` and `calls`: refined-call/call unknowns.
  - `control_flow`: call-event unknowns relevant to guard/lifecycle decisions.
  - `dataflow`: data-flow unknown edges, budget rows, and evidence unknown rows
    where available.
- Update CLI tests so `unknowns --cap dataflow` and
  `inspect unknowns --cap dataflow` return policy unknown rows or an empty
  supported report instead of an unsupported capability row.
- Update docs to describe the policy evidence keys and the cache boundary:
  provider facts are cached; query results are evaluated by rules; query
  parameters are visible through `query_digest`.

</specifics>

<deferred>
## Deferred Ideas

- A separate public structured `policy_evidence_v1` JSON field.
- Public query-result cache persisted across runs.
- Public path replay APIs or raw graph node IDs.
- Suggestions or auto-fixes for advanced policy violations.
- Exact per-query unknown explanations that require future model-pack or
  language-specific taxonomy work.

</deferred>

