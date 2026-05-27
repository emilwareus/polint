# Project Research Summary

**Project:** polint v1.3 — Graph Engine Precision
**Domain:** Multi-language static-analysis engine — shared semantic graph + unified call-graph solver (Go x/tools RTA + JS/TS Jelly oracle alignment)
**Researched:** 2026-05-27
**Confidence:** HIGH

## Executive Summary

v1.3 is the milestone where polint's v1.2 substrate (kernel facade, MIR, CFG, calls, summaries, type/value/place/alias, data flow, evidence, layer cache, benchmark adapters) is reorganized into a **single shared semantic graph + unified call-graph solver**. Today's isolated fact families produce 2.7% recall on Go x/tools RTA and 0.63% recall on Jelly — not because the substrate is wrong, but because there is no shared identity, reachability layer, language-native Go semantics, or function-token/object-property solver to convert facts into benchmark-grade edges. The v1.3 goal is to lift both suites to >25-30% recall (with the algorithmic ceiling around 70-90% Go RTA / 35-60% Jelly token / 55-80% Jelly object-model) while holding precision as a first-class gate — refusing the "recall by flooding" anti-pattern that would silently destroy the product's repo-local-policy value.

The recommended approach is staged and dependency-ordered, with one architectural keystone: **language frontends emit constraints into a single solver, not a per-language solver each.** Ship identity + benchmark renderers + dedup + reachability roots first (low-cost, high-impact, makes every downstream metric trustworthy). Then JS inventory + scope + module graph (Oxc-backed) and the Go semantic frontend (out-of-process `polint-go-frontend` sidecar over NDJSON, never cgo) land in parallel. After that, build `analysis::solver` as a generalization of v1.2's `points_to::solver` — same deterministic worklist discipline, broader constraint vocabulary (`CopyEdge`, `Alloc`, `FieldLoad`, `FieldStore`, `CallConstraint`, `ModelEdge`). With the solver in place, Go RTA and JS function-token propagation can ship in parallel, JS object/property/`this` modeling follows, then adaptation models (gated behind precision floors and sandboxing), and finally cache + budgets are consolidated as a cross-cutting concern. The v1.2 promotion discipline carries forward: **everything new is `pub(crate)`; no SDK promotion in v1.3** until two-milestone benchmark gates approve.

Key risks are concentrated in three areas: **(1) determinism** — solver iteration over `HashMap` will silently introduce flakes, so `BTreeMap`/`IndexMap`/`FxHashMap` with sorted boundaries are mandatory from day one; **(2) cache invalidation** — RTA and token propagation are whole-program fact families whose dependency indexes must be designed in their first phase, not bolted on later; **(3) benchmark-label leakage and recall flooding** — the adaptation agent must run in a sandbox that cannot see oracle files, and promotion gates must enforce a precision floor, not just recall deltas. The Go sidecar adds process-lifecycle, version-skew, and `SetupMissing` surface area that v1.2 has not yet exercised — these need typed protocols and per-mode `unsupported_taxonomy` categories from the very first Go-semantic phase.

## Key Findings

### Recommended Stack

The v1.2 stack stays (Rust 2024, rustc 1.94.0, Oxc, tree-sitter-go, petgraph, rayon, serde, insta, proptest, assert_cmd, all already pinned). v1.3 adds a small, deterministic, hot-path-oriented set of crates and a Go sidecar binary. **No async runtime, no datalog/Datafrog framework, no cgo, no second graph crate, no broad new public surface.**

**Core additions (Rust workspace):**
- `fixedbitset 0.5.7` + `roaring 0.11.4`: dense and sparse bitsets for reachability, points-to membership, token sets, RTA reached-function sets.
- `hashbrown 0.17.1` + `rustc-hash 2.1.2`: raw-entry SwissTable + `FxHasher` for integer-keyed solver maps.
- `smallvec 1.15.1` + `indexmap 2.14.0`: inline-store small vectors for `points_to`/`call_targets`/`flow_edges`; insertion-ordered maps for any solver output that gets hashed/snapshotted.
- `string-interner 0.20.0` + `blake3 1.8.5`: stable property/qualified-name interning; fast cryptographic-strength digests for new cache key families (sidecar input digest, model digest, solver budget digest).
- `which 8.0.2`: locate `polint-go-frontend` on PATH; emit structured `SetupMissing` if absent.
- Oxc bump to 0.133.0 (Oxc semantic fidelity is load-bearing for JS scope/binding/`SymbolTable`).

