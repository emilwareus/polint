# Phase 46: Go Semantic Frontend & Sidecar - Context

**Gathered:** 2026-06-01
**Status:** Ready for planning
**Mode:** `/gsd-discuss-phase 46 --auto`

<domain>
## Phase Boundary

Phase 46 is the Go frontend producer slice for the v1.3 graph engine. It delivers exactly GO-01, GO-02, GO-03, and GO-04:

1. Ship a co-packaged `polint-go-frontend` Go sidecar backed by `go/packages`, `go/ssa`, and `golang.org/x/tools v0.45.0`.
2. Emit versioned semantic facts over a typed NDJSON stdio protocol: packages, functions, methods, receiver types, init functions, method sets, callsites, type facts, and setup errors.
3. Add the Rust sidecar client and Go semantic lowering layer under the crate-private Go analysis boundary, mapping sidecar facts into the private semantic graph and its constraint vocabulary.
4. Harden the process boundary with explicit terminators, timeouts, cancellation, one long-lived sidecar per `polint check`, clean orphan handling, sidecar/toolchain cache inputs, and distinct failure categories.

This phase is a producer and protocol hardening phase, not the Go RTA solver. It does not implement the unified solver core (Phase 47), Go RTA dynamic-dispatch fixpoint (Phase 48), refined-call projection over solver output (Phase 52), final cache/budget consolidation (Phase 53), or public SDK/CLI graph promotion. It may add private diagnostics and private unknown-taxonomy rows needed by GO-04, but `polint inspect unknowns --format json` remains Phase 52. All new Rust modules and types stay `pub(crate)` and the public-surface-leak gate must remain green.

</domain>

<decisions>
## Implementation Decisions

### Sidecar Identity, Packaging, And Relationship To Existing Go Symbols

- **D-01:** Add a distinct semantic sidecar named `polint-go-frontend`. Do not overload the existing `polint-go-symbols` schema or binary name. The existing symbols sidecar remains the source for current public symbol/reference facts; the new frontend owns richer SSA/type/call facts for the private graph engine.
- **D-02:** Reuse the proven packaging pattern from `crates/polint/go-sidecar/polint-go-symbols/`: embedded source fallback, optional installed binary beside the `polint` executable, deterministic materialization under temp, drift tests for embedded sources, and an override env var for tests/development. The override should be specific to the new binary (for example `POLINT_GO_FRONTEND`) so existing `POLINT_GO_SYMBOLS` behavior does not change.
- **D-03:** The new sidecar should use `golang.org/x/tools v0.45.0` as the roadmap-required line. The existing `polint-go-symbols` module currently uses `v0.42.0`; planner may either upgrade shared Go-sidecar dependencies when compatible or keep the two modules separate, but Phase 46 acceptance must prove the semantic sidecar is on `v0.45.0`.
- **D-04:** Keep the minimum Go toolchain policy explicit. Current Go lifecycle code and symbol-sidecar tests assume Go 1.24. Phase 46 should preserve or deliberately update that minimum in one place, document it, and include the reported Go toolchain version in sidecar output and cache inputs.
- **D-05:** The new sidecar is crate-private implementation infrastructure. Do not document it as a public extension API, do not expose raw sidecar DTOs through `polint::sdk`, and do not make rules depend on `go/packages`, `go/ssa`, or sidecar internals.

### Protocol And Process Boundary

- **D-06:** Use a typed NDJSON protocol with a versioned schema such as `polint-go-semantic-1`. Prefer request/response frames with explicit `begin`, `row`, `error`, and `end`/terminator messages over one giant JSON object, so the Rust client can detect partial output, malformed rows, and missing terminators deterministically.
- **D-07:** The Rust client should manage one long-lived `polint-go-frontend` process per `polint check` run, not one `go run` per package. It sends the resolved lifecycle/config request once and streams package/fact rows back. This satisfies GO-03 and avoids reintroducing high process startup cost in large repos.
- **D-08:** Process execution must be synchronous and deterministic from the Rust side. Do not introduce a Tokio/async runtime. Use standard process pipes/threads or equivalent bounded blocking IO, with deterministic row sorting before facts enter `AnalysisDb`.
- **D-09:** Add per-request timeout and cancellation handling at the sidecar client boundary. On timeout or parent cancellation, close stdin, send/propagate termination when possible, then kill/wait the child. A SIGTERM/orphan fixture must assert no surviving `polint-go-frontend` process after 5 seconds.
- **D-10:** Every protocol failure should become a controlled internal error or diagnostic, not a panic: unsupported schema, invalid NDJSON row, missing terminator, child exit failure, timeout, path escaping, and package-load errors all need distinct error paths.

### Go Fact Schema And Stable Identity

