# Phase 55: SDK Query Vocabulary and Preview Contract - Context

**Gathered:** 2026-06-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 55 defines the public preview vocabulary for v1.4 policy queries. It should add the SDK names, capability derivation, manifest metadata, docs stubs, and fail-closed diagnostics that downstream phases will implement. It should not implement full event/call/control-flow/data-flow query behavior yet, and it must not expose raw CFG, call graph, semantic graph, data-flow graph, solver, provider, `AnalysisDb`, or private ID types.

</domain>

<decisions>
## Implementation Decisions

### Public Vocabulary Boundary
- **D-01:** Promote policy-level preview views, not raw analysis views. The public view set for this phase is `Events<'_>`, `Calls<'_>`, `ControlFlow<'_>`, and `DataFlow<'_>`.
- **D-02:** Keep existing low-level `Cfg<'_>` and `CallGraph<'_>` reserved and unsupported. Do not turn them into aliases for the new policy views.
- **D-03:** Treat `DataFlow<'_>` as the future policy query view, not a raw data-flow graph view. Its public methods should eventually be `forbidden(FlowQuery)` and required-barrier style methods, not node/edge traversal.

### Capability Support Semantics
- **D-04:** Phase 55 may make preview names compile and appear in manifests, but rules must fail closed until each query family is backed by real facts. No placeholder facts, no empty success, and no silent no-op execution.
- **D-05:** Use distinct capability names for the policy surface: `events`, `calls`, `control_flow`, and `dataflow`. `call_graph` remains the reserved raw graph capability; `cfg` remains the reserved raw CFG capability.
- **D-06:** Capability diagnostics must stay honest. Unsupported or setup-missing preview capabilities should produce `polint/capability` diagnostics with docs links and must prevent the requesting rule from running.

### Module Organization
- **D-07:** Keep fact-view structs canonical under `polint::sdk::facts::*` so the existing `#[polint::rule]` macro rules still apply. Add `Events<'_>`, `Calls<'_>`, and `ControlFlow<'_>` there; update `DataFlow<'_>` in place.
- **D-08:** Put query structs, pattern structs, policy status/precision enums, and violation/result helper types in a new `polint::sdk::policy` module, then re-export the supported preview surface through `polint::sdk::prelude::*`.
- **D-09:** Avoid a new public crate, new public graph module, or broad barrel export of internals. Public re-exports should be explicit and reviewed as API additions.

### Pattern and Query Syntax
- **D-10:** There is one public authoring style: construct a plain query object, set explicit option fields, run one method on a typed view, then report returned violations. Do not add fluent builders, closure filters, or string query languages.
- **D-11:** Query structs should use `Query::new(required...)` constructors plus explicit fields with deterministic defaults. Required inputs belong in `new`; optional knobs stay as named fields.
- **D-12:** Pattern structs should use typed constructors such as `EventPattern::call`, `EventPattern::write_field`, `SourcePattern::http_request`, `SinkPattern::call`, `GuardPattern::call_any`, and `BarrierPattern::call_any`.
- **D-13:** Default matching should be simple and predictable: exact canonical strings and explicit lists first. Do not start with regex-heavy or parser-specific pattern semantics. If broader matching is needed, add one small named pattern primitive later rather than many equivalent spellings.
- **D-14:** Preview docs must define canonical name expectations. Rule authors should not need to know raw AST, MIR, CFG, solver, or graph node IDs to write patterns.

### Validation Gates
- **D-15:** Phase 55 validation proves compile/manifest/capability behavior only. Full query-result behavior belongs to Phases 56-59.
- **D-16:** Add at least one external temp-repo style test for preview view signatures and query/pattern construction using only `polint::sdk::prelude::*` and `polint::runner::run_cli`.
- **D-17:** Update public-surface leak gates deliberately for the new preview names and document the promotion in `docs/API-VISIBILITY-PLAN.md`. The same gates must continue proving raw internals are not reachable.
- **D-18:** Update `polint facts list`, docs under `docs/facts/`, macro capability tests, and reserved capability diagnostics together. Do not leave public names undocumented or unsupported behavior ambiguous.

### the agent's Discretion
- The planner may choose exact enum/struct field names when the intent above is preserved.
- The planner may split implementation into multiple plans if macro, SDK, CLI metadata, docs, and tests have cleaner ownership boundaries.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone and Phase Scope
- `.planning/ROADMAP.md` — Phase 55 goal, dependencies, requirements, success criteria, and promotion discipline.
- `.planning/REQUIREMENTS.md` — v1.4 API design contract plus API-01 through API-06.
- `.planning/PROJECT.md` — project value, public API discipline, current milestone target features, and out-of-scope boundaries.