**Go sidecar (architectural keystone):**
- Out-of-process Go helper `polint-go-frontend`, NDJSON over stdio. Pinned `golang.org/x/tools v0.45.0` for `go/packages` + `go/ssa` + `go/callgraph/rta`. Co-shipped binary; sidecar digest participates in cache keys. **Rejected:** cgo-hosted Go runtime (breaks rayon determinism + `unsafe_code = "forbid"`), reimplementation of `go/types` in Rust (multi-engineer-year, diverges from compatibility authority), protobuf/MessagePack/bincode wire (negligible perf win, large debug/tooling cost).

**Conditional / deferred:** `bincode 3.0` and `dashmap 6.2.1` only if profiling demands them. `tokio`/async explicitly forbidden in the solver core for determinism.

See `.planning/research/STACK.md`.

### Expected Features

The full v1.3 feature inventory is in `.planning/research/FEATURES.md`. Every must-have is a `P1` table-stakes feature without which either the benchmark gates cannot pass or the reported numbers are statistically untrustworthy.

**Must have (table stakes — all P1):**
- **Stable internal identity** for functions, callsites, scopes, packages — the keystone; five other features become noise without it.
- **Benchmark renderers** (`Go RelString`, `Jelly file:start_line:start_col:end_line:end_col`) — recall is unmeasurable without them.
- **Observed-edge deduplication** by semantic identity before scoring.
- **Identity-vs-unsupported taxonomy** (`wrong_identity`, `unsupported_edge`, `unresolved_edge`, `package_load_limitation`, `model_missing`).
- **Explicit reachability roots** (`main`/`init`/exported/tests/configured) + **reachable-graph scoring mode** (per-suite oracle alignment: `oracle-rta`, `oracle-jelly`, `whole-repo`).
- **Go semantic frontend** (`go/packages` loader + module-root inference + types + receiver types + method sets + init + generics) — sidecar-backed.
- **Go RTA provider** (reachable, address-taken, dynamic dispatch by signature, interface invoke by method-set, fixed-point iteration).
- **JS/TS function and callsite inventory** with **exact-span identity** (1-based line/col, inclusive end, Jelly-shaped).
- **JS/TS scopes/bindings** + **module graph** (ESM + CJS + tsconfig path aliases).
- **JS/TS direct binding** (the lowest-precision Jelly recall layer — `f()`, `ns.f()`, imported aliases).
- **JS/TS function-token propagation** (the main Jelly recall lever).
- **JS/TS object/property/prototype/class/`this` model** with allocation-site abstraction.
- **Shared semantic graph + unified call solver** (the architectural keystone).
- **Per-family caches + solver budgets** (tokens, properties, fanout, package depth, model expansion).
- **Unsupported / unknown taxonomy** across every provider.

**Should have (differentiators — P2, ship after gates pass):**
- **Adaptation model layer** with schema validation, accept/reject reporting, prompt-hash + delta — polint's product wedge per the agent-extension research.
- **Per-edge algorithm provenance ladder** (`syntactic` → `bound` → `direct` → `cha` → `rta` → `vta` → `token` → `points_to` → `repo_model`).
- **JS native-callable shims** (`Array.prototype.map`, `forEach`, `Promise.then`, ...) loaded through the same adaptation schema (overridable per-repo).
- **Confidence label tightening** + **demand-driven graph slices** wired to the unified solver.
- **Cold-vs-warm benchmark runtime + cache hit/miss reporting**.

**Defer (v1.4+):**
- Go VTA provider; bounded Andersen-style points-to as a separate opt-in family; context sensitivity (k-CFA / object-sensitive); public SDK surface for the new graph engine (must clear two-milestone promotion gate); Python and Java parity; reflection / dynamic-import auto-modelling.