- **D-11:** The sidecar fact schema must include enough data for both GO-01 and the Phase 42 deferrals: package ID/path/name/module path/test variant, function and method identities, receiver type information, init functions, method sets, callsite spans, callee/candidate references, and type facts.
- **D-12:** Use official Go identities where they exist: `packages.Package.ID`, module path, package path, `go/types` object identity, `objectpath` where stable, receiver type strings, and `ssa.Function` identity for synthetic/init/anonymous functions. Do not invent names from source text when official tooling can provide them.
- **D-13:** Stable keys emitted by the Rust lowering layer must be length-prefixed/labeled and built from official Go identities plus source spans, never from run-local insertion order or dense IDs. Dense IDs are assigned only after stable-key sorting, matching Phases 42-45.
- **D-14:** Full Go module import-path qualification lands here. The `analysis::identity::provider` and Go RTA oracle comments currently defer import-path RelString behavior to Phase 46; this phase should feed the identity renderer with module/package path facts rather than continuing to rely only on package-clause names.
- **D-15:** Represent anonymous, synthetic, init, wrapper, bound method, and generic-instantiation functions honestly. If an SSA construct cannot be mapped to a stable source span or existing identity, emit an unsupported/unresolved row with a reason instead of fabricating a benchmark-matchable identity.

### Lowering Into Semantic Graph Constraints

- **D-16:** Add the Rust integration under a private Go semantic boundary, preferably `crates/polint/src/go/semantic/`, with modules for protocol DTOs, client/process management, lowering, cache inputs, validation, and tests. Wire it from `go/mod.rs` and the analysis provider DAG without exposing public SDK types.
- **D-17:** Lower Go sidecar facts into the existing `analysis::semantic_graph` model from Phase 44: packages, functions, callsites, types, and receiver/method-set concepts should compose existing graph node kinds and existing fact IDs rather than duplicating identity payloads.
- **D-18:** Emit `CallConstraint` rows for Go callsite obligations and direct/static call evidence where the sidecar can prove it. Interface dispatch, dynamic dispatch, and RTA candidate expansion should remain unresolved/unsupported constraints or candidate facts for Phase 48; do not emit solver-derived call edges in Phase 46.
- **D-19:** Emit type/receiver/method-set evidence in the narrowest vocabulary the current graph can represent. If `TypeConstraint` is sufficient, use it. If additional crate-private fact families are needed before Phase 47/48 can consume them, keep them private, stable-keyed, cache-digested, and clearly documented as Go frontend facts.
- **D-20:** The lowering layer must preserve source span exactness and validate every sidecar file path against discovered in-repository Go files, reusing the existing path validation discipline from `symbol_graph::go`. Absolute or repo-escaping paths produce setup diagnostics, not accepted facts.

### Lifecycle, Cache, And Unknown Taxonomy

- **D-21:** Reuse `go::lifecycle::GoAnalysisConfig` as the authoritative lifecycle input: inferred/configured module roots, package patterns, build tags, include_tests, offline mode, and synthetic workspace behavior. Do not introduce hidden side files or one-off flags outside `.polint.toml`.
- **D-22:** Cache identity for the Go semantic frontend must include sidecar schema/provider version, sidecar binary/source digest, Go toolchain version, `golang.org/x/tools` version, lifecycle inputs, module roots, package patterns, build tags, include_tests, offline mode, and relevant upstream provider digests.
- **D-23:** Surface GO-04 categories distinctly in private unsupported/unknown rows and diagnostics: `GoPackagesLoadFailed`, `GoVersionUnsupported`, and `GoSidecarTimeout`. Existing broad setup-missing diagnostics may wrap these for current output, but the distinct categories must be preserved for Phase 52 taxonomy consolidation.
- **D-24:** Package-load errors from `packages.Load` should be row-level when possible. A single bad package should produce explicit package error facts and let other packages emit facts if the sidecar can do so honestly; whole-run failure is reserved for protocol/toolchain/process failures that make output untrustworthy.
- **D-25:** If Go setup is missing or unsupported, rules must not run with placeholder semantic facts. Emit capability/setup diagnostics and keep the graph honest, following the AGENTS Go analysis lifecycle contract.

### Verification And Acceptance

- **D-26:** Add temp-repo or fixture tests for single-module, nested-module, multi-module, checked-in `go.work`, synthetic `go.work`, build tags, include/exclude tests, and package-load failures. These should prove lifecycle parity with `go::lifecycle`.
- **D-27:** Add semantic graph snapshot fixtures proving Go functions, methods, init functions, receiver/method-set facts, direct/static calls, and unresolved interface/dynamic calls lower into stable graph facts/constraints.
- **D-28:** Add protocol/process tests for schema mismatch, invalid NDJSON, missing terminator, timeout, cancellation, nonzero exit, orphan cleanup, and repo-escaping file paths.
- **D-29:** Add cache regression tests for sidecar digest, Go toolchain version, lifecycle settings, and x/tools version changes: positive must-invalidate cases and negative must-preserve-hit cases where unrelated inputs do not churn.
- **D-30:** Keep the inherited Phase 43 determinism gate and Phase 42 public-surface-leak gate green. New Go semantic facts must be stable under provider-order shuffles and unreachable from `polint::sdk::prelude::*`.

