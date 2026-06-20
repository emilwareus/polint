# Phase 56: Events and Calls Query Surface - Context

**Gathered:** 2026-06-20
**Status:** Ready for planning
**Mode:** Autonomous discuss

<domain>
## Phase Boundary

Phase 56 promotes only the event and call policy-query methods from Phase 55
preview vocabulary to provider-backed behavior:

- `Events<'_>::matching(EventPattern)`
- `Calls<'_>::forbidden_reachable(ReachQuery)`

The goal is to let repo-local rules ask whether important semantic events and
forbidden calls are reachable from roots or trust boundaries without exposing
raw call-graph, AST, MIR, CFG, solver, `AnalysisDb`, or internal ID types.

`ControlFlow<'_>` and policy-level `DataFlow<'_>` remain fail-closed until their
later phases. `Cfg<'_>` and `CallGraph<'_>` remain reserved raw capabilities.

</domain>

<decisions>
## Implementation Decisions

### API Shape

- Preserve the Phase 55 "one good way" rule-authoring style: construct a plain
  query object, set explicit public fields, call one method on a typed fact view,
  then report returned `PolicyViolation`s.
- Do not add builders, closure filters, regex mini-languages, raw graph handles,
  public traversal APIs, public node IDs, or alternate spellings for the same
  query behavior.
- Add only the minimal public accessors/helpers needed for returned violations.
  Pattern internals should stay implementation detail unless a public getter is
  necessary for rule authors.

### Event Matching Semantics

- `EventPattern::call("canonical.target")` matches deterministic call events
  backed by existing direct/refined call facts.
- `EventPattern::write_field("canonical.field")` should use existing syntactic or
  semantic write facts if present. If no backed write-event family exists yet,
  return no matches rather than invent placeholder facts, and document the limit.
- Event results must expose file/range, status, precision, and evidence through
  `PolicyViolation::diagnostic`; they must not expose raw AST/MIR/graph IDs.
- Matching should be exact canonical string matching first. Broader matching
  primitives are deferred unless already represented by existing public query
  fields.

### Reachability Semantics

- `Calls<'_>::forbidden_reachable(ReachQuery)` should run over the v1.3 refined
  call projection and reachability roots/marks already stored in `AnalysisDb`.
- The query target is an `EventPattern`; Phase 56 supports call targets first.
  Non-call target kinds should fail honestly through an unsupported/unknown
  violation or documented empty behavior, not panic.
- Root constraints come from `ReachQuery::roots`. Empty roots should mean the
  supported root set from existing reachability facts, not arbitrary file-wide
  scanning.
- Root patterns should match stable public descriptors for entrypoints/trust
  boundaries/lifecycle roots where existing facts provide them. If the current
  facts cannot express a requested root kind, surface unknown/setup/budget
  evidence rather than claiming exact absence.
- Apply `include_tests`, `max_depth`, `max_paths`, and `minimum_precision`
  deterministically. Query output must be stable across runs.

### Unknowns, Budgets, And Precision

- Preserve v1.3 precision floors. Reachability facts must not be upgraded to
  exact unless the underlying provider already supports that precision.
- Map refined-call statuses such as unresolved, ambiguous, unsupported,
  setup-missing, rejected, and budget-exceeded into `PolicyStatus` and diagnostic
  evidence.
- Budget exhaustion should be visible to rules as a returned violation/evidence
  row or status-bearing result, not as silent truncation.
- Rules requesting `events` or `calls` should execute only after those
  capabilities are backed by real facts. `control_flow` and `dataflow` should
  still produce `polint/capability` diagnostics and block execution.

### Capability And Documentation

- Mark `events` and `calls` as supported in the analysis-plan capability view
  only when their public methods are backed by real data.
- Keep `polint facts list` honest: `events` and `calls` can remain preview
  stability while support changes from fail-closed to provider-backed.
- Update docs under `docs/facts/events.md` and `docs/facts/calls.md` to describe
  current limits, canonical string matching, unknown evidence, and examples.

### Test Strategy

- Add at least one temp-repo style CLI test where an outside rule imports only
  `polint::sdk::prelude::*`, requests `Events<'_>` and/or `Calls<'_>`, receives
  real results, and reports JSON diagnostics through `polint check --format json`.
- Update the Phase 55 fail-closed temp-repo test so it still proves
  `ControlFlow<'_>` and `DataFlow<'_>` block rule execution without expecting
  `events`/`calls` to stay unsupported.
