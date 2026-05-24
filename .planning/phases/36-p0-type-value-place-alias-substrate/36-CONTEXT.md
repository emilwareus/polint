# Phase 36: P0 Type/Value/Place/Alias Substrate - Context

**Gathered:** 2026-05-24
**Status:** Ready for planning
**Mode:** `$gsd-discuss-phase 36 --auto`

<domain>
## Phase Boundary

Phase 36 delivers the first private precision substrate for normalized type, value, allocation, access-path, local narrowing, points-to-constraint, and alias-answer facts. It builds on the existing semantic MIR, place identity, CFG/control dependence, direct calls, abstract domains, summaries, extension sink, and framework entrypoint layers. The goal is to make later refined call graph, data-flow, slicing, benchmark, and public SDK phases consume honest internal precision facts instead of ad hoc helper logic.

This phase does **not** promote public `Types<'_>`, `Values<'_>`, or `Aliases<'_>` SDK views; does not make whole-repo points-to mandatory for baseline `polint check`; does not add refined call graph providers; does not wire full data-flow source/sink propagation; and does not claim exact alias coverage for dynamic JS/TS, reflection, generated APIs, or setup-missing language facts. Phase 37 consumes this substrate for refined call graph providers, Phase 38 consumes it for local plus summary-projected data flow, Phase 39 consumes it for evidence/path explanations, and Phase 41 decides which validated typed views become public.

</domain>

<decisions>
## Implementation Decisions

### Fact Family Scope and Layering

- **D-01:** Implement Phase 36 as a layered private substrate, not as a single "alias analysis" pass. The planned fact families are: `TypeFact`, `NarrowedTypeFact`, `ValueFact`, `AllocationTokenFact`, `AccessPathFact`, `PointsToConstraintFact`, `PointsToSetFact` or equivalent solver output, `AliasAnswerFact`, and explicit unresolved/unsupported precision rows.
- **D-02:** Reuse and extend the existing `PlaceFact` model from `analysis::places` instead of replacing it. Existing stable place IDs from semantic MIR remain the identity anchor. Phase 36 may add richer root/projection/access-path facts, but must preserve existing MIR, calls, domains, summaries, and entrypoint behavior.
- **D-03:** Store type/value/allocation/alias rows in new crate-private analysis modules, likely under `analysis::types`, `analysis::values`, `analysis::access_paths`, `analysis::points_to`, and `analysis::aliases`, or a similarly clear layout. Keep all new types `pub(crate)` unless a later promotion phase explicitly widens them.
- **D-04:** Fact rows must carry stable keys, run-local dense IDs, language, file/function/body references where applicable, provenance, precision, confidence/status, validation status, and explicit unknown/unsupported/setup-missing/budget-exceeded states.
- **D-05:** Type/value/place/alias facts should be normalized and sorted by stable key before storage, metadata assignment, debug serialization, eval observation, and cache digesting. Do not let parser traversal order or hash map order determine IDs or output.

### Type and Narrowing Facts

- **D-06:** Type facts distinguish at least declared, inferred/resolved, narrowed-at-CFG-location, extension-provided, unknown, unsupported, and setup-missing phases. These phases are separate evidence rows, not overwritten strings on a single subject.
- **D-07:** The shared type envelope should support primitive/literal/nullish types, function/callable signatures, class/object/module types, nominal IDs, structural shape IDs, union/intersection-like sets where supported, generics/placeholders, and explicit `Any`/`Unknown`/`Unsupported` distinctions.
- **D-08:** Local narrowing is driven by the existing CFG and abstract-domain observations. First-tier narrowing should cover nullish checks, truthiness, strict equality/literal guards, `typeof`, `instanceof`, `in`/property checks where available, optional chaining, receiver narrowing, and Go nil/interface/pointer receiver facts where evidence exists.
- **D-09:** Official language tooling may be used as an input when it is the language compatibility authority, especially Go `go/types`/`go/packages` style facts and TypeScript compiler-compatible semantics. Any such output must be normalized into polint-owned facts with setup/config/toolchain digests and never exposed as raw tool output.
- **D-10:** Unsupported dynamic constructs must produce bounded unknown facts rather than optimistic exact facts. Examples include JS `eval`, dynamic property keys, proxies, monkeypatching-like behavior, reflection, generated APIs, unresolved framework dispatch, missing TS/Go setup, and budget exhaustion.

