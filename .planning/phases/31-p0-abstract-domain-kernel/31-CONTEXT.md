# Phase 31: P0 Abstract-Domain Kernel - Context

**Gathered:** 2026-05-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 31 delivers the first private abstract-domain substrate over the existing semantic MIR and CFG layers. It should add crate-private lattice/transfer traits, deterministic local solving, stable domain output/storage/metadata, law and transfer tests, debug/eval proof, and first P0 local domains for reachability, nilness/nullishness, truthiness, constants/literal sets, simple string facts, and cheap initializedness where practical.

This phase must not promote public SDK query views, public CLI output, refined call graph providers, interprocedural summaries, extension-authored domains, or broad type/value/alias facts. Those belong to later phases.

</domain>

<decisions>
## Implementation Decisions

### Kernel Boundary And Visibility

- **D-01:** Keep all abstract-domain contracts crate-private, following the Phase 28-30 pattern. Public SDK, runner, CLI, README, and `docs/facts/` surfaces should not advertise domain facts in Phase 31.
- **D-02:** The domain kernel should consume existing polint-owned MIR, CFG, place, call, symbol/reference, and metadata rows. It must not read parser AST nodes, raw source snippets, or language-tool object graphs directly.
- **D-03:** Add a private provider/layer only if it can participate honestly in the existing kernel manifest, metadata validation, cache identity, debug, eval, and public no-leak patterns. If provider wiring is staged, keep incomplete pieces test-only and explicit.

### Lattice And Domain Product

- **D-04:** Start with small, law-tested domain traits: bottom/top, ordering, join, `join_into`, widening/fuel hooks, stable digest, and transfer hooks. Do not build a symbolic executor, Datalog substrate, or relational numeric engine in this phase.
- **D-05:** Model local abstract state as a deterministic product of independently versioned core domain slots. The first slots should prioritize reachability, nilness/nullishness, truthiness, constants/literal sets, simple string facts, and cheap initializedness.
- **D-06:** Treat intervals, shape/property facts, typestate/resource facts, path predicates, extension domain slots, and summary payload algebra as future-facing design pressure only unless a tiny internal hook is required to keep the P0 shape extensible.

### Solver Scope And Semantics

- **D-07:** The first solver should be local and deterministic over per-function CFG/MIR, with stable block order, stable operation order, bounded iteration, explicit widening fuel, and deterministic tie-breaking via sorted stable keys.
- **D-08:** Transfer logic should separate lattice operations from operation/edge effects. Branch/edge-specific assumptions should be represented explicitly enough for truthiness, nilness, constants, and reachability tests.
- **D-09:** Calls should be conservative in Phase 31. Direct call facts can inform unknown/havoc decisions, but summary application and summary SCC scheduling belong to Phase 32 and Phase 33.

### Truthfulness And Unknown States

- **D-10:** Unsupported semantics, unresolved calls, dynamic writes, setup gaps, budget exhaustion, and widening/top transitions must produce explicit unknown/top/budget/unsupported statuses rather than silent certainty.
- **D-11:** Domain-derived diagnostics or eval facts must not use "must" semantics unless the domain can justify exact local precision. Heuristic or setup-aware facts should carry conservative precision/status labels.

### Validation, Debug, Eval, And Cache

- **D-12:** Domain-law tests are mandatory: partial order laws, join idempotence/commutativity/associativity/upper-bound laws, widening convergence samples, stable serialization/digest, and transfer monotonicity samples.
- **D-13:** Add native eval fixtures only for internal observation. Fixtures should prove top/unknown/budget states, stable output, provider/status/precision labels, and deterministic cold/warm/no-cache behavior when cache wiring exists.
- **D-14:** Debug snapshots must stay crate-private/test-facing and serialize compact stable IDs, labels, counts, and relative source evidence only. No raw source bodies, AST dumps, absolute paths, or run-local IDs as stable identity.
- **D-15:** Cache identity should include domain versions, schema labels, reduction/widening policy, semantic MIR/CFG/calls input digests, config/lifecycle inputs, and absent extension/model/toolchain slots.

### the agent's Discretion

