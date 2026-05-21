# Phase 32: Summary Kernel and Direct Summaries - Context

**Gathered:** 2026-05-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 32 delivers the first private summary kernel and direct (local, single-function) summaries for Go and TS/JS. It should add typed summary keys, a summary store with callable-level identity, typed summary domain traits, four core direct-summary domains (control effects, call effects, memory-touch approximations, and data-flow TITO), summary metadata with status/precision/provenance, and validation/debug/eval/cache infrastructure.

This phase computes summaries from local analysis only — each function's summary is derived from its own MIR, CFG, places, direct call facts, and Phase 31 domain results without consulting callee summaries. Interprocedural summary closure (SCC scheduling, demand queries, callee summary application) belongs to Phase 33. Extension-authored summary providers belong to Phase 34. Framework entrypoints and trust boundaries belong to Phase 35. Full data-flow TITO with access paths belongs to Phase 38.

</domain>

<decisions>
## Implementation Decisions

### Summary Kernel Architecture

- **D-01:** Add summaries as a new crate-private `analysis::summaries` module, separate from `analysis::domains`. Summaries have callable-level identity and lifecycle distinct from the per-block/per-operation domain solver results. The module should follow the established `analysis::mir`, `analysis::cfg`, `analysis::calls`, `analysis::domains` pattern for visibility, provider, validation, debug, and eval.
- **D-02:** Implement a `SummaryDomain` trait with typed payload, domain ID, domain version, bottom/top/join/less_equal/widen, and stable digest. This mirrors the local `AbstractDomain` trait but operates at callable granularity rather than per-place state.
- **D-03:** Implement a `SummaryStore` as a typed internal store indexed by `SummaryKey`. The store should reject payloads with missing required digests, mismatched domain versions, precision claims exceeding provider ceilings, or conflicting higher-trust summaries. The existing `SummaryKey` struct in `analysis_kernel/incremental/keys.rs` provides the identity vocabulary.
- **D-04:** Keep all summary kernel contracts crate-private. Public SDK views (`Effects<'_>`, `TaintFlows<'_>`, etc.) remain deferred to Phase 41 promotion. Public CLI, inspect, test, docs, and README must not advertise summary internals.

### Summary Domain Selection

- **D-05:** Implement four core direct-summary domains in Phase 32:
  1. **ControlEffects** — exit set (returns, throws, panics, process exit, does-not-return, unknown), async kind, cleanup/defer/finally effects.
  2. **CallEffects** — direct callee edges, unresolved calls, callback-use evidence (invoked immediately vs. stored for later) when syntactically determinable.
  3. **MemoryEffects** — read/write/readwrite/none for receiver, parameter (by index), return, local, global/module resources, plus a coarse "may have external effects" flag for filesystem/network/database/env/process/time.
  4. **DataFlowTito** — parameter-to-return value flow, receiver mutation, argument mutation by direct assignment/update, source-like returns, and sink-like argument consumption when modeled.
- **D-06:** Each domain should have an explicit `bottom()` (no effects observed) and `unknown_top(reason)` (conservative havoc). Missing callees, unresolved calls, dynamic writes, unsupported semantics, and budget exhaustion must produce unknown/top summaries rather than empty/clean summaries.

### TITO And Data-Flow Precision

- **D-07:** Phase 32 TITO should be simple: parameter-to-return and receiver/argument mutation by direct observation from MIR operations and place identity. Do not implement field-level access-path tracking, flow-through-containers, sanitizer/barrier modeling, or taint propagation in this phase. Those belong to Phase 38 (local plus summary-projected data flow).
- **D-08:** Flow edges should carry `FlowKind` (Value, BySideEffect) and endpoint roots (Param index, Receiver, Return). Taint, Barrier, and Sanitizer flow kinds should be defined in the enum for future use but not populated by Phase 32 direct builders.

### Memory-Touch Granularity

- **D-09:** Phase 32 memory effects should cover: Receiver, Param(index), Return, Local, Global(symbol), and Module(module) resources with Read/Write/ReadWrite/None access kinds. External effects (FileSystem, Network, Database, Env, Process, Time) should use a single coarse `MayHaveExternalEffects` flag rather than per-resource tracking.
- **D-10:** Heap-allocated abstract locations, fine-grained field/property access, and external-resource-specific tracking should be deferred to Phase 36 (type/value/place/alias substrate) and Phase 38 (data flow).

### Summary-To-Solver Relationship