### Value, Allocation, and Access Path Facts

- **D-11:** Value facts start with high-yield P0 domains: null/undefined/nil, booleans/truthiness, literal strings/numbers where already available, function objects, class/constructor objects, module namespace objects, object/array/composite-literal allocation tokens, receiver values, and call-return values.
- **D-12:** Allocation tokens are stable internal identities for object-like values, function literals, class constructors, composite literals, module namespaces, closures, and synthetic framework/extension-modeled objects. Allocation tokens must include language, file/function/body context, source span where available, and provenance.
- **D-13:** Access paths should be explicit facts rather than only `Vec<PlaceProjection>` embedded in `PlaceFact`. They should support field/property/index/deref/await/call-return projections, known vs unknown keys, receiver paths, and bounded depth/status so later data-flow and evidence phases can reference them directly.
- **D-14:** Unknown dynamic keys should not collapse the whole object to exact field-insensitive precision. Represent them as unknown/budgeted access-path or points-to facts and preserve enough evidence for debug/eval output.

### Points-To and Alias Semantics

- **D-15:** Whole-repo points-to remains optional and demand/request scoped. Baseline behavior must not require running an expensive global solver for every `polint check`. The provider may compute local/bounded facts eagerly where cheap, then use query-scoped or budgeted solving for expensive precision.
- **D-16:** Use a bounded inclusion-based points-to substrate as the first solver direction: address-of/allocation, copy, load/store, field/property load/store, element load/store, call-return, and summary-flow constraints. It should support dense IDs, deterministic bitsets, delta propagation, SCC collapse for copy constraints, field sensitivity where modeled, type filters, and explicit budget exhaustion.
- **D-17:** Alias answers are a query/service layer derived from identity, language ownership/disjointness, local flow, extension facts, points-to sets, and future sparse refinements. Do not store "the alias graph" as the primary source of truth.
- **D-18:** Alias statuses must include exactly the roadmap-required vocabulary: `NoAlias`, `MayAlias`, `MustAlias`, `PartialAlias`, and `Unknown`. `MustAlias` should be rare and evidence-backed; `MayAlias` and `Unknown` are the honest defaults when facts are incomplete or dynamic.
- **D-19:** Alias answers carry evidence and reason codes: same stable place, disjoint locals/allocations, disjoint points-to sets, overlapping points-to sets, singleton-equal object, unsupported dynamic construct, setup missing, budget exceeded, or extension-provided assertion.
- **D-20:** Alias queries should use a provider stack in this order unless planning finds a stronger local pattern: identity/disjointness, language ownership, local flow/narrowing, validated extension alias facts, bounded points-to, then future sparse refinement. Return the first definitive answer with evidence; otherwise return `Unknown`.

### Provider Placement, Cache, and Extension Integration

- **D-21:** Add a new native provider such as `polint.type_value_alias` after `polint.entrypoints` and before `polint.extensions` if it only consumes native facts, or split native and extension-aware merge phases if extension-provided type/value/alias facts must participate before downstream consumers. The planner may choose exact placement, but provider order must be explicit, deterministic, and reflected in provider manifests/tests.
- **D-22:** Provider inputs should include source files, functions, symbols/references/scopes, semantic imports, module topology, MIR bodies/operations/places/unsupported semantics, CFG nodes/edges/control dependence, direct calls/unresolved calls, abstract-domain observations/events, direct summaries, entrypoints/trust boundaries/dispatch edges, and relevant language lifecycle/config/toolchain digests.
- **D-23:** Provider outputs should include normalized type facts, narrowed type facts, value facts, allocation tokens, access paths, points-to constraints/results, alias answers/events, and unresolved precision rows. Use schema labels such as `type-value-alias-facts-1` or a clearer equivalent.
- **D-24:** Cache identity must include provider/schema version, source/config/lifecycle inputs, upstream provider output digests, language setup/toolchain digests when official tooling is used, extension digest slots, model digest slots, budget/precision-tier settings, and absent sentinels for unused future components.
- **D-25:** Extension facts may add type hints, value facts, allocation tokens, points-to constraints, no-alias/must-alias evidence, and function/API summaries through the Phase 34 typed sink boundary. Extension facts cannot delete native facts; conflicts produce validation diagnostics or quarantine rather than silent overrides.
- **D-26:** Extension-provided exactness is precision-ceiling gated. Exact alias or type facts require validation evidence; generated-unvalidated or heuristic extension facts stay labeled and quarantine-eligible.

