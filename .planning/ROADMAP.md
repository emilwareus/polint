# Roadmap: polint

## Milestones

- [x] **v1.0 MVP** - repo-local static analysis framework for Go and TypeScript/JavaScript, shipped 2026-05-02. Archive: [v1.0 roadmap](milestones/v1.0-ROADMAP.md).
- [x] **v1.1 Capability Fulfillment** - capability planning, resolved imports/module graph, and symbol/reference foundations for Go and TS/JS.
- [x] **v1.2 Static Analysis Engine Implementation** - private, validated, cache-aware, agent-extensible analysis engine substrate; 22 phases and 136 plans shipped 2026-05-27. Archive: [v1.2 roadmap](milestones/v1.2-ROADMAP.md).
- [x] **v1.3 Graph Engine Precision** - shared semantic graph, reachability/root semantics, Go RTA, JS/TS token/object models, adaptation, unknown taxonomy, budgets, and benchmark promotion gates. Archive: [v1.3 roadmap](milestones/v1.3-ROADMAP.md).
- [ ] 🚧 **v1.4 Policy Query Surface** - preview SDK views and typed query objects for realistic repo-local policies over calls, control flow, and data flow.

## Current Status

**Milestone:** v1.4 Policy Query Surface (active)
**Phases planned:** 8 (Phase 55 - Phase 62)
**Requirements coverage:** 33/33 mapped
**Granularity:** fine

Phase numbering continues from v1.3's last phase 54. v1.4 promotes a narrow policy-level SDK surface while keeping raw analysis internals private.

## Phases (v1.4)

- [ ] **Phase 55: SDK Query Vocabulary and Preview Contract** - Define preview views, query structs, pattern structs, capability derivation, and the "one public way" API contract.
- [ ] **Phase 56: Events and Calls Query Surface** - Implement `Events<'_>` and `Calls<'_>` policy queries over v1.3 refined calls, reachability roots, and unknown taxonomy.
- [ ] **Phase 57: Control-Flow Guard and Lifecycle Queries** - Implement `ControlFlow<'_>` guard and cleanup policies without exposing raw CFG/dominance graphs.
- [ ] **Phase 58: Data-Flow Source/Sink/Barrier Queries** - Promote `DataFlow<'_>` preview methods for forbidden flows and required barriers over bounded private path search.
- [ ] **Phase 59: Violation Evidence, Unknowns, and Cache Semantics** - Normalize violation results, diagnostic evidence, deterministic ordering, cache keys, and user-visible unknown/budget behavior.
- [ ] **Phase 60: Flagship Rule Templates and Agent Ergonomics** - Generate realistic policy templates and update README/examples/skill text around the same query-object syntax.
- [ ] **Phase 61: Public Docs and External SDK Validation** - Document every preview view/query and prove external repo-local rule usage through temp-repo tests.
- [ ] **Phase 62: Promotion Gate, Boundary Proof, and Closeout** - Enforce public-surface leak gates, full regression, deterministic checks, and milestone exit verification.

## Phase Details

### Phase 55: SDK Query Vocabulary and Preview Contract

**Goal:** Establish the public vocabulary and constraints before implementation so the milestone has one clear rule-authoring shape.

**Depends on:** v1.2/v1.3 private graph, calls, CFG, data-flow, evidence, unknown taxonomy, capability derivation, rule manifests.

**Requirements:** API-01, API-02, API-03, API-04, API-05, API-06

**Success Criteria:**
1. `Events<'_>`, `Calls<'_>`, `ControlFlow<'_>`, and `DataFlow<'_>` are exported from the SDK prelude as preview views and are constructible only through macro-derived fact-view parameters.
2. Public query structs exist for `ReachQuery`, `GuardQuery`, `LifecycleQuery`, and `FlowQuery` with `new(required...)`, explicit option fields, deterministic defaults, and no competing fluent/string/closure DSL.
3. Public pattern structs exist for events, sources, sinks, guards, and barriers with reviewed constructors for the flagship policy examples.
4. Capability derivation, rule manifests, and support diagnostics understand the new preview views and fail closed when setup is missing.
5. `Cfg<'_>` and `CallGraph<'_>` remain reserved low-level names; raw graph, solver, provider, and private ID types stay unreachable from supported public surfaces.

**Implementation notes:**
- Start with type definitions, docs comments, macro capability mapping, and capability diagnostics before wiring query behavior.
- Use plain structs and typed constructors. Avoid trait-heavy or generic APIs unless an existing SDK pattern requires them.
- Treat all preview names as public liabilities even if documented as preview.

### Phase 56: Events and Calls Query Surface

**Goal:** Let rules ask whether important calls/events are reachable from roots or trust boundaries without exposing call-graph internals.

**Depends on:** Phase 55; v1.3 refined-call projection; reachability roots; unknown taxonomy.

**Requirements:** CALL-01, CALL-02, CALL-03, CALL-04