- **D-11:** Direct summaries should be computed by a dedicated summary builder that reads polint-owned facts, not by raw AST traversal. The builder should:
  1. Lift control-effect approximations from Phase 31 domain solver results where available (reachability → does-not-return, nilness/truthiness → throw/panic evidence).
  2. Run a dedicated MIR/CFG pass for TITO and memory effects, since these require tracking flow through operations rather than just lattice values at program points.
- **D-12:** The summary builder should consume: MIR bodies/operations, CFG facts, place identity, direct call facts, and domain solver results. It must not re-run the local solver or duplicate domain computation.

### Validation, Cache, Debug, And Evaluation

- **D-13:** Extend metadata and validation for all new summary fact families. Validation should catch: dangling callable/function/symbol references, duplicate summary keys, mismatched domain versions, invalid precision claims, missing status/provenance, conflicting summaries for the same key, and malformed TITO/memory/control/call payloads.
- **D-14:** Cache identity for summaries must include: provider/schema version, source/config/lifecycle inputs, semantic MIR output digest, CFG output digest, calls output digest, domain output digest, summary domain versions, and absent extension/model/toolchain slots.
- **D-15:** Add internal debug snapshots with summary counts by language, domain, status, precision, and function. Snapshots must avoid raw source bodies, AST dumps, absolute paths, parser allocation IDs, timestamps, and run-local dense IDs as stable identity.
- **D-16:** Add deterministic eval fixtures for Go and TS/JS covering: direct no-return functions, throw/panic summaries, normal return, simple TITO (param returned, receiver mutated), memory effects (reads param, writes receiver, mutates global), unresolved calls producing unknown summaries, and functions with no observable effects.
- **D-17:** Add public no-leak proof. Public CLI JSON/help, `polint inspect`, `polint test`, SDK exports, README/docs, and external temp-repo rules must not expose or advertise private summary internals.

### Claude's Discretion

- The planner may choose exact Rust module layout (e.g., `analysis/summaries/{mod,kernel,store,domains,builder,validate,provider,debug}.rs`) as long as visibility stays crate-private.
- The planner may decide whether the four summary domains are implemented as separate types or as variants of a single `SummaryPayload` enum, provided querying, validation, debug, and cache remain straightforward.
- The planner may decide whether the summary provider runs as one pass computing all four domains or as separate sub-passes per domain, provided output is deterministic and metadata/validation cover all domains.
- The planner may decide whether to add a new `LayerKind::DirectSummaries` or reuse/extend the existing layer vocabulary, provided cache identity remains correct.
- The planner may defer full SCC closure, callee summary application, extension summary providers, framework models, access-path TITO, heap/alias tracking, and public SDK views if Phase 32 success criteria are satisfied honestly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 32 goal, requirement mapping (SAE-INT-02), and success criteria.
- `.planning/REQUIREMENTS.md` - SAE-INT-02 requirement text and traceability.
- `.planning/PROJECT.md` - Public API discipline, current milestone goals, and active v1.2 boundaries.

### Effects And Summaries Research

- `research/effects-summaries/FINAL-REPORT.md` - Executive summary, state-of-art convergence, summary kernel + typed domain architecture recommendation.
- `research/effects-summaries/RECOMMENDED_IMPLEMENTATION.md` - Recommended SummaryKey, SummaryStore, SummaryDomain trait, four core domains (ControlEffects, CallEffects, MemoryEffects, DataFlowTito), local summary builders, and language provider order.
- `research/effects-summaries/VALIDATION.md` - Validation levels, required fixture families, extension validation matrix, cache invalidation tests, and accuracy metrics.

### Upstream Phase Decisions

- `.planning/phases/31-p0-abstract-domain-kernel/31-CONTEXT.md` - Domain solver, product state, lattice traits, law tests, transfer semantics, and explicit deferral of summaries to Phase 32.
- `.planning/phases/30-direct-call-facts/30-CONTEXT.md` - Direct call facts, call-site/target/unresolved model, store indexes, and explicit deferral of summary-assisted targets.
- `.planning/phases/29-local-cfg-and-control-dependence/29-CONTEXT.md` - CFG contracts, derived analyses, and explicit deferral of summary effects.

### Existing Implementation Patterns