### Validation, Debug, Evaluation, and Public Boundary

- **D-27:** Validation must check dangling references to files/functions/MIR bodies/places/calls/entrypoints, invalid spans, duplicate stable keys, precision ceiling violations, type/value/status incompatibilities, malformed access paths, missing provenance, unsupported official-tool setup, and alias answers that claim impossible precision without evidence.
- **D-28:** Debug snapshots should include counts by language, fact family, type/value/alias status, precision, unsupported reason, solver budget status, and extension/native provenance. Snapshots must avoid raw source bodies, absolute paths, parser object IDs, timestamps, and nondeterministic ordering.
- **D-29:** Eval fixtures must cover receiver narrowing, function values, object/property allocations, field sensitivity limits, unresolved aliases, Go official-tool digest participation where used, TS/JS narrowing and dynamic-property unknowns, extension-improved precision, cold/warm/no-cache determinism, and no-leak public boundary checks.
- **D-30:** Add public no-leak proof. Normal `polint check --format json`, CLI help, SDK exports, runner surface, README, and `docs/facts` must not expose private provider IDs, type/value/alias internals, solver/debug vocabulary, or preview SDK view names unless intentionally promoted in Phase 41.

### The Agent's Discretion

- The planner may decide exact Rust module layout and whether to combine or split `analysis::types`, `analysis::values`, `analysis::access_paths`, `analysis::points_to`, and `analysis::aliases`, as long as the implementation stays crate-private and testable.
- The planner may decide whether to deliver points-to constraints and alias query service in the same plan or separate plans, provided Phase 36 still satisfies explicit alias statuses and fixture coverage.
- The planner may decide whether the provider emits local points-to sets eagerly or only constraints plus query-scoped results, provided whole-repo points-to is not mandatory baseline behavior.
- The planner may decide how much Go official-tool integration to implement in this phase versus representing the hook/digest boundary first, provided success criteria about official-tool input digests are addressed where official tooling is used.
- The planner may split execution across fact contracts/store, provider/cache wiring, Go type/value facts, TS/JS type/value/narrowing facts, points-to/alias service, extension sink integration, and validation/debug/eval/no-leak proof.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 36 goal, SAE-PREC-01 mapping, research links, and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-PREC-01 requirement text and v1.2 boundaries.
- `.planning/PROJECT.md` — Product boundaries, private-analysis-first milestone intent, and public API discipline.

### Type/Value/Alias Research

- `research/type-alias-points-to/FINAL-REPORT.md` — Executive decision to build layered type/value/place/narrowing/points-to/alias stack; alias as query layer; official tooling allowance; precision ladder.
- `research/type-alias-points-to/RECOMMENDED_IMPLEMENTATION.md` — Proposed internal module layout, fact sketches, points-to constraints, alias provider stack, extension surface, and implementation order.
- `research/type-alias-points-to/VALIDATION.md` — Validation evidence, caveats, and confidence notes for type/value-before-points-to, Andersen baseline, provider-stack alias, and official Go tooling details.
- `research/type-alias-points-to/languages/go.md` — Go-specific type/value/alias guidance and official-tooling expectations.
- `research/type-alias-points-to/languages/typescript-javascript.md` — TS/JS-specific narrowing, value, dynamic-property, and type-system guidance.
- `research/type-alias-points-to/algorithms/core-algorithms.md` — Core algorithm patterns for places, values, constraints, and alias queries.
- `research/type-alias-points-to/algorithms/precision-cost-ladder.md` — Tiered precision/cost strategy for optional expensive analyses.
- `research/type-alias-points-to/algorithms/andersen-solver.md` — Bounded inclusion-based points-to solver details.

