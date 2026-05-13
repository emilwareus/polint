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

The long-term direction is a fully capable static-analysis platform with a
complete graph representation of a codebase: module/dependency graph, symbol
graph, call graph, per-function CFG, coverage/test-evidence links, dataflow,
taint, and interprocedural summaries. v1.1 should advance that destination in
sequenced, truthful slices rather than trying to build the whole analyzer at
once.

## Phase Summary

| Phase | Name | Goal | Requirements |
|-------|------|------|--------------|
| 11 | Capability-Driven Analysis Plan | Make `Capabilities` drive analysis, setup checks, and cache semantics. (Complete: 3/3 plans executed on 2026-05-09.) | PLAN-01, PLAN-02, PLAN-03, PLAN-04 |
| 12 | Resolved Imports and Module Relationships | Resolve syntactic imports into module/file/package relationships. (Complete: 5/5 plans executed on 2026-05-11.) | MOD-01, MOD-02, MOD-03, MOD-04 |
| 13 | 3/6 | In Progress|  |
| 14 | Direct and Resolved Call Facts | Add call edges with resolution status and confidence. | CALL-01, CALL-02, CALL-03, CALL-04 |
| 15 | Control-Flow Facts for Go and TS/JS | Add typed SDK views for per-function control flow. | CFG-01, CFG-02, CFG-03, CFG-04 |
| 16 | Coverage Facts Import | Import Go and TS/JS coverage reports into public coverage facts. | COV-01, COV-02, COV-03, COV-04 |
| 17 | Test Suite Metrics | Provide reusable Go and TS/JS test-quality metrics. | TEST-01, TEST-02, TEST-03, TEST-04 |
| 18 | Python Adapter | Add Python with a declared initial capability subset. | PY-01, PY-02, PY-03, PY-04 |
| 19 | Java Adapter | Add Java with setup-aware packages, symbols, tests, and coverage. | JAVA-01, JAVA-02, JAVA-03, JAVA-04 |

## Phase Details

### Phase 11: Capability-Driven Analysis Plan

**Goal:** Make `Capabilities` operational instead of descriptive.

**Why:** Every later capability depends on the engine knowing what rules asked
for before analysis starts.

**Requirements:** PLAN-01, PLAN-02, PLAN-03, PLAN-04

**Plans:** 3/3 plans complete

Plans:
- [x] 11-01-PLAN.md — Internal AnalysisPlan contract and RuleCtx support view
- [x] 11-02-PLAN.md — Runner, adapter, and cache integration
- [x] 11-03-PLAN.md — Capability diagnostics, external proof, and docs

**Success criteria:**

1. Enabled rules are merged into a deterministic `AnalysisPlan`.
2. Go and TS/JS adapters receive the plan before harvesting optional facts.
3. Cache keys change when the resolved plan changes.
4. Missing or unsupported setup produces clear diagnostics or structured warnings.
5. `polint check` reports unsupported or setup-missing requested capabilities clearly.

### Phase 12: Resolved Imports and Module Relationships

**Goal:** Resolve syntactic imports into module/file/package relationships.

**Architecture:**
[`docs/roadmap/12_RESOLVED_IMPORTS_MODULE_GRAPH_ARCHITECTURE.md`](../docs/roadmap/12_RESOLVED_IMPORTS_MODULE_GRAPH_ARCHITECTURE.md)

**Why:** Architecture rules and codebase-understanding agents need to know what
imports point to, not just their literal strings.

**Requirements:** MOD-01, MOD-02, MOD-03, MOD-04

**Plans:** 5/5 plans complete

Plans:
- [x] 12-01-PLAN.md — Core fact model, SDK views, and macro capability contract
- [x] 12-02-PLAN.md — Project-wide module graph provider and runner wiring
- [x] 12-03-PLAN.md — TypeScript/JavaScript resolver integration
- [x] 12-04-PLAN.md — Go package metadata resolver integration
- [x] 12-05-PLAN.md — External SDK proof and public fact documentation