- The planner may choose exact Rust module names and task splits, but should preserve the existing private-provider style used by `analysis::mir`, `analysis::cfg`, and `analysis::calls`.
- The planner may defer a domain slot if existing MIR/CFG facts cannot support it honestly yet, but must keep SAE-INT-01 coverage explicit and explain any narrowed "where practical" interpretation.
- The planner may decide whether Phase 31 creates one provider or separates domain contracts, solver, debug/eval, and cache wiring across multiple plans.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 31 goal, requirement mapping, and success criteria.
- `.planning/REQUIREMENTS.md` - SAE-INT-01 requirement text and traceability.
- `.planning/PROJECT.md` - Public API discipline, current milestone goals, and active v1.2 boundaries.

### Abstract Interpretation Research

- `research/abstract-interpretation/FINAL-REPORT.md` - Product-level recommendation, domain priority, precision ladder, and explicit unknown guidance.
- `research/abstract-interpretation/RECOMMENDED_IMPLEMENTATION.md` - Recommended native domain kernel shape, product state, solver model, and ownership boundaries.
- `research/abstract-interpretation/VALIDATION.md` - Law, monotonicity, widening, determinism, cache, precision, and benchmark validation gates.
- `research/abstract-interpretation/implementation/MIR-CONTRACT.md` - Minimum semantic operation contract domains should consume instead of parser ASTs.
- `research/abstract-interpretation/implementation/EXTENSION-DOMAIN-CONTRACT.md` - Future extension-slot constraints; use as design pressure, not Phase 31 scope expansion.
- `research/abstract-interpretation/implementation/BOOTSTRAP-SEQUENCE.md` - Bootstrap order if planning needs a finer implementation sequence.

### Existing Implementation Patterns

- `crates/polint/src/analysis/mir/` - Private semantic MIR/place contracts and lowering pattern.
- `crates/polint/src/analysis/cfg/` - Private CFG contracts, builder, derived analyses, provider, validation, debug, and eval pattern.
- `crates/polint/src/analysis/calls/` - Private call fact provider, deterministic store/indexes, validation, debug, eval, and no-leak pattern.
- `crates/polint/src/analysis_kernel/` - Provider manifest, metadata, validation, debug, cache key, and run-report integration points.
- `crates/polint/src/eval/` and `tests/eval-fixtures/` - Internal fixture format and deterministic observation pattern.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::mir` already provides semantic bodies, operations, places, unsupported semantic rows, stable keys, and crate-private stores suitable as domain inputs.
- `analysis::cfg` already provides per-function CFG facts, derived reachability/dominance/control-dependence rows, deterministic graph helpers, validation, and debug JSON.
- `analysis::calls` already provides direct/unresolved call facts and can be used to make call transfers conservative without inventing summaries.
- `analysis_kernel::metadata`, `validation`, `debug`, and `incremental` provide the established metadata, validation, debug, digest, and layer-cache vocabulary.

### Established Patterns

- New v1.2 fact families stay crate-private, use run-local dense IDs plus stable keys, and are guarded by no-leak CLI tests.
- Provider outputs are normalized deterministically, validated before use, and exposed to eval through test-only debug JSON rather than public check JSON.
- Cache identities include provider/schema/config/lifecycle/upstream digests plus absent future extension/model/toolchain slots.

### Integration Points

- Phase 31 should likely run after `polint.calls` and before later summary/data-flow phases.
- Metadata validation should call an abstract-domain validator once rows exist.
- Internal eval should observe domain debug rows through `metadata_debug_json_for_test`.
- Public capability planning should keep any future domain/call-graph/data-flow views unsupported until a later promotion phase.

</code_context>

<specifics>
## Specific Ideas

- Use `join_into` or an equivalent mutation API in the solver rather than open-coding `leq` polarity checks.
- Prefer deterministic `BTreeMap`/sorted-key structures for state slots, worklists, debug rows, and eval observations.
- Make top/unknown reasons first-class enough to explain precision loss in later diagnostics.
- Keep first transfer semantics intentionally small and fixture-backed; precision can improve in later type/value, summary, and refined call graph phases.

</specifics>

<deferred>
## Deferred Ideas

- Interprocedural summaries, summary SCC scheduling, and summary cache: Phase 32 and Phase 33.
- Extension-authored domain slots and external provider sinks: Phase 34.
- Framework entrypoints and trust-boundary models: Phase 35.
- P0 type/value/place/alias substrate beyond cheap literals and initializedness: Phase 36.
- Refined call graph, data-flow, slicing/path evidence, benchmark gates, and public SDK promotion: Phases 37-41.

</deferred>

---

*Phase: 31-p0-abstract-domain-kernel*
*Context gathered: 2026-05-21*