**Success Criteria:**
1. `Events<'_>::matching(EventPattern)` returns deterministic semantic event matches for calls, writes, trust-boundary roots, and lifecycle events without leaking raw AST/MIR/graph IDs.
2. `Calls<'_>::forbidden_reachable(ReachQuery)` returns violations for raw APIs reachable from selected roots with root, path, callsite, target, precision, and unknown evidence.
3. `ReachQuery` supports root pattern, target pattern, package/module scope, tests inclusion, max depth, max paths, and minimum precision/confidence fields.
4. Fixtures cover raw reachable admin APIs, framework route roots, CLI roots, tests excluded/included, unresolved calls, and budget-exceeded paths.
5. Existing v1.3 precision floors and refined-call contracts remain intact.

**Example policies unlocked:**
- Raw database admin/client methods must not be reachable from HTTP handlers.
- Deprecated internal package APIs must not be reachable from production entrypoints.
- Dangerous shell/network/file APIs must not be reachable from unauthenticated roots.

### Phase 57: Control-Flow Guard and Lifecycle Queries

**Goal:** Let rules express "this event requires a guard" and "this acquired resource requires cleanup" policies over private CFG/dominance facts.

**Depends on:** Phase 55; Phase 56 event patterns; private CFG, dominance, postdominance, summaries, evidence.

**Requirements:** CTRL-01, CTRL-02, CTRL-03, CTRL-04

**Success Criteria:**
1. `ControlFlow<'_>::missing_guard(GuardQuery)` finds missing auth/validation/allowlist guards before sensitive events.
2. `ControlFlow<'_>::missing_cleanup(LifecycleQuery)` finds missing rollback/commit/close/unlock/end cleanup on success and error exits.
3. Query options support same-function and bounded-interprocedural modes without exposing raw CFG nodes or dominance graphs.
4. Violations include event spans, guard/cleanup candidates, uncovered path evidence, conservative unknowns, and budget status.
5. Fixtures cover auth-before-write, validation-before-money-move, transaction rollback, file close, lock unlock, and tracing span end.

**Example policies unlocked:**
- Balance or permission writes require an authorization guard.
- Money movement requires validation before persistence.
- Transactions opened with `Begin` must commit or rollback on every exit path.

### Phase 58: Data-Flow Source/Sink/Barrier Queries

**Goal:** Let rules express source-to-sink and required-sanitizer policies through `DataFlow<'_>` preview methods.

**Depends on:** Phase 55; Phase 56 events/calls; private data-flow facts, summaries, source/sink/model facts, evidence paths.

**Requirements:** FLOW-01, FLOW-02, FLOW-03, FLOW-04, FLOW-05

**Success Criteria:**
1. `DataFlow<'_>::forbidden(FlowQuery)` reports source-to-sink violations with optional barriers/sanitizers.
2. Required-barrier semantics cover request-to-shell validation, SSRF host allowlists, dangerous HTML escaping, and user-controlled path validation.
3. Built-in source/sink patterns cover HTTP inputs, route params, environment/secrets, PII-like identifiers, file paths, URLs, loggers, analytics, shell, SQL/query, HTML/raw JSX, and outbound network clients.
4. Flow queries use bounded private path search with ranked evidence paths, summary expansion handles, deterministic caps, and repeated-run stability.
5. Results expose exact/heuristic/unsupported/unknown/budget-exceeded states honestly, with heuristic wording in diagnostics and docs.

**Example policies unlocked:**
- Request values must not flow to `exec.Command` unless validated.
- Secrets must not flow to logs unless redacted.
- User-controlled URLs must not flow to HTTP clients unless allowlisted.
- Raw HTML sinks must receive escaped/sanitized values.

### Phase 59: Violation Evidence, Unknowns, and Cache Semantics

**Goal:** Make every query family report results in one diagnostic/evidence shape with deterministic cache-safe behavior.

**Depends on:** Phases 56-58; private evidence, unknown taxonomy, cache identity, diagnostics.

**Requirements:** EVID-01, EVID-02, EVID-03, EVID-04, EVID-05

**Success Criteria:**
1. Query methods return a consistent violation type with `diagnostic(rule_id, message)` and structured evidence projection to JSON/SARIF.
2. Evidence records query type, matched patterns, spans, path steps, precision/confidence/status, budgets, and unknown reasons.
3. Results are sorted/deduped deterministically across parallel execution, cache restore, provider-order shuffles, and repeated runs.
4. Query parameters, preview API versions, rule options, lifecycle inputs, solver budgets, and model/adaptation files participate in cache identity.
5. Setup gaps, unsupported semantics, and budget exhaustion are visible to users and do not create silent false negatives.

**Implementation notes:**
- Keep rule-specific wording in the rule. Keep path/precision/unknown structure in the shared violation evidence.
- Prefer one shared evidence schema over per-query bespoke JSON.

### Phase 60: Flagship Rule Templates and Agent Ergonomics

**Goal:** Turn the API into concrete value users can copy immediately.

**Depends on:** Phases 56-59.

**Requirements:** TPL-01, TPL-02, TPL-03, TPL-04, TPL-05