**Anti-features (rejected; will resurface under recall pressure):**
- Recall-by-flooding; benchmark-label adaptation; wildcard / broad-pattern models; single monolithic call-graph without algorithm/confidence labels; whole-program closed-world points-to as default; auto-modeled reflection edges; tree-sitter-only Go pipeline; span identity from display names; hiding unresolved/unsupported facts by default; one scoring mode across Go and Jelly suites.

### Architecture Approach

The architectural keystone is a **single `analysis::solver` consuming constraints emitted by language-specific frontends**, sitting on top of the v1.2 substrate (MIR, CFG, calls, summaries, type/value/place/alias, entrypoints, extensions) and feeding the existing `refined_calls`/`data_flow`/`evidence` downstream views unchanged. **No new crate**: everything stays inside `crates/polint` as `pub(crate)` modules, in line with v1.2's promotion discipline.

**Major components:**

1. **`analysis::semantic_graph`** (new) — Shared node/edge store with stable identities for functions, callsites, scopes, places, abstract objects, modules, packages. Typed edges (`Call`, `MemberOf`, `Alloc`, `Flow`). A *projection* over existing facts, not a parallel substrate.
2. **`analysis::reachability`** (new) — `RootSet` discovery from v1.2 entrypoints + per-suite mode-aware reachable-graph BFS. Drives both Go RTA's reachable-from-roots fixpoint and per-suite scoring filters.
3. **`analysis::solver`** (new) — Generalization of v1.2's `points_to::solver`. `ConstraintStore` keyed by stable IDs, deterministic `BTreeMap`/`VecDeque` worklist, explicit `SolverBudget`, per-language driver siblings (`go_rta.rs`, `ts_tokens.rs`, `ts_object_model.rs`) that share the same iteration logic. Folds `points_to` in as a sub-domain.
4. **`analysis::adaptation`** (new) — Validated repo-local model files (`.polint/models/*.toml`) → `ModelEdge` constraints into the solver. Schema requires concrete patterns; validator confirms targets resolve in the semantic graph; accepted/rejected counts + prompt hash + delta reported in `benchmark adapted` mode.
5. **`analysis::benchmark_identity`** (new) — Per-suite renderers (`go_relstring.rs`, `jelly_span.rs`) + dedup + categorization. Pure functions over identities; never consumed by rules.
6. **`analysis::unknown_taxonomy`** (new) — `SetupMissing`, `UnsupportedSemantic`, `MissingFact`, `OutOfScope`, plus sidecar-specific failure modes (`GoPackagesLoadFailed`, `GoVersionUnsupported`, `GoSidecarTimeout`). Pervasive across providers.
7. **Frontend extensions** — `src/go/semantic/` (sidecar client + `lower.rs` mapping NDJSON to constraints) and `src/ts/{inventory,scope,object_model}/` (Oxc-backed constraint emitters). Frontends own identity mapping and constraint emission; they do **not** own solving.
8. **`polint-go-frontend` sidecar binary** (new, Go) — `go/packages` + `go/ssa` + NDJSON emitter. Co-shipped, sidecar binary digest in cache keys, JSON schema versioned.
9. **`refined_calls::provider` rework** — Becomes a thin projection over solver output emitting `RefinedCallEdgeFact` unchanged. v1.2's heuristic refiners retire. Downstream `data_flow`/`evidence`/public SDK views see identical facts.

**Suggested build order (dependency-driven; see `ARCHITECTURE.md` §"Suggested Build Order"):**

