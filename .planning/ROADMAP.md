# Roadmap: polint v1.1 Capability Fulfillment

## Current Milestone: v1.1 Capability Fulfillment

## Milestones

- [x] **v1.0 MVP** - repo-local static analysis framework for Go and TypeScript/JavaScript, shipped 2026-05-02. See [v1.0 roadmap archive](milestones/v1.0-ROADMAP.md), [requirements archive](milestones/v1.0-REQUIREMENTS.md), and [milestone audit](milestones/v1.0-MILESTONE-AUDIT.md).
- [ ] **v1.1 Capability Fulfillment** - make capabilities real for Go and TypeScript/JavaScript first, then add Python and Java through the same adapter contract.

## Current Status

v1.1 is active. Phase numbering continues after the archived v1.0 phase
directories, so this milestone starts at Phase 11.

## Milestone Goal

Fulfill polint's capability promise: when a rule declares needed analysis facts,
the engine plans, validates setup, harvests, caches, and exposes those facts
through the public SDK.

## Phase Summary

| Phase | Name | Goal | Requirements |
|-------|------|------|--------------|
| 11 | Capability-Driven Analysis Plan | Make `Capabilities` drive analysis, setup checks, and cache semantics. (In Progress: 1/3 plans executed.) | PLAN-01, PLAN-02, PLAN-03, PLAN-04 |
| 12 | CFG Facts for Go and TS/JS | Expose real per-function control-flow graphs through `RuleCtx`. | CFG-01, CFG-02, CFG-03, CFG-04 |
| 13 | Coverage Facts Import | Import Go and TS/JS coverage reports into public coverage facts. | COV-01, COV-02, COV-03, COV-04 |
| 14 | Resolved Imports and Module Graph | Resolve imports into file/package/module relationships. | MOD-01, MOD-02, MOD-03, MOD-04 |
| 15 | Direct Call Graph Facts | Expose direct call edges with resolution status and confidence. | CALL-01, CALL-02, CALL-03, CALL-04 |
| 16 | Symbols and References | Expose definitions, symbols, references, and stable symbol IDs. | SYM-01, SYM-02, SYM-03, SYM-04 |
| 17 | Test Suite Metrics | Provide reusable Go and TS/JS test-quality metrics. | TEST-01, TEST-02, TEST-03, TEST-04 |
| 18 | Python Adapter | Add Python with a declared initial capability subset. | PY-01, PY-02, PY-03, PY-04 |
| 19 | Java Adapter | Add Java with setup-aware packages, symbols, tests, and coverage. | JAVA-01, JAVA-02, JAVA-03, JAVA-04 |

## Phase Details

### Phase 11: Capability-Driven Analysis Plan

**Goal:** Make `Capabilities` operational instead of descriptive.

**Why:** Every later capability depends on the engine knowing what rules asked
for before analysis starts.

**Requirements:** PLAN-01, PLAN-02, PLAN-03, PLAN-04

**Plans:** 1/3 plans executed

Plans:
- [x] 11-01-PLAN.md — Internal AnalysisPlan contract and RuleCtx support view
- [ ] 11-02-PLAN.md — Runner, adapter, and cache integration
- [ ] 11-03-PLAN.md — Explain plan CLI, external proof, and docs

**Success criteria:**

1. Enabled rules are merged into a deterministic `AnalysisPlan`.
2. Go and TS/JS adapters receive the plan before harvesting optional facts.
3. Cache keys change when the resolved plan changes.
4. Missing or unsupported setup produces clear diagnostics or structured warnings.
5. `polint explain plan` shows the requested capabilities and setup checks.

### Phase 12: CFG Facts for Go and TS/JS

**Goal:** Expose real per-function control-flow graphs through the SDK.

**Why:** Rule authors need to reason about function paths without parsing ASTs
themselves.

**Requirements:** CFG-01, CFG-02, CFG-03, CFG-04

**Success criteria:**

1. Public CFG graph types are available through `polint::sdk::prelude::*`.
2. Go functions expose syntax-level CFGs for common branch and exit constructs.
3. TS/JS functions expose syntax-level CFGs through the same graph model.
4. `RuleCtx::cfg(function_id)` returns CFGs for analyzed functions.
5. `polint graph cfg` renders non-placeholder DOT output.

### Phase 13: Coverage Facts Import

**Goal:** Import external coverage reports into public coverage facts.

**Why:** Coverage lets rules combine static findings with CI/test evidence.

**Requirements:** COV-01, COV-02, COV-03, COV-04

**Success criteria:**

1. `.polint.toml` can configure Go and TS/JS coverage report paths.
2. Go `coverprofile` files map to repo-relative line/function/branch coverage where possible.
3. TS/JS LCOV files map to repo-relative line/function/branch coverage where possible.
4. Coverage facts include source and precision metadata.
5. Missing coverage setup is reported clearly when coverage is requested.