- `crates/polint/src/analysis/domains/` - Phase 31 abstract domain infrastructure: lattice traits, core domains, product state, solver, transfer, results, facts, store, provider, debug, and validation.
- `crates/polint/src/analysis/calls/` - Direct call facts, call-site/target store, indexes, provider, validation, debug, and eval pattern.
- `crates/polint/src/analysis/mir/` - Private semantic MIR/place contracts and lowering.
- `crates/polint/src/analysis/cfg/` - Private CFG contracts, builder, derived analyses, provider.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Existing `SummaryKey` struct (lines 90-98) with callable_stable_key, summary_domain, summary_version, body_shape_digest, dependency_summary_digests, and extension_digest.
- `crates/polint/src/analysis_kernel/` - Provider manifest, metadata, validation, debug, cache key, and run-report integration points.
- `crates/polint/src/eval/` and `tests/eval-fixtures/` - Internal fixture format and deterministic observation pattern.

### API And Visibility

- `AGENTS.md` - Public API visibility, Rust best-practice usage, rule-authoring platform contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::domains` provides the local abstract domain solver, product state, lattice traits, core domain results, and transfer infrastructure. Phase 32 summaries can lift control-effect approximations directly from solver results.
- `analysis::calls` provides direct call-site/target/unresolved facts that summary builders need to determine callee effects and mark unresolved calls as producing unknown summaries.
- `analysis::mir` provides semantic bodies, operations, place identities, and call-shaped evidence that summary builders consume.
- `analysis::cfg` provides per-function CFG facts, reachability, and derived analyses that inform control-effect and TITO computation.
- `analysis_kernel::incremental::keys` already defines `SummaryKey` with the right identity fields for callable-level summary storage.
- `analysis_kernel::metadata`, `validation`, `debug`, and `incremental` provide established vocabulary for fact families, precision, provenance, stable keys, digests, and layer cache.

### Established Patterns

- New v1.2 fact families stay crate-private, use run-local dense IDs plus stable keys, and are guarded by no-leak CLI tests.
- Provider outputs are normalized deterministically, validated before use, and exposed to eval through test-only debug JSON.
- Unknown, unsupported, setup-missing, partial, dynamic, and heuristic states are explicit facts/statuses, not hidden logs.
- Cache identities include provider/schema/config/lifecycle/upstream digests plus absent future extension/model/toolchain slots.
- The `domains/provider.rs` pattern (derive → normalize → output_digest → store → metadata refresh) is the model for the summary provider.

### Integration Points

- Add `analysis::summaries` as a new private module, wired into the kernel provider sequence after `polint.domains` and before future summary consumers.
- Extend `FactFamily` enum with summary-related families (SummaryControl, SummaryCall, SummaryMemory, SummaryTito, SummaryMeta).
- Extend provider manifests, run reports, metadata validation, debug JSON, and eval observation for the summary provider.
- Consume Phase 31 domain solver results as input to control-effect lifting.
- Consume Phase 30 direct call facts as input to call-effect and unknown-callee summary computation.

</code_context>

<specifics>
## Specific Ideas

- Auto mode selected the research-driven defaults: all four core domains at direct/local level, simple TITO without access paths, coarse external-effect flag for memory.
- The existing `SummaryKey` in `analysis_kernel/incremental/keys.rs` already has the right shape — the summary store should use this key type rather than inventing a new one.
- Control-effect lifting from domain solver results (reachability → does-not-return, constant-folded throw conditions) is a key integration point that avoids duplicating local analysis.
- Phase 32 direct summaries should be useful to Phase 33 SCC closure without requiring Phase 33 to re-compute local analysis — the store should support querying direct summaries for any analyzed function.

</specifics>

<deferred>
## Deferred Ideas

- Interprocedural summary closure, SCC scheduling, demand queries, and callee summary application: Phase 33.
- Extension-authored summary providers, typed sinks, activation levels, and validation gates: Phase 34.
- Framework entrypoints, trust boundaries, and synthetic summary targets: Phase 35.
- Access-path TITO, field-level flow, sanitizer/barrier modeling, and heap/alias tracking: Phase 36 and Phase 38.
- Refined call graph providers consuming summaries: Phase 37.
- Full data-flow path search and evidence bundles: Phase 38 and Phase 39.
- Benchmark adapters and promotion gates for summary precision claims: Phase 40.
- Public SDK views (Effects, TaintFlows, ResourceFlows) and agent ergonomics: Phase 41.

</deferred>

---

*Phase: 32-summary-kernel-and-direct-summaries*
*Context gathered: 2026-05-21*