**Success criteria:**

1. `ResolvedImportFact` records resolved targets or explicit unresolved reasons.
2. TS/JS imports resolve through project-aware resolver setup.
3. Go imports resolve through Go package/module metadata where available.
4. A typed SDK view exposes module relationship facts.
5. Unresolved imports are visible to rules with explicit reasons.

### Phase 13: Symbols and References

**Goal:** Expose stable definitions, symbols, and references.

**Why:** This moves rules beyond string matching and gives call graphs,
ownership rules, exported API checks, and agent navigation a shared semantic
identity model.

**Requirements:** SYM-01, SYM-02, SYM-03, SYM-04

**Success criteria:**

1. Public symbol/reference fact types and IDs are available through the SDK.
2. Go symbols and references are populated from typed package information where setup exists.
3. TS/JS symbols and references are populated from Oxc semantic facts where setup exists.
4. A typed SDK view exposes references for supported symbols.
5. Precision tiers distinguish exact, heuristic, unresolved, and ambiguous facts.

### Phase 14: Direct and Resolved Call Facts

**Goal:** Expose call relationships with resolution status and confidence.

**Why:** Call facts enable architecture rules, dependency reasoning, impact
analysis, and future interprocedural analysis.

**Requirements:** CALL-01, CALL-02, CALL-03, CALL-04

**Success criteria:**

1. Go and TS/JS call expressions produce `CallEdgeFact` entries.
2. Call facts include caller, callee text, span, resolution status, and confidence.
3. Call facts attach resolved targets when import/symbol facts make that possible.
4. A typed SDK view exposes function-local call relationships.
5. External-consumer tests prove rules can read direct call facts through the public SDK.

### Phase 15: Control-Flow Facts for Go and TS/JS

**Goal:** Add real per-function control-flow facts through the SDK.

**Why:** Rule authors need to reason about function paths without parsing ASTs
themselves, and dataflow needs a real intra-procedural graph foundation.

**Requirements:** CFG-01, CFG-02, CFG-03, CFG-04

**Success criteria:**

1. Public control-flow fact types are available through `polint::sdk::prelude::*`.
2. Go functions expose syntax-level CFGs for common branch and exit constructs.
3. TS/JS functions expose syntax-level control-flow facts through the same fact model.
4. A typed SDK view returns control-flow facts for analyzed functions.
5. External-consumer tests prove rules can read CFG facts through the public SDK.

### Phase 16: Coverage Facts Import

**Goal:** Import external coverage reports into public coverage facts.

**Why:** Coverage lets rules combine static findings with CI/test evidence.

**Requirements:** COV-01, COV-02, COV-03, COV-04

**Success criteria:**

1. `.polint.toml` can configure Go and TS/JS coverage report paths.
2. Go `coverprofile` files map to repo-relative line/function/branch coverage where possible.
3. TS/JS LCOV files map to repo-relative line/function/branch coverage where possible.
4. Coverage facts include source and precision metadata.
5. Missing coverage setup is reported clearly when coverage is requested.

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

- public typed fact views when new facts are introduced
- SDK query methods on those typed views
- adapter implementation for the targeted language tier
- cache-key participation when harvested facts change
- docs under `docs/facts/` when new facts are public
- external temp-repo test using only `polint::sdk::prelude::*`
- no new CLI/debug surface unless it is deliberately promoted as supported product behavior
- clear unsupported/setup diagnostics instead of silent empty facts

## Traceability

All 36 v1.1 requirements are mapped in
[`REQUIREMENTS.md`](REQUIREMENTS.md#traceability). No v1.1 requirements are
currently unmapped.

## Source Material

- [`docs/roadmap/00_ROADMAP.md`](../docs/roadmap/00_ROADMAP.md)
- [`docs/CAPABILITY-FULFILLMENT-RESEARCH.md`](../docs/CAPABILITY-FULFILLMENT-RESEARCH.md)