### Agent's Discretion

- The exact wire-frame names and DTO module layout are planner decisions, provided the protocol is versioned, NDJSON-framed, terminator-checked, and tested for malformed/partial output.
- The planner may choose whether the semantic sidecar lives as a sibling module under `crates/polint/go-sidecar/polint-go-frontend/` or shares some helper code with `polint-go-symbols`; avoid coupling the two schemas.
- The planner may choose whether to keep `polint-go-symbols` and `polint-go-frontend` as two child processes or eventually have one binary serve both commands. For Phase 46, preserve existing symbol behavior and do not make public symbol/reference facts depend on unfinished semantic graph lowering.
- Natural plan slicing: (1) sidecar module + schema + `go/packages`/`go/ssa` emission; (2) Rust client/process/protocol hardening; (3) lowering into semantic graph and identity import-path integration; (4) taxonomy/cache/determinism/public-surface/orphan verification.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 46 goal, GO-01/GO-02/GO-03/GO-04 success criteria, parallel eligibility note, and downstream dependencies.
- `.planning/REQUIREMENTS.md` - GO-01 through GO-04 text, related GO-05 boundary, CACHE-01/TAX-01 follow-on requirements, and v1.3 out-of-scope table.
- `.planning/PROJECT.md` - v1.3 graph engine milestone goal, Go x/tools RTA benchmark baseline, private-analysis-first discipline, and no-public-SDK-promotion framing.
- `.planning/STATE.md` - current milestone state, Phase 46 readiness, Phase 42 import-path deferrals, and open leak-gate repo-admin action.

### Immediate Upstream Phase Context

- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` - Go RelString renderer intent, full import-path deferral to Phase 46, identity stable-key discipline, and public-surface-leak gate.
- `.planning/phases/43-reachability-roots-per-suite-scoring-mode/43-CONTEXT.md` - Go roots, direct-call reachability boundary, lifecycle/config digest discipline, and inherited determinism gate.
- `.planning/phases/44-semantic-graph-skeleton-constraint-vocabulary/44-CONTEXT.md` - private `analysis::semantic_graph`, `ConstraintKind`, `CallConstraint`, `TypeConstraint`, stable-key/dense-ID rules, cache/provider/validation discipline.
- `.planning/phases/45-js-ts-inventory-scope-bindings-module-graph-direct-calls/45-CONTEXT.md` - sibling frontend producer pattern, direct binding to graph constraints, and no-solver/no-public-promotion boundaries.

### v1.3 Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` - primary benchmark motivation: Go x/tools RTA baseline, frontend-to-shared-graph architecture, and precision-first target.
- `research/call-graphs/FINAL-REPORT.md` - call-graph layering, Go semantic frontend motivation, and unresolved/dynamic edge handling.
- `research/type-alias-points-to/SUBAGENT-FINDINGS.md` - official Go tooling (`go/types`, `go/packages`, `go/ssa`, x/tools callgraph) as the Go authority.
- `research/cfg-control-flow/SUBAGENT-FINDINGS.md` - `go/ssa` as the best Go CFG/semantic substrate.
- `research/module-graph/PAPER-INDEX.md` - `go/packages` package loading reference.

### Existing Implementation Touch Points