- Cover raw reachable admin/client API, framework/entry root behavior, tests
  excluded and included, unresolved calls, and max-path/depth budget evidence as
  much as existing fixtures make practical.

</decisions>

<canonical_refs>
## Canonical References

- `.planning/ROADMAP.md` - Phase 56 goal, dependencies, and success criteria.
- `.planning/REQUIREMENTS.md` - CALL-01 through CALL-04.
- `.planning/phases/55-sdk-query-vocabulary-and-preview-contract/55-CONTEXT.md`
  - public vocabulary and syntax decisions.
- `.planning/phases/55-sdk-query-vocabulary-and-preview-contract/55-VERIFICATION.md`
  - Phase 55 boundaries and verification status.
- `crates/polint/src/sdk/facts.rs` - fact-view structs and Phase 55 preview
  methods.
- `crates/polint/src/sdk/policy.rs` - public query, pattern, and violation
  vocabulary.
- `crates/polint/src/analysis_plan.rs` - capability support and fail-closed
  diagnostics.
- `crates/polint/src/core/mod.rs` - stored refined-call, reachability, entrypoint,
  trust-boundary, and diagnostic facts.
- `crates/polint/src/analysis/refined_calls/` - v1.3 refined-call projection.
- `crates/polint/src/analysis/reachability/` - roots, reachability marks, status,
  precision, and unknown taxonomy.
- `crates/polint/tests/cli.rs` - temp-repo tests for external rule authoring
  behavior.
- `docs/facts/events.md` and `docs/facts/calls.md` - public docs to update.

</canonical_refs>

<code_context>
## Existing Code Insights

- Phase 55 already added `Events<'_>`, `Calls<'_>`, `ReachQuery`,
  `EventPattern`, `PolicyViolation`, `PolicyStatus`, and `PolicyPrecision` to
  the SDK prelude.
- `Events::matching` and `Calls::forbidden_reachable` currently call
  `preview_query_unavailable(...)`, so support must be promoted together with
  capability status changes.
- `EventPattern` currently stores private kind/value fields. Internal matching
  code will need crate-private accessors or helper methods without broadening the
  public API unnecessarily.
- `PolicyViolation` currently exposes status, precision, and diagnostic
  conversion. Provider-backed query code will need a crate-private constructor
  and may need public evidence accessors if tests or future users need to inspect
  results before reporting.
- `AnalysisPlan::support_for` currently treats `events` and `calls` as
  unsupported preview capabilities. `control_flow` and `dataflow` should remain
  unsupported in this phase.
- `AnalysisDb` stores refined-call edges, reachability roots/marks, entrypoint
  facts, and trust-boundary facts that should be used as the private backing
  model instead of creating a raw public call graph.

</code_context>

<specifics>
## Specific Rule Syntax To Preserve

```rust
#[polint::rule(
    id = "local/no-admin-from-routes",
    description = "Raw admin client must not be reachable from request handlers",
    severity = "error"
)]
pub(crate) fn no_admin_from_routes(ctx: &mut RuleCtx<'_>, calls: Calls<'_>) -> RuleResult {
    let mut query = ReachQuery::new(EventPattern::call("db.AdminClient.deleteUser"));
    query.roots = vec![EventPattern::call("http.route")];
    query.include_tests = false;
    query.max_depth = 12;
    query.max_paths = 10;
    query.minimum_precision = PolicyPrecision::Conservative;

    for violation in calls.forbidden_reachable(query) {
        ctx.report(violation.diagnostic(
            ctx.rule_id(),
            "Raw admin client is reachable from a request handler.",
        ));
    }

    Ok(())
}
```

The rule author sees policy concepts only: roots, targets, paths, precision, and
diagnostic evidence. They do not see internal graph nodes or solver details.

</specifics>

<deferred>
## Deferred Ideas

- `ControlFlow<'_>` guard and lifecycle queries remain Phase 57.
- `DataFlow<'_>` source/sink/barrier queries remain Phase 58.
- Shared richer evidence/cache semantics remain Phase 59 unless required to make
  Phase 56 results honest.
- Generated templates using `Calls<'_>` remain Phase 60.
- Final docs/promotions/boundary proof remain Phases 61-62.

</deferred>

---

*Phase: 56-Events and Calls Query Surface*
*Context gathered: 2026-06-20*
