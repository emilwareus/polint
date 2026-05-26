# Requirements: polint Static Analysis Engine Implementation

**Defined:** 2026-05-16
**Core Value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.
**Source of Truth:** `research/ROADMAP.md`, "Implementation Roadmap: One PR Per Step"

## v1.2 Requirements

Requirements for the Static Analysis Engine Implementation milestone. Each requirement maps to exactly one shippable phase, preserving the PR order from `research/ROADMAP.md`.

### Foundation

- [x] **SAE-FND-01**: polint has a private analysis kernel facade with provider manifests for existing source, Go syntax, TS/JS syntax, module graph, symbol graph, and metrics providers, preserving current behavior.
- [x] **SAE-FND-02**: Existing fact families carry internal provenance, precision, confidence, validation status, stable-key metadata, and deterministic merge validation.
- [x] **SAE-FND-03**: polint has an internal evaluation harness MVP with deterministic expected/observed JSON, matchers, metrics, and native fixtures for kernel, provenance, cache, and extension invariants.
- [ ] **SAE-FND-04**: polint records input snapshots, typed cache keys, provider output metadata, cache stats, and lifecycle/toolchain/rule/model digest inputs needed for correct cache invalidation.
- [x] **SAE-FND-05**: Existing cheap fact layers persist through a conservative layer cache with dependency indexes, change sets, hit/miss reporting, and stale-reuse safeguards.
- [x] **SAE-FND-06**: Rule macro metadata generates rule manifests, `polint inspect rule --format json` is available as an intentional CLI surface, and the first `polint test` fixture runner proves public-SDK rule behavior.

### Semantic Backbone

- [x] **SAE-SEM-01**: The semantic index includes scopes, richer imports, resolution facts, aliases, generated-symbol hooks, unresolved references, stable export identities, and language-owned Go and TS/JS providers.
- [x] **SAE-SEM-02**: The module/package/topology graph models workspace roots, packages/projects/source sets, declared requirements, lockfile/tool-resolved edges, import-to-package facts, and repo topology overlays for Go and TS/JS.
- [x] **SAE-SEM-03**: polint has a private semantic MIR and normalized place identity for Go and TS/JS function bodies, with deterministic lowering snapshots and explicit unsupported operations.
- [x] **SAE-SEM-04**: polint builds local CFG, dominance, postdominance, and control-dependence facts over MIR for supported Go and TS/JS constructs.
- [x] **SAE-SEM-05**: polint records direct call-site, direct target, and unresolved-call facts with call indexes and debug snapshots while keeping public whole-program call graph views unsupported.

### Interprocedural Substrate

- [x] **SAE-INT-01**: polint has a P0 abstract-domain kernel with lattice/transfer traits, deterministic worklist solving, and first local domains for reachability, nilness/nullishness, truthiness, constants, simple strings, and cheap initializedness.
- [x] **SAE-INT-02**: polint has a summary kernel with summary keys, typed summary domains, local/direct summaries, control effects, return/TITO, memory-touch approximations, resource/external effects, and summary metadata.
- [ ] **SAE-INT-03**: polint has an internal demand-query layer, summary SCC scheduling/cache, extension-aware cache quarantine, and query trace/debug output for expensive analyses.
- [x] **SAE-INT-04**: polint has a repo-local Rust extension/provider sink with typed sinks, declared read sets, validation, precision ceilings, provenance, activation status, fixture requirements, and cache-key participation.
- [ ] **SAE-INT-05**: polint models framework entrypoints, lifecycle callbacks, dispatch, jobs, CLIs, MCP tools/resources/prompts, tests, generated dispatch, and trust boundaries with Go and TS/JS defaults plus extension overlays.

### Precision

- [x] **SAE-PREC-01**: polint has a P0 type/value/place/alias substrate with declared/inferred/narrowed type facts, value/allocation facts, access-path facts, local narrowing, and explicit alias statuses.
- [ ] **SAE-PREC-02**: polint has opt-in refined call graph providers over direct calls, entrypoints, summaries, type/value facts, function tokens, receiver types, and bounded points-to constraints with explicit unresolved and budget-exceeded statuses.
- [x] **SAE-PREC-03**: polint has local and summary-projected data-flow facts, source/sink/sanitizer/barrier model sinks, budgets, unknown/havoc facts, and query-scoped path search.
- [x] **SAE-PREC-04**: polint has internal slicing, path explanation, structured evidence nodes/edges, ranked paths, summary expansion handles, provenance-rich diagnostic evidence, and JSON/SARIF evidence rendering.

### Promotion

- [ ] **SAE-PROM-01**: polint has external benchmark adapters and promotion gates that record default-vs-extension deltas, runtime, memory, cache reuse, unknown counts, graph/path metrics, and accepted/rejected extension facts.
- [ ] **SAE-PROM-02**: Validated typed SDK query views and agent ergonomics are promoted only where contracts are proven, including bounded query builders and stable JSON for accepted public commands.

## Future Requirements

Deferred until after this implementation sequence validates the internal engine substrate:

- **SAE-FUT-01**: Public stable SDK views for any analysis family not explicitly promoted in this milestone.
- **SAE-FUT-02**: Broad language parity for Python, Java, and later adapters across all advanced analysis families.
- **SAE-FUT-03**: Watch/daemon-mode red-green incrementality beyond the native layered cache and query foundations.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Public broad analysis APIs before validation | The research roadmap requires private/internal implementation first and deliberate public promotion only when contracts are proven. |
| Replacing current user behavior while building the kernel | Each phase must preserve existing CLI, SDK, and rule behavior unless the phase explicitly promotes a reviewed contract change. |
| Perfect whole-program precision | Unknown, unsupported, setup-missing, ambiguous, and budget-exceeded states must stay observable instead of being hidden behind overconfident facts. |
| Random OSS analyzers as runtime dependencies | Official language tooling may be used where it is the compatibility source of truth, but outputs must be normalized into polint-owned facts. |
| Public whole-program graph/query surfaces before promotion gates | Internal snapshots and hidden/preview debug paths may exist, but stable public commands and SDK views require benchmark and fixture evidence. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SAE-FND-01 | Phase 20 | Complete |
| SAE-FND-02 | Phase 21 | Complete |
| SAE-FND-03 | Phase 22 | Complete |
| SAE-FND-04 | Phase 23 | Pending |
| SAE-FND-05 | Phase 24 | Complete |
| SAE-FND-06 | Phase 25 | Complete |
| SAE-SEM-01 | Phase 26 | Complete |
| SAE-SEM-02 | Phase 27 | Complete |
| SAE-SEM-03 | Phase 28 | Complete |
| SAE-SEM-04 | Phase 29 | Complete |
| SAE-SEM-05 | Phase 30 | Complete |
| SAE-INT-01 | Phase 31 | Complete |
| SAE-INT-02 | Phase 32 | Complete |
| SAE-INT-03 | Phase 33 | Pending |
| SAE-INT-04 | Phase 34 | Complete |
| SAE-INT-05 | Phase 35 | In Progress |
| SAE-PREC-01 | Phase 36 | Complete |
| SAE-PREC-02 | Phase 37 | Complete |
| SAE-PREC-03 | Phase 38 | Complete |
| SAE-PREC-04 | Phase 39 | Complete |
| SAE-PROM-01 | Phase 40 | Pending |
| SAE-PROM-02 | Phase 41 | Pending |

**Coverage:**
- v1.2 requirements: 22 total
- Mapped to phases: 22
- Unmapped: 0

---
*Requirements defined: 2026-05-16*
*Last updated: 2026-05-16 after converting `research/ROADMAP.md` implementation PRs into GSD phases*