**Success Criteria:**
1. `polint new-rule` can scaffold request-to-shell, secret-log, PII-analytics, sensitive-write-guard, transaction-cleanup, raw-reachable-api, SSRF, dangerous-HTML, unsafe-deserialization, and user-controlled-file-path templates.
2. Every template uses the same query-object style and imports only `polint::sdk::prelude::*`.
3. Templates carry honest heuristic wording where source/sink detection is heuristic.
4. README, examples, generated skill text, and docs position templates as repo-local starting points, not bundled default rules.
5. Template fixtures prove each scaffold compiles and produces an expected diagnostic in a temp repo.

**Template syntax target:**

```rust
let mut query = FlowQuery::new(SourcePattern::http_request(), SinkPattern::call("exec.Command"));
query.barriers = BarrierPattern::call_any(["validate_command"]);
query.max_paths = 10;

for violation in flow.forbidden(query) {
    ctx.report(violation.diagnostic(ctx.rule_id(), "Request data reaches shell execution."));
}
```

### Phase 61: Public Docs and External SDK Validation

**Goal:** Prove the preview surface is usable by outside rule authors and documented honestly.

**Depends on:** Phases 55-60.

**Requirements:** VAL-01, VAL-02

**Success Criteria:**
1. `docs/facts/` includes preview pages for events, calls, control-flow, data-flow, patterns, query structs, violation evidence, precision tiers, unknowns, budgets, and limits.
2. Temp-repo tests cover each preview view and query family through generated `.polint/rules`, public SDK imports only, `polint::runner::run_cli`, real facts, and `polint check --format json` assertions.
3. Docs include the flagship policy examples and explicitly label heuristic behavior.
4. Capability support and `polint inspect`/manifest output show preview status consistently.

**Implementation notes:**
- These tests are the external-consumer contract. They should not reach into `polint::core`, parser adapters, analysis modules, or test helpers.

### Phase 62: Promotion Gate, Boundary Proof, and Closeout

**Goal:** Enforce the v1.4 exit gates and prove the policy query surface is useful without leaking internals.

**Depends on:** All earlier v1.4 phases.

**Requirements:** VAL-03, VAL-04

**Success Criteria:**
1. Public-surface leak tests prove raw CFG, call graph, semantic graph, data-flow graph, solver, provider, `AnalysisDb`, and private IDs are unreachable from supported SDK/CLI/runner/docs/skill surfaces.
2. Full workspace formatting, clippy, tests, temp-repo SDK tests, cache invalidation tests, docs/example smoke tests, and deterministic repeated-run checks pass.
3. A milestone audit records which preview APIs are ready, which remain preview-limited, and which future stabilization items move to v1.5.
4. `PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`, and `STATE.md` are updated with final traceability and next-step guidance.

## Phase Progress

| Phase | Name | Plans Complete | Status | Completed |
|-------|------|----------------|--------|-----------|
| 55 | SDK Query Vocabulary and Preview Contract | 0/0 | Planned | - |
| 56 | Events and Calls Query Surface | 0/0 | Planned | - |
| 57 | Control-Flow Guard and Lifecycle Queries | 0/0 | Planned | - |
| 58 | Data-Flow Source/Sink/Barrier Queries | 0/0 | Planned | - |
| 59 | Violation Evidence, Unknowns, and Cache Semantics | 0/0 | Planned | - |
| 60 | Flagship Rule Templates and Agent Ergonomics | 0/0 | Planned | - |
| 61 | Public Docs and External SDK Validation | 0/0 | Planned | - |
| 62 | Promotion Gate, Boundary Proof, and Closeout | 0/0 | Planned | - |

## Parallel-Eligible Phases

- **Phase 56 -> Phase 57/58:** Events/calls should land before control-flow and data-flow policies because both reuse event/pattern vocabulary.
- **Phase 57 and Phase 58:** Control-flow and data-flow query implementation may proceed in parallel after Phase 56 if write ownership is split cleanly.
- **Phase 60 and Phase 61:** Template work and docs/tests can overlap after query semantics stabilize, but temp-repo tests should be the final authority.

## Promotion Discipline

- New public names are preview policy views, not raw analysis internals.
- `Cfg<'_>` and `CallGraph<'_>` stay reserved unless a separate explicit promotion phase changes that.
- `DataFlow<'_>` becomes a policy query view, not a raw graph view.
- Rules continue to consume typed fact-view parameters through `#[polint::rule]`; do not reintroduce broad fact access through `RuleCtx`.
- Capability names must stay honest: unsupported or setup-missing hard capabilities produce capability diagnostics and the rule does not run with placeholder facts.
- Query APIs must expose precision, unknown, and budget state rather than claiming exact whole-program coverage.

## Next Up

**Phase 55: SDK Query Vocabulary and Preview Contract** - define the preview API contract before wiring behavior.

Suggested command:

```bash
/gsd-discuss-phase 55 /Users/emilwareus/conductor/workspaces/exlint/louisville-v1
```

For a direct implementation plan:

```bash
/gsd-plan-phase 55 /Users/emilwareus/conductor/workspaces/exlint/louisville-v1
```

---
*Roadmap created: 2026-06-20*