### Public API and Capability Contracts
- `docs/API-VISIBILITY-PLAN.md` — public/private API discipline and promotion record location for intentional new SDK names.
- `docs/facts/capability-plans.md` — reserved capability behavior and docs link used by capability diagnostics.
- `docs/facts/data-flow.md` — current `DataFlow<'_>` reserved status that Phase 55 must update honestly.

### SDK and Macro Integration Points
- `crates/polint/src/sdk/facts.rs` — canonical fact-view structs and `FactView` implementations.
- `crates/polint/src/sdk/mod.rs` — prelude exports and hidden generated-rule support.
- `crates/polint-macros/src/lib.rs` — fact-view parameter validation and capability-name derivation.
- `crates/polint/src/analysis_plan.rs` — capability support status, unsupported/setup-missing diagnostics, and fail-closed rule execution behavior.
- `crates/polint/src/rule_manifest.rs` — generated rule manifest capability support rows.

### CLI and Gate Integration Points
- `crates/polint/src/cli/mod.rs` — `facts list`, `inspect`, and public fact-view metadata.
- `crates/polint/tests/cli.rs` — unsupported reserved capability tests and temp-repo CLI behavior.
- `crates/polint/tests/public_surface_leak.rs` — public prelude allow-list and raw-internal leak gate.
- `tests/fixtures/public-surface-leak-probe/src/lib.rs` — external probe crate proving what `polint::sdk::prelude::*` exposes.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/sdk/facts.rs`: Existing borrowed fact-view pattern is a good fit for `Events<'_>`, `Calls<'_>`, `ControlFlow<'_>`, and updated `DataFlow<'_>`.
- `crates/polint-macros/src/lib.rs`: The macro already enforces canonical SDK paths and placeholder lifetimes. Extend this path rather than creating a new rule-authoring mechanism.
- `crates/polint/src/analysis_plan.rs`: Existing capability support machinery already supports `Supported`, `Unsupported`, and `SetupMissing` states with diagnostics.
- `crates/polint/src/cli/mod.rs`: Existing `PublicFactView` metadata can represent preview/reserved/stable capability status.
- `crates/polint/tests/public_surface_leak.rs`: Existing gate can be extended for deliberate preview additions while continuing to block private type leaks.

### Established Patterns
- Public fact views are `Clone + Copy` borrowed wrappers over `AnalysisDb` and are constructed through hidden `FactView`.
- Rule functions import `polint::sdk::prelude::*`, request fact views as typed parameters, and rely on macro-derived capabilities.
- Unsupported hard capabilities produce `polint/capability` diagnostics and the rule should not execute.
- Public CLI/fact docs are considered product contracts. Every visible capability status needs docs and tests.
- Internal analysis modules stay `pub(crate)`; public re-exports are narrow and intentional.

### Integration Points
- Add `sdk::policy` as a public SDK module for query/pattern/result vocabulary.
- Extend `sdk::facts` and `sdk::prelude` for preview fact views and policy types.
- Extend macro capability mapping for `Events`, `Calls`, `ControlFlow`, and `DataFlow`.
- Extend `Capabilities`, `AnalysisPlan`, rule manifests, CLI facts metadata, docs, and tests in one coordinated slice.
- Update leak-gate allow-lists with a documented promotion record.

</code_context>

<specifics>
## Specific Ideas

Target syntax remains the simple rule shape from `.planning/REQUIREMENTS.md`:

```rust
let mut query = FlowQuery::new(
    SourcePattern::secret_like(["token", "password", "apiKey"]),
    SinkPattern::logger(),
);
query.barriers = BarrierPattern::call_any(["redact", "mask_secret"]);
query.max_paths = 20;

for violation in flow.forbidden(query) {
    ctx.report(violation.diagnostic(
        ctx.rule_id(),
        "Secret-like value reaches logging without redaction.",
    ));
}
```

The API should feel closer to Go than JS: one obvious way, plain structs, named fields, predictable defaults, and no competing DSLs.

</specifics>

<deferred>
## Deferred Ideas

- Full `Events<'_>` and `Calls<'_>` query behavior belongs to Phase 56.
- Full `ControlFlow<'_>` guard/lifecycle behavior belongs to Phase 57.
- Full `DataFlow<'_>` source/sink/barrier behavior belongs to Phase 58.
- Shared violation evidence and cache semantics belong to Phase 59.
- Generated rule templates belong to Phase 60.
- Public docs and external SDK validation broaden in Phase 61.
- Final promotion/boundary proof belongs to Phase 62.

</deferred>

---

*Phase: 55-SDK Query Vocabulary and Preview Contract*
*Context gathered: 2026-06-20*