- `crates/polint/src/go/lifecycle.rs` - authoritative Go lifecycle config, inferred/configured module roots, package patterns, build tags, include_tests, offline mode, and synthetic `go.work` behavior.
- `crates/polint/src/symbol_graph/go.rs` - existing Go sidecar client/materialization/path-validation/error patterns; useful as a template, not the semantic sidecar schema.
- `crates/polint/go-sidecar/polint-go-symbols/` - existing Go sidecar packaging and `go/packages` usage; preserve behavior while adding the new semantic sidecar.
- `crates/polint/src/analysis/semantic_graph/{facts.rs,constraints.rs,build.rs,provider.rs,store.rs,validate.rs,cache_key.rs}` - graph node/edge/constraint vocabulary and provider/cache/validation patterns the Go lowering writes into.
- `crates/polint/src/analysis/identity/{provider.rs,render/go_relstring.rs}` - Go package/module identity and RelString renderer; Phase 46 should complete the import-path deferrals called out in comments.
- `crates/polint/src/eval/external/go_x_tools_callgraph.rs` - Go RTA benchmark identity/scoring path with Phase 46 deferral comments.
- `crates/polint/src/analysis_kernel/provider.rs` - provider manifest/order machinery and determinism-gate enrollment.
- `crates/polint/tests/public_surface_leak.rs` - public API leak gate; new Go semantic types must stay crate-private.
- `docs/CONSUMER-SETUP.md` and `docs/facts/symbols-and-references.md` - current documented Go symbol sidecar behavior; update only if user-visible setup requirements change.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `go::lifecycle::GoAnalysisConfig` already encodes the monorepo lifecycle contract: module roots from config or nearest `go.mod`, package patterns, build tags, include_tests, offline mode, files without module roots, and synthetic `go.work`.
- `symbol_graph::go` already has embedded sidecar source materialization, installed binary detection, env override, schema validation, null-as-empty deserialization, path validation against discovered Go files, and setup diagnostics. These are strong templates for the new client.
- `crates/polint/go-sidecar/polint-go-symbols/internal/symbols/emit.go` already uses `go/packages` with syntax/types/types-info/module data, emits package/module/symbol/reference/scope/import/export rows, and handles multi-module workspace setup.
- `analysis::semantic_graph` already has `NodeKind`, `EdgeKind`, `ConstraintKind`, `ConstraintFact`, provider digesting, validation, stable-key sorting, and TS direct-binding projection into `CopyEdge`/`CallConstraint` rows.
- `analysis::identity` already owns Go RelString rendering and currently marks full module import path as a Phase 46 deliverable.

### Established Patterns

- New analysis families are private (`pub(crate)`), stable-key sorted before dense ID assignment, validated before storage, cache-digested with upstream provider/lifecycle inputs, and guarded by public-surface-leak tests.
- Setup gaps and unsupported semantics are explicit rows/diagnostics. The project does not fabricate placeholder facts or flood possible edges to improve recall.
- Provider output digests use stable content and provider/schema versions, never run-local dense IDs.
- Existing Go lifecycle behavior avoids writing generated lifecycle files into the repository; synthetic workspaces live in temp files.
- Process-boundary failures are converted into diagnostics or capability/setup gaps rather than panics.

### Integration Points

- Add a private Go semantic module under `crates/polint/src/go/semantic/` and expose it from `crates/polint/src/go/mod.rs` with crate-private visibility.
- Add a sibling sidecar source tree such as `crates/polint/go-sidecar/polint-go-frontend/` with its own schema/version and drift tests.
- Register the Go semantic frontend/lowering provider in `analysis_kernel::provider.rs` at the point where Go syntax/lifecycle/symbol/module facts are available and before semantic graph consumers that need Go constraints.
- Extend semantic graph building/provider inputs so Go semantic facts contribute nodes/constraints and participate in the output digest.
- Wire cache inputs through `analysis::cache_key` / provider-specific cache-key helpers with sidecar digest and Go toolchain version.
- Add tests near existing sidecar tests in `symbol_graph::go` or new `go::semantic` test modules, plus fixture-level semantic graph and determinism coverage.

</code_context>

<specifics>
## Specific Ideas

- Prefer a protocol shape that can stream rows and still fail closed: one request frame, many typed row frames, a final terminator frame with summary counts/digest, and schema/version fields on every session.
- Required row families should include packages, functions, methods, receiver types, init functions, method sets, callsites, type facts, candidate sets, and package/load errors.
- Include `go_version`, `x_tools_version`, sidecar schema, and sidecar build/source digest in the session summary so Rust cache-key code does not need to infer them indirectly.
- Keep at least one fixture intentionally unresolved for interface dispatch/dynamic call behavior so Phase 48 can prove it converts the right unknowns into RTA-derived edges.
- Use package row data to complete full Go import-path RelString behavior and update the Go x/tools RTA oracle path only where it improves benchmark matching without regressing existing bare-name fixtures.

</specifics>

<deferred>
## Deferred Ideas

- Unified solver core, deterministic `VecDeque` worklist, `SolverBudget`, and `SolverPolicy` are Phase 47.
- Go RTA dynamic-dispatch fixpoint, address-taken tracking, interface invoke matching, and reachable-function expansion are Phase 48.
- Refined-call projection over solver output and public `RefinedCallEdgeFact` preservation are Phase 52.
- Consolidated `polint inspect unknowns --format json` public CLI is Phase 52; Phase 46 preserves categories privately.
- Cross-family cache and solver-budget consolidation across all new v1.3 families is Phase 53.
- Public SDK views over Go semantic graph/call graph facts are out of v1.3.

</deferred>

---

*Phase: 46-Go Semantic Frontend & Sidecar*
*Context gathered: 2026-06-01*