### Upstream Phase Decisions

- `.planning/phases/35-framework-entrypoints-and-trust-boundaries/35-CONTEXT.md` — Framework entrypoints, trust boundaries, dispatch edges, and explicit deferral of type/value/place/alias facts to Phase 36.
- `.planning/phases/34-rust-extension-provider-sink/34-CONTEXT.md` — Extension host, typed sinks, validation, precision ceilings, cache quarantine, and default-vs-extended eval; Phase 36 extension facts must use this boundary.
- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md` — Demand query, summary SCC cache, and quarantine substrate used by optional/query-scoped precision work.
- `.planning/phases/32-summary-kernel-and-direct-summaries/32-CONTEXT.md` — Direct summary domains and memory/TITO effects that Phase 36 can refine.
- `.planning/phases/31-p0-abstract-domain-kernel/31-CONTEXT.md` — Existing local domain solver and precision/status vocabulary.
- `.planning/phases/30-direct-call-facts/30-CONTEXT.md` — Direct call facts and function-value unresolved vocabulary that Phase 36 can improve.
- `.planning/phases/28-private-semantic-mir-and-place-identity/28-CONTEXT.md` — MIR and place identity foundation that Phase 36 must extend instead of replacing.

### Existing Implementation

- `crates/polint/src/analysis/places.rs` — Existing `PlaceFact`, `PlaceRoot`, `PlaceProjection`, stable key builder, and status model.
- `crates/polint/src/analysis/mir/` — MIR bodies, operations, values, unsupported semantics, Go/TS lowering, and existing value/place vocabulary.
- `crates/polint/src/analysis/domains/` — Abstract-domain facts, solver, transfer, status/precision model, and provider pattern.
- `crates/polint/src/analysis/summaries/` — Direct summary store, domains, provider, cache key, validation, and debug infrastructure.
- `crates/polint/src/analysis/calls/` — Direct call-site/target/unresolved facts, function-value uncertainty, and integration points for later refined call graphs.
- `crates/polint/src/analysis/entrypoints/` — Framework entrypoint/trust-boundary facts that can seed source/value/alias precision.
- `crates/polint/src/analysis/extensions/sinks.rs` — Extension fact candidate, precision/confidence/status, normalization, and typed sink baseline.
- `crates/polint/src/analysis_kernel/provider.rs` — Provider manifest/order/schema vocabulary where the new provider is registered.
- `crates/polint/src/analysis_kernel/metadata.rs` and `crates/polint/src/analysis_kernel/validation.rs` — Metadata and validation patterns for new fact families.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` and `crates/polint/src/analysis_kernel/incremental/quarantine.rs` — Cache key vocabulary, extension digest slots, and quarantine behavior.
- `crates/polint/src/eval/` and `tests/eval-fixtures/` — Internal eval model, fixture observation, deterministic matching, and no-leak proof patterns.
- `AGENTS.md` and `docs/API-VISIBILITY-PLAN.md` — Public API visibility discipline and supported surface boundaries.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::places::PlaceFact` already provides stable place IDs, roots, projections, statuses, and deterministic stable keys. Phase 36 should add richer access-path facts around this rather than invalidating existing MIR consumers.
- `analysis::mir::MirValue` already distinguishes places, literals, temporaries, call returns, and unknown evidence. This is the natural seed for `ValueFact` and `AllocationTokenFact`.
- `analysis::domains` already has local observations/events with `Present`, `Top`, `Unknown`, `Unsupported`, `SetupMissing`, and `BudgetExceeded` plus precision labels. Type/value/narrowing should reuse this explicit-status discipline.
- `analysis::summaries` already stores memory-touch, TITO, control, and call summary domains. Phase 36 can refine these with type/value/access-path facts without inventing a separate summary store.
- `analysis::calls` already records unresolved function-value calls. Phase 36 can supply function-object/value facts that Phase 37 uses to refine those call targets.
- `analysis::entrypoints` already models framework sources and trust boundaries. These facts can seed value/source classifications and later flow, but Phase 36 should not wire full taint propagation.
- `analysis::extensions::sinks::ExtensionFactCandidate` already has stable key, binding refs, span, precision, confidence, status, evidence, and payload labels. Phase 36 extension facts should extend or specialize this model rather than bypassing it.

### Established Patterns

- New v1.2 analysis families remain crate-private until validated and intentionally promoted.
- Provider output follows extract -> normalize -> output digest -> store -> metadata refresh -> validate -> debug/eval.
- Cache keys include provider/schema/config/lifecycle/upstream output digests and absent extension/model/toolchain sentinels.
- Unknown, unsupported, setup-missing, and budget-exceeded states are first-class facts, not swallowed fallbacks.
- Public no-leak tests protect normal CLI JSON/help, SDK, runner, README, and facts docs from private analysis vocabulary.
- Extension facts merge after validation, preserve provenance and precision ceilings, and quarantine extension-influenced cache entries.

### Integration Points

- Register a new provider manifest and kernel run step near the existing precision-producing providers: after MIR/CFG/calls/domains/summaries/entrypoints, before downstream refined call/data-flow phases depend on it.
- Extend `FactFamily`, metadata assignment, validation, debug output, eval observation, cache keys, and no-leak tests for type/value/allocation/access-path/points-to/alias facts.
- Add fixture coverage under `tests/eval-fixtures/` for mixed Go and TS/JS cases similar to semantic-MIR, abstract-domain, direct-call, direct-summary, and framework-entrypoint fixtures.
- Use Go lifecycle/config digest infrastructure when official Go tooling is consulted; use TS/JS lifecycle/config digest infrastructure when TypeScript-compatible semantic inputs are consulted.
- Keep normal rule execution and public SDK behavior unchanged unless a later phase deliberately promotes a public fact view.

</code_context>

<specifics>
## Specific Ideas

- Start with a vertical that extends current place/MIR output: parameter/local/receiver places -> access paths -> type/value rows -> local narrowing -> alias answers for same-place, disjoint locals, and unknown dynamic cases.
- For Go, cover receiver narrowing, nil/interface/pointer receiver differences, composite literal allocation tokens, selector/field access paths, function values, and official-tool input digest behavior where used.
- For TS/JS, cover `null`/`undefined`, truthiness, `typeof`, `instanceof`, strict literal equality, `in` checks, optional chaining, function/arrow/class/object allocations, module namespace objects, and unknown dynamic property keys.
- Add a minimal bounded points-to fixture only after type/value/access-path rows exist; the solver should prove address/copy/field flows with budgeted unknowns rather than aiming for whole-repo completeness.
- Alias eval should assert all five statuses: `NoAlias`, `MayAlias`, `MustAlias`, `PartialAlias`, and `Unknown`.
- Extension fixture should show a repo-local extension adding a type hint or alias assertion that improves native unknowns while preserving validation, provenance, precision ceiling, cache digest, and default-vs-extended delta evidence.

</specifics>

<deferred>
## Deferred Ideas

- Refined call graph providers over direct calls, entrypoints, summaries, type/value facts, function tokens, receiver types, and bounded points-to constraints: Phase 37.
- Local plus summary-projected data-flow facts, source/sink/sanitizer/barrier models, budgets, unknown/havoc facts, and query-scoped path search: Phase 38.
- Slicing, ranked paths, evidence bundles, summary expansion handles, and diagnostic evidence rendering: Phase 39.
- External benchmark adapters and promotion gates for precision claims: Phase 40.
- Public `Types<'_>`, `Values<'_>`, `Aliases<'_>`, bounded query builders, and agent ergonomics: Phase 41.
- Sparse MemorySSA/SVFG-like flow-sensitive refinement and high-k context sensitivity: future work after benchmark evidence shows a need.
- Broad Python/Java parity for type/value/alias facts: future milestone after Go and TS/JS prove the model.

</deferred>

---

*Phase: 36-p0-type-value-place-alias-substrate*
*Context gathered: 2026-05-24*