### Phase 14: Resolved Imports and Module Graph

**Goal:** Resolve syntactic imports into module/file/package relationships.

**Why:** Architecture rules need to know what imports point to, not just their
literal strings.

**Requirements:** MOD-01, MOD-02, MOD-03, MOD-04

**Success criteria:**

1. `ResolvedImportFact` records resolved targets or explicit unresolved reasons.
2. TS/JS imports resolve through project-aware resolver setup.
3. Go imports resolve through Go package/module metadata where available.
4. `RuleCtx::module_graph()` exposes module graph relationships.
5. `polint explain import` helps users debug import resolution.

### Phase 15: Direct Call Graph Facts

**Goal:** Expose direct call relationships with resolution status and confidence.

**Why:** Call graph facts enable architectural, dependency, and future
interprocedural rules.

**Requirements:** CALL-01, CALL-02, CALL-03, CALL-04

**Success criteria:**

1. Go and TS/JS call expressions produce `CallEdgeFact` entries.
2. Call facts include caller, callee text, span, resolution status, and confidence.
3. Call facts attach resolved targets when import/symbol facts make that possible.
4. `RuleCtx::calls_from(function_id)` exposes function-local call relationships.
5. `polint graph calls` renders useful call graph output.

### Phase 16: Symbols and References

**Goal:** Expose stable definitions, symbols, and references.

**Why:** This moves rules beyond string matching and gives later graph features a
shared semantic foundation.

**Requirements:** SYM-01, SYM-02, SYM-03, SYM-04

**Success criteria:**

1. Public symbol/reference fact types and IDs are available through the SDK.
2. Go symbols and references are populated from typed package information where setup exists.
3. TS/JS symbols and references are populated from Oxc semantic facts where setup exists.
4. `RuleCtx::references_to(symbol_id)` works for supported symbols.
5. Precision tiers distinguish exact, heuristic, unresolved, and ambiguous facts.

### Phase 17: Test Suite Metrics

**Goal:** Add reusable normalized test-quality metrics.

**Why:** Rules should not reimplement framework detection and evidence scoring.

**Requirements:** TEST-01, TEST-02, TEST-03, TEST-04

**Success criteria:**

1. `TestMetricFact` and related test evidence facts are public SDK types.
2. Go metrics aggregate existing `TestFact` data into reusable scores and counts.
3. TS/JS metrics detect common Jest/Vitest/Mocha structures and assertions.
4. Rules can query related test evidence by file and function.
5. Metrics state heuristic limits and avoid exact coverage claims.

### Phase 18: Python Adapter

**Goal:** Add Python through the shared adapter contract with an explicit initial capability tier.

**Why:** Python should reuse the same rule SDK and fact model rather than
creating a separate product surface.

**Requirements:** PY-01, PY-02, PY-03, PY-04

**Success criteria:**

1. Python file discovery, parsing, diagnostics, and source facts work through the shared pipeline.
2. Python exposes functions/classes, imports, literals, basic branches, test facts, and coverage import.
3. Python import/call uncertainty and optional interpreter/virtualenv setup are explicit.
4. Python example rule packs use only `polint::sdk::prelude::*`.
5. The language support matrix documents Python's supported capability tier.

### Phase 19: Java Adapter

**Goal:** Add Java through the shared adapter contract with setup-aware project facts.

**Why:** Java rules need packages, classpath-aware facts, test facts, and
coverage without leaking raw Java tooling into the SDK.

**Requirements:** JAVA-01, JAVA-02, JAVA-03, JAVA-04

**Success criteria:**

1. Java file discovery, parsing, diagnostics, and source facts work through the shared pipeline.
2. Java exposes packages/imports, classes/methods, literals, basic branches, test facts, and coverage import.
3. Classpath/build setup requirements are explicit when deeper facts are requested.
4. JaCoCo reports can become coverage facts.
5. Java example rule packs use only `polint::sdk::prelude::*`.

## Release Gate For Each Phase

Each phase should ship with:

- public fact types or graph types when new facts are introduced
- public `RuleCtx` accessors
- adapter implementation for the targeted language tier
- cache-key participation when harvested facts change
- docs under `docs/facts/` when new facts are public
- external temp-repo test using only `polint::sdk::prelude::*`
- CLI `explain` or `graph` support when useful for debugging
- clear unsupported/setup diagnostics instead of silent empty facts

## Traceability

All 36 v1.1 requirements are mapped in
[`REQUIREMENTS.md`](REQUIREMENTS.md#traceability). No v1.1 requirements are
currently unmapped.

## Source Material

- [`docs/roadmap/00_ROADMAP.md`](../docs/roadmap/00_ROADMAP.md)
- [`docs/CAPABILITY-FULFILLMENT-RESEARCH.md`](../docs/CAPABILITY-FULFILLMENT-RESEARCH.md)