1. Identity + dedup + benchmark renderers. 2. Reachability + roots + per-suite scoring mode. 3. `semantic_graph` skeleton. 4. JS inventory + exact spans. 5. JS scope + module graph + direct binding. 6. Go semantic frontend (parallel with #4-5). 7. `solver` core. 8. Go RTA driver (parallel with #9). 9. JS function-token driver. 10. JS object/property/`this` model. 11. Adaptation models. 12. `refined_calls::provider` rework. 13. Unknown taxonomy (continuous). 14. Cache + budgets consolidation. 15. Benchmark promotion gate extension.

See `.planning/research/ARCHITECTURE.md`.

### Critical Pitfalls

The top high-impact pitfalls from `.planning/research/PITFALLS.md` — each is structural to v1.3 and must be prevented design-time, not via policy.

1. **Determinism drift from solver iteration order.** `HashMap`/`HashSet` iteration order leaks into solver step counts, widening triggers, and budget cut-off decisions. **Prevent by** using `BTreeMap`/`IndexMap`/`FxHashMap` for all observable solver state, installing a determinism gate (10 runs with shuffled provider order → byte-identical observed JSON) before any solver lands, and totally-ordering every tie-break.

2. **Cache invalidation gaps with new cross-family fact dependencies.** RTA and token propagation are whole-program — a single function edit must invalidate the unified-graph layer transitively. **Prevent by** designing the dependency index in the `semantic_graph` phase, requiring a single-input-mutation fixture per new fact family, including sidecar binary digest + Go toolchain version + adaptation model digest + solver budgets in cache keys, and adding negative (must-preserve-hit) fixtures alongside positive (must-invalidate) ones.

3. **Recall-via-flooding and benchmark-label leakage.** A solver achieves recall by emitting every plausible edge; an adaptation agent reads expected-edge JSON and authors models that mirror oracle labels. **Prevent by** hard precision floors in promotion gates (Go ≥60%, configurable per suite), F-score β=0.5 tracking alongside F1, sandboxing the adaptation agent into a directory that does not contain `research/evaluation-harness/repos/*/expected*` or oracle JSON, hashing the adaptation prompt + asserting no benchmark-suite paths leak in, rejecting model facts whose RHS exactly matches oracle expectations, and held-out subset reporting per suite.

4. **Go sidecar FFI lifecycle.** `go/packages` can take minutes, hang under `go list` misbehavior, version-skew across Go toolchains. **Prevent by** wrapping the sidecar in a typed process boundary (length-prefixed framing or strict NDJSON with explicit terminators), enforcing per-request timeouts + cancellation propagation, pinning Go toolchain version in `InputSnapshot`, using a single long-lived sidecar per `polint check`, distinguishing `GoPackagesLoadFailed` / `GoVersionUnsupported` / `GoSidecarTimeout` in the unknown taxonomy, and adding a SIGTERM-cleanup fixture asserting no orphan Go processes survive after 5 seconds.

5. **Span identity drift between Oxc and Jelly.** Template literals, JSX whitespace, optional calls, class static blocks, accessors — Oxc and Jelly count differently. **Prevent by** building the Jelly span renderer first, gating ≥99% oracle-span coverage on Jelly fixtures across Linux + macOS CI (CRLF/LF normalization fixture), documenting the chosen outer-span for every known-ambiguous Oxc AST form, and classifying span mismatches as `wrong_identity` (not "missing edge") in the taxonomy.

6. **Public/private API leak.** **Prevent by** marking every v1.3 new type `pub(crate)` from day one, adding a CI gate that compiles a minimal external rule crate and asserts no v1.3 solver types are reachable from `polint::sdk::prelude::*`, routing the benchmark adapter through `pub(crate) trait BenchmarkInternalView` rather than the public SDK, and reserving SDK promotion for a separate explicit phase after two-milestone benchmark stability.

7. **Per-language regression masking.** A solver tuning that helps Go RTA precision silently regresses JS recall; aggregate F1 hides it. **Prevent by** per-language solver policy traits (`solver_config.go.*` vs `solver_config.js.*`), per-suite + per-language delta reporting + enforcement in the promotion gate, and a polyglot canary fixture mixing Go and TS files run on every solver change.

Additional pitfalls covered in PITFALLS.md: token-set memory blowup without per-family budgets (#6), scoring mode conflation between reachable and full-graph (#9), provenance loss across solver-derived edges (#10), and hidden coupling between abstract-domain kernel / summary kernel / unified solver causing dependency cycles (#12).

## Implications for Roadmap

Based on research, suggested phase structure for v1.3. Phase boundaries map 1:1 onto the dependency-driven build order; some phases can run in parallel (called out below). v1.3 starts at phase 42 (continues numbering from v1.2's last phase 41).

### Phase 42: Benchmark Identity, Renderers, Dedup, and Identity Taxonomy
**Rationale:** Identity is the keystone; nothing else's metric impact is measurable until renderers + dedup land.
**Delivers:** `analysis::benchmark_identity` (`go_relstring.rs`, `jelly_span.rs`, `dedupe.rs`, `categorize.rs`); identity record `(file, span, language, package/module, container, display, signature digest)`; identity-vs-unsupported categories surfaced in eval JSON; ≥99% Jelly oracle-span coverage gate; CRLF/LF normalization fixture; public-surface-leak CI gate also lands here.

### Phase 43: Reachability, Roots, and Per-Suite Scoring Mode
**Rationale:** Per-suite scoring mode is foundational; without it Go precision is invisible. Determinism gate installed here so every subsequent solver phase inherits it.
**Delivers:** `analysis::reachability` (root discovery from v1.2 entrypoints, BFS over semantic graph skeleton, per-suite policy); `scoring_mode` field in suite manifests; **determinism gate fixture**.

### Phase 44: Semantic Graph Skeleton
**Rationale:** Skeleton needed before frontends can emit into it. Constraint vocabulary defined here.
**Delivers:** `analysis::semantic_graph` with typed `NodeKind`/`EdgeKind`, indexes, validation, provider manifest, cache key. `Constraint` enum vocabulary (`CopyEdge`, `Alloc`, `FieldLoad`, `FieldStore`, `CallConstraint`, `ModelEdge`, `TypeConstraint`). Dependency index design lands here.

### Phase 45 (parallel with 46): JS/TS Inventory + Exact Spans + Scope + Module Graph + Direct Binding
**Rationale:** Each layer roughly doubles recoverable Jelly recall. Can run in parallel with Go semantic frontend.
**Delivers:** `src/ts/inventory/` (exact callsite/function enumeration with Jelly-shaped spans); `src/ts/scope/` (bindings, imports CJS+ESM+tsconfig aliases, module graph); direct call emission as `CopyEdge` + `CallConstraint`s.

### Phase 46 (parallel with 45): Go Semantic Frontend + Sidecar
**Rationale:** Largest single Go workstream; gates Go RTA.
**Delivers:** `crates/polint/go-sidecar/polint-go-frontend/` Go binary; `src/go/semantic/{sidecar_client,facts,lower}.rs`; sidecar process boundary (typed protocol, timeout, cancellation, version pinning); failure-mode categories; sidecar binary digest in cache keys.

### Phase 47: Unified Solver Core
**Rationale:** Generalization of v1.2's `points_to::solver`. Folds in `points_to` as a sub-domain.
**Delivers:** `analysis::solver` (`ConstraintStore`, deterministic `VecDeque` worklist, `SolverBudget`, `BudgetStatus`); language-neutral solver iteration; `DerivedEdgeProvenance` contract; per-language `SolverPolicy` trait scaffolding.

### Phase 48 (parallel with 49): Go RTA Driver
**Rationale:** Main Go recall lever. Hand-rolled RTA in Rust over emitted SSA-like facts (recommended starting point per STACK.md).
**Delivers:** `analysis::solver::go_rta` (reachability fixpoint, address-taken tracking, dynamic dispatch by signature, interface invoke resolution).

### Phase 49 (parallel with 48): JS/TS Function-Token Propagation Driver
**Rationale:** Main Jelly recall lever. Token caps + allocation-site abstraction essential from day one.
**Delivers:** `analysis::solver::ts_tokens`; per-variable token cap with `"too-many-tokens"` sentinel; `BudgetExceeded` reporting; memory-ceiling fixture.

### Phase 50: JS/TS Object/Property/`this` Model + Driver
**Rationale:** Largest precision/cost tradeoff. Ships behind a capability flag until benchmark gates approve.
**Delivers:** `src/ts/object_model/` + `analysis::solver::ts_object_model` (allocation-site abstraction, bounded property buckets, prototype-walk termination, `this` for arrow vs. method/constructor/bound/`call`/`apply`).

### Phase 51: Adaptation Model Layer
**Rationale:** Must come *after* the solver is capable; premature adaptation produces flooding incentives.
**Delivers:** `analysis::adaptation/` (TOML schema, loader, validator confirming target symbols exist, `ModelEdge` constraint emission); `benchmark adapted` mode with prompt hash + accepted/rejected counts + delta + held-out subset reporting; **sandbox**: adaptation agent runs in a directory containing only the SUT.

### Phase 52: Refined-Calls Provider Rework + Unknown Taxonomy Consolidation
**Rationale:** Once solver-derived edges are reliable, retire v1.2's heuristic refiners.
**Delivers:** `refined_calls::provider` rewrite (consumes solver output); `analysis::unknown_taxonomy` consolidation; diagnostic queue via `polint inspect unknowns --format json`.

### Phase 53: Cache + Budgets Consolidation
**Rationale:** Per-family caches and budgets are designed in each prior phase but consolidated here.
**Delivers:** Audit of every new provider's `cache_key.rs`; consolidated budget config; cold/warm RSS thresholds in benchmark report.

### Phase 54: Benchmark Promotion Gate Extension
**Rationale:** Final exit gate for v1.3. Precision floors, per-language deltas, held-out subsets, F-score β=0.5 alongside F1.
**Delivers:** `polint-bench` extension; hard gates on precision floor + per-language deltas; v1.3 milestone audit.

### Phase Ordering Rationale

- **Identity-first (Phase 42) is non-negotiable.** Every downstream metric is unmeasurable until renderers + dedup are correct.
- **Reachability (Phase 43) installs the determinism gate.** Subsequent solver phases inherit it automatically.
- **Semantic graph skeleton (Phase 44) defines the constraint vocabulary** so 45/46/47/48/49/50/51 all emit into a stable shape.
- **45 and 46 in parallel** because JS frontend work and Go sidecar work share no Rust modules.
- **47 (solver) needs at least 45's constraints to validate.** Folds in `points_to` as a sub-domain.
- **48 and 49 in parallel** because Go RTA and JS token solver share the solver but their drivers are independent.
- **50 after 49** because object/property modeling depends on token propagation being stable.
- **51 only after at least one driver is functional.** Premature adaptation produces flooding incentives.
- **52 (refined-calls rework + taxonomy)** preserves v1.2 downstream contracts and finalizes diagnostic queue shape.
- **53** is the consolidation sweep; **54** enforces the v1.3 exit gates.

### Research Flags

**Phases likely needing deeper research during plan-phase:**

- **Phase 46 (Go semantic frontend):** Wire-format details for sidecar NDJSON; the open question about whether to consume `rta.Analyze` output directly vs. emit SSA-like facts and reimplement RTA in Rust. Plan-phase research should lock the sidecar JSON schema.
- **Phase 48 (Go RTA driver):** Algorithmic edge cases — generics edge identity, init-order edges, reflection handling. Plan-phase research should map x/tools' specific test expectations to RTA implementation choices.
- **Phase 50 (JS object/property model):** Bounded property widening strategy — the largest precision/cost tradeoff space in v1.3.
- **Phase 51 (adaptation model layer):** Sandboxing mechanism; held-out subset selection per suite; prompt sanitizer specification.

**Phases with standard patterns (skip plan-phase research):**

- **Phase 42:** Identity + renderers have direct v1.2 precedents in `analysis::ids` + `FactMeta`.
- **Phase 43:** Reachability + roots are standard graph algorithms.
- **Phase 44:** Semantic graph skeleton follows v1.2 fact-family layout convention.
- **Phase 47:** Solver generalizes v1.2 `points_to::solver` directly.
- **Phase 52:** `refined_calls::provider` rework is pure projection work.
- **Phase 53, 54:** Consolidation phases — patterns already exist in v1.2.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All new crates verified against crates.io on 2026-05-27; sidecar architecture rationale draws on validated v1.2 precedent (`polint-go-symbols`) and the established gopls/staticcheck pattern. |
| Features | HIGH | Feature decomposition derived from in-repo research and the v1.2 phase ledger. Anti-features explicitly cataloged with rejection rationale. |
| Architecture | HIGH | Module layout grounded in existing v1.2 layout. Architectural keystone is the convergent shape of Soufflé/Doop/Wala/OPAL/Unimocg/SVF. Build order matches the source research's Recommended Implementation Order. |
| Pitfalls | HIGH | 12 pitfalls all grounded in v1.2 phase precedents or explicit warnings in the source GRAPH-ENGINE-BENCHMARK-RESEARCH. Mitigations have direct fixture-design implications and are mapped to specific phases. |

**Overall confidence:** HIGH.

### Gaps to Address

- **Sidecar JSON schema not yet pinned.** Phase 46 plan-phase research must finalize line termination, schema version field shape, partial-output framing, and the consume-`rta.Analyze`-directly vs. emit-SSA-facts decision.
- **Adaptation model TOML schema not yet pinned.** Phase 51 plan-phase research must finalize required vs. optional fields, confidence enum granularity, target-pattern syntax, external-boundary declaration shape, evidence-field shape.
- **Sandbox mechanism for adaptation agent not yet specified.** Phase 51 needs a concrete decision on directory isolation, env-var allowlist, filesystem-deny rules.
- **Per-language solver policy boundary not yet mapped.** Phase 47 should introduce the `SolverPolicy` trait; actual divergence points are decided in Phase 48/49.
- **JS object-model property widening threshold.** Phase 50 plan-phase research must pick a default `max_property_buckets`.
- **`x/tools` RTA quirks specific to test fixtures.** Phase 48 plan-phase research should enumerate the specific Go x/tools testdata cases.

## Sources

### Primary (HIGH confidence)
- `.planning/PROJECT.md` — v1.2 phase ledger, v1.3 milestone definition, constraints, key decisions.
- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — current baselines, target band projections, complexity bounds, Recommended Implementation Order, Adaptation Boundary anti-patterns.
- `research/call-graphs/{RECOMMENDED_IMPLEMENTATION.md, implementation/BOOTSTRAP-INTEGRATION.md, FINAL-REPORT.md, STANDARD.md}` — native-engine principle, refused dependencies list, fact-model architecture, algorithm provenance ladder.
- `research/type-alias-points-to/{RECOMMENDED_IMPLEMENTATION,FINAL-REPORT}.md` — bounded Andersen solver shape, type/value/places/alias substrate, "use language tooling as compatibility authority" principle.
- `research/agent-extension-surface/FINAL-REPORT.md` — adaptation model schema, validated-models-as-data, accept/reject reporting.
- `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md` — kernel facade, provider manifests, scheduling.
- `research/evaluation-harness/STANDARD.md` — adaptation rule (no expected labels), reporting requirements, scope rule.
- `research/evaluation-harness/decisions/decision-log.md` — D1–D6 strategy decisions.
- v1.2 phase artifacts: `crates/polint/src/analysis/{points_to::solver, refined_calls::{provider,cache_key}, ids, calls, summaries}/` — direct precedents being generalized.
- crates.io API (queried 2026-05-27) — all stack version pins verified.
- pkg.go.dev `golang.org/x/tools/go/packages` + `go/ssa` (queried 2026-05-27) — sidecar approach validated against gopls/golangci-lint/staticcheck precedent.

### Secondary (MEDIUM confidence)
- `golang/tools cmd/callgraph/main.go` — per-algorithm selection (Go-side reference).
- `golang/go issue #61160` — open issue confirming reflection edges remain hard.
- `cs-au-dk/jelly` repository — JS/TS function-token analyser format.
- `opalj/JCG` — JVM call-graph benchmark methodology.

---
*Research completed: 2026-05-27*
*Ready for roadmap: yes*
