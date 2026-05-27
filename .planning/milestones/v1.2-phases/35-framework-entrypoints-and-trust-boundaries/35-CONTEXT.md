# Phase 35: Framework Entrypoints and Trust Boundaries - Context

**Gathered:** 2026-05-23
**Status:** Ready for planning
**Mode:** `$gsd-discuss-phase 35 --auto`

<domain>
## Phase Boundary

Phase 35 delivers native and extension-overlay facts for framework entrypoints, lifecycle hooks, dispatch edges, and trust boundaries for Go and TS/JS. It should add internal `EntrypointFact`, `TrustBoundaryFact`, `FrameworkDispatchEdgeFact`, and `UnresolvedFrameworkFact` types, a native `polint.entrypoints` provider with scoped first-tier recognizers, extension overlay integration through Phase 34 typed sinks, precision/provenance/validation/cache/eval/debug infrastructure, and honest uncertainty for unresolved framework behavior.

This phase does **not** add full data-flow sources/sinks/sanitizers/barriers, refined call graph providers, slicing/evidence, public SDK query views, or agent-scaffolded extension workflows. Phase 35 proves that framework boundary facts can enter the private analysis substrate from both native recognizers and validated extension overlays. Phase 36 adds type/value/alias facts, Phase 37 can use entrypoints for refined call graphs, Phase 38 can consume trust boundaries as data-flow sources, and Phase 41 promotes stable public ergonomics.

</domain>

<decisions>
## Implementation Decisions

### Fact Family Scope and Design

- **D-01:** Introduce four internal fact families in Phase 35: `EntrypointFact`, `TrustBoundaryFact`, `FrameworkDispatchEdgeFact`, and `UnresolvedFrameworkFact`. These are the minimum set needed to prove the framework boundary layer end-to-end. Defer `FrameworkComponentFact`, `RegistrationFact`, `LifecycleFact`, `FrameworkSourceFact`, and `FrameworkSinkBoundaryFact` to later phases that need richer component/lifecycle modeling.
- **D-02:** `EntrypointFact` must carry: stable_key, language, framework_id, kind (enum with HttpRoute, HttpMiddleware, McpTool, McpResource, McpPrompt, CliCommand, Test, Job, QueueConsumer, ServerlessHandler, LifecycleCallback, EventListener, GeneratedDispatch), target (function/symbol binding ref), registration span, trigger metadata (method, path, tool name, event, etc.), optional trust_boundary link, precision, confidence, validation status, provenance, and provider_id.
- **D-03:** `TrustBoundaryFact` is a separate fact family linked to entrypoints by entrypoint stable key. Each fact identifies a concrete source of untrusted data entering through an entrypoint: source_kind (PathParam, Query, Body, Header, Cookie, McpArgs, CliArgs, Env, Stdin, QueuePayload, ExternalResourceReturn, Unknown), target expression or parameter, optional access path, optional protocol, precision, and provider_id.
- **D-04:** `FrameworkDispatchEdgeFact` represents synthetic call edges from framework/protocol roots to handlers/middleware/callbacks. Carries: from (dispatch source), to (target ref), edge_kind (RouteDispatch, MiddlewareChain, LifecycleHook, EventDispatch, McpDispatch, TestRunner, JobScheduler), optional guard/route metadata, optional ordering, and precision. These edges are consumed by Phase 37 refined call graph providers.
- **D-05:** `UnresolvedFrameworkFact` captures explicitly unknown or unsupported framework behavior: dynamic routes, unknown wrappers, unresolved handlers, missing setup, unsupported framework versions, and budget-exceeded recognizer states. Must carry reason, evidence, and the scope of uncertainty (which entrypoints or registrations are affected).

### Default Recognizer Tier

- **D-06:** Go default recognizers cover `net/http` and `chi` (the two most common Go HTTP frameworks). Recognition is Tier 0-1: linear scan of import tables, call expressions matching known registration patterns (`http.HandleFunc`, `http.Handle`, `r.Get/Post/Put/Delete/...` for chi), and direct handler function binding. Do not implement deep router builder tracking or middleware ordering in Phase 35.
- **D-07:** TS/JS default recognizers cover Express and MCP TypeScript SDK. Express recognition: `app.get/post/put/delete/use/route` calls, `Router()` creation and method calls. MCP SDK recognition: `server.tool/resource/prompt` registration calls. Recognition is Tier 0-1: import/require detection, call expression matching, literal argument extraction for routes/tool names. Do not implement Fastify, Nest decorator metadata, Koa, or Hapi in Phase 35.
- **D-08:** Go test entrypoint recognition: functions matching `Test*`, `Benchmark*`, `Example*`, `Fuzz*` naming conventions in `_test.go` files with `testing` package imports. TS/JS test entrypoint recognition: `describe/it/test` calls when test framework imports are detected (jest, vitest, mocha). These are Tier 0 linear scans.
- **D-09:** CLI entrypoint recognition for Go: `cobra.Command` registration patterns and `flag` package usage in `main` functions. CLI entrypoint recognition for TS/JS: `commander`/`yargs` patterns. These are Tier 0-1 recognizers. Emit as `CliCommand` entrypoints with `Heuristic` or `Conservative` precision depending on evidence quality.
- **D-10:** Any framework or pattern not covered by default recognizers should produce `UnresolvedFrameworkFact` rows where the recognizer detects framework presence (import detection) but cannot resolve registration patterns. Do not silently skip unrecognized frameworks.

### Provider Placement and Architecture

- **D-11:** Add a single `polint.entrypoints` provider running after `polint.calls` and before `polint.extensions` in the provider DAG. The provider consumes: source files, imports, symbols, references, module graph, semantic MIR, and direct call facts. Language-specific extraction (Go recognizers, TS/JS recognizers) runs behind the shared provider boundary with normalized output.
- **D-12:** The provider follows the established Phase 30-32 pattern: extract → normalize → output_digest → store → metadata refresh → validate → debug. All four fact families are emitted as a single provider output, normalized deterministically by sorted stable keys, and assigned run-local dense IDs after normalization.
- **D-13:** Provider manifest declares inputs, outputs (entrypoints, trust_boundaries, dispatch_edges, unresolved_framework), language scope (go, typescript, javascript), schema version, precision ceiling (never Exact — framework facts are at best ResolvedStatic or SetupAware), and cache inputs.
- **D-14:** Cache identity for entrypoints must include: provider/schema version, source/config/lifecycle inputs, upstream syntax output digests, symbol graph output digest, calls output digest, module topology output digest, framework-relevant manifest/config file digests (package.json, go.mod for dependency detection), and absent extension/model/toolchain slots.

### Extension Overlay Integration

- **D-15:** Phase 34 extension providers can emit `EntrypointFact`, `TrustBoundaryFact`, `FrameworkDispatchEdgeFact`, and `UnresolvedFrameworkFact` through the existing typed extension sink. Extension-emitted framework facts follow the same validation, precision ceiling, provenance, and merge rules as other extension facts from Phase 34.
- **D-16:** Extension framework facts merge after native `polint.entrypoints` output. Merge policy is additive set union by normalized stable key. Extension facts cannot delete or suppress native entrypoint facts. Conflicting facts (same stable key, different payload) produce `polint/model` validation diagnostics and keep the native fact.
- **D-17:** Extension-emitted entrypoints that resolve previously unresolved native framework registrations should reduce unknown counts in eval reports. The eval harness should measure default-vs-extended unknown reduction for framework facts specifically.
- **D-18:** Extension framework facts carry `ExtensionFactPrecision` labels from Phase 34 (SetupAware, Heuristic, GeneratedUnvalidated) and cannot claim `Exact` unless validation evidence supports it.

### Trust Boundary Representation

- **D-19:** Trust boundaries are per-entrypoint, per-source-kind facts. A single HTTP route entrypoint may have multiple trust boundary facts (one for path params, one for query string, one for body, one for headers). Each identifies the concrete parameter or expression where untrusted data enters the function.
- **D-20:** Source kinds for Phase 35: `PathParam`, `QueryString`, `RequestBody`, `RequestHeader`, `Cookie`, `McpArguments`, `McpResourceUri`, `CliArgs`, `CliFlags`, `EnvVar`, `Stdin`, `QueuePayload`, `ExternalReturn`, `Unknown`. The `Unknown` kind is mandatory for entrypoints where the recognizer detects a boundary but cannot determine the specific source kind.
- **D-21:** Trust boundary precision follows entrypoint precision. A `Heuristic` entrypoint cannot have `ExactStatic` trust boundaries. Trust boundaries for extension-discovered entrypoints carry the extension's precision ceiling.
- **D-22:** Trust boundary facts are consumed by Phase 38 (data flow) as taint sources. Phase 35 only produces the facts; it does not wire them into data-flow propagation.

### Validation, Cache, Debug, and Evaluation

- **D-23:** Extend metadata validation for all four framework fact families. Validation should catch: dangling function/symbol binding references, invalid spans, duplicate stable keys, precision ceiling violations (no Exact from framework recognizers), missing provenance, conflicting entrypoint registrations for the same handler, and malformed trigger/source metadata.
- **D-24:** Add internal debug snapshots with entrypoint counts by language, framework, kind, status, and precision. Dispatch edge counts and unresolved framework counts. Snapshots avoid raw source bodies, absolute paths, parser IDs, and timestamps.
- **D-25:** Add deterministic eval fixtures covering: HTTP route recognition (Go net/http, chi; TS/JS Express), MCP tool/resource/prompt recognition (TS/JS), test entrypoint recognition (Go, TS/JS), CLI entrypoint recognition (Go, TS/JS), trust boundary source kinds for HTTP and MCP, unresolved framework dispatch, extension-overlay entrypoint improvement, and deterministic cold/warm/no-cache three-way equality.
- **D-26:** Add public no-leak proof. Normal `polint check` JSON/help, SDK exports, README/docs must not expose private framework fact internals, provider IDs, or debug terms. Keep `Entrypoints<'_>` SDK view deferred to Phase 41 unless a minimal preview is needed for fixture crates.

### Public Surface and Deferrals

- **D-27:** Keep all framework fact families, the entrypoints provider, recognizers, trust boundary facts, and dispatch edges crate-private in Phase 35. No new SDK view, CLI command, JSON field, or docs promotion. Public `Entrypoints<'_>` SDK view is deferred to Phase 41.
- **D-28:** Do not add framework-aware refined call graph edges, data-flow source/sink wiring, middleware ordering fixpoints, or inter-file builder summary tracking in Phase 35. These belong to Phases 37-38.

### Claude's Discretion

- The planner may choose exact Rust module layout (e.g., `analysis/entrypoints/{mod,facts,recognizers,store,provider,validation,debug}.rs` or `analysis/framework/...`) as long as visibility stays crate-private.
- The planner may decide whether Go and TS/JS recognizers are separate files/modules or methods on a shared recognizer type, provided language-specific logic is isolated behind the normalized fact output.
- The planner may decide whether to add a new `FactFamily::Entrypoint` or use `FactFamily::FrameworkEntrypoint` / similar naming, provided eval, metadata, validation, and cache all correctly handle the new families.
- The planner may decide whether dispatch edge facts are emitted in the same provider pass as entrypoints or as a post-pass over entrypoint + call facts, provided output is deterministic.
- The planner may decide whether to add `ProviderKind::FrameworkAnalysis` or reuse `ProviderKind::WholeRepoDerived` for the entrypoints provider.
- The planner may split work across: (1) fact contracts and store, (2) provider/cache/manifest wiring, (3) Go recognizers, (4) TS/JS recognizers, (5) trust boundary extraction, (6) extension overlay integration, (7) validation/debug/eval/no-leak proof — as long as each plan is independently reviewable and compiling.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 35 goal, requirement mapping (SAE-INT-05), research links, and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-INT-05 requirement text and traceability.
- `.planning/PROJECT.md` — v1.2 boundaries, public API discipline, and extension-surface milestone intent.

### Framework Entrypoints Research

- `research/framework-entrypoints/FINAL-REPORT.md` — Core fact families, precision tiers, state-of-art convergence, first-tier scope, and product-specific extension model.
- `research/framework-entrypoints/RECOMMENDED_IMPLEMENTATION.md` — Provider DAG architecture, EntrypointFact/TrustBoundaryFact/FrameworkDispatchEdgeFact sketches, merge semantics, cache key vocabulary, and repo-local Rust provider surface.
- `research/framework-entrypoints/VALIDATION.md` — Ground truth shape, matching modes (strict/loose), metrics (precision, recall, binding accuracy, source-object coverage, unknown rate/reduction, extension delta, cache determinism), and complexity budgets (Tier 0-4).

### Upstream Phase Decisions

- `.planning/phases/34-rust-extension-provider-sink/34-CONTEXT.md` — Extension host, typed sinks, validation/merge gates, provenance/precision ceilings, cache quarantine, and default-vs-extended eval. Phase 35 extension overlays build directly on this boundary.
- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md` — Demand-query substrate, SCC scheduling, extension-aware quarantine. Framework facts participate in quarantine when extension-contributed.
- `.planning/phases/32-summary-kernel-and-direct-summaries/32-CONTEXT.md` — Summary kernel, direct summary store. Framework entrypoint summaries are deferred but the summary store is available for future use.

### Existing Implementation

- `crates/polint/src/analysis_kernel/provider.rs` — Provider manifests, provider kinds (SourceDiscovery, LanguageSyntax, WholeRepoDerived, MetricsDerived), 13-provider execution order, and schema version vocabulary. Phase 35 inserts `polint.entrypoints` after `polint.calls`.
- `crates/polint/src/analysis_kernel/mod.rs` — Kernel run sequence, provider scheduling, and integration point for new providers.
- `crates/polint/src/analysis/calls/facts.rs` — CallSiteFact, CallTargetFact, UnresolvedCallFact, CallAlgorithm::FrameworkModel, CallProvenance::Extension, and UnresolvedCallReason::FrameworkDispatch — vocabulary already reserved for framework-aware call edges.
- `crates/polint/src/analysis/extensions/` — Extension host, protocol, sinks (ExtensionFactCandidate), discovery, validation, and provider infrastructure from Phase 34.
- `crates/polint/src/analysis/extensions/sinks.rs` — ExtensionFactCandidate, ExtensionFactPrecision, ExtensionFactConfidence, ExtensionFactStatus — the typed sink Phase 35 extensions emit through.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` — LayerKind, cache key vocabulary with extension digest slots.
- `crates/polint/src/analysis_kernel/incremental/quarantine.rs` — Cache-level quarantine store for extension-influenced entries.
- `crates/polint/src/analysis_kernel/metadata.rs` and `crates/polint/src/analysis_kernel/validation.rs` — Fact metadata, provider manifest validation, precision ceiling checks, and diagnostics patterns.
- `crates/polint/src/eval/` and `tests/eval-fixtures/` — Internal eval fixture format, observation, and deterministic comparison patterns.
- `AGENTS.md` and `docs/API-VISIBILITY-PLAN.md` — Public API visibility rules and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::calls::facts` already defines `CallAlgorithm::FrameworkModel` and `CallProvenance::Extension` — framework dispatch edges can reference these when wiring into the call graph in Phase 37.
- `analysis::calls::facts` already defines `UnresolvedCallReason::FrameworkDispatch` — the unresolved call vocabulary is ready for framework-related call uncertainty.
- `analysis::extensions::sinks` provides `ExtensionFactCandidate` with fact_family, stable_key, binding_refs, span, precision, confidence, status, evidence, and payload_labels — framework extension facts can use this structure directly.
- `analysis::extensions::provider` runs after all native providers and handles discovery, handshake, validation, and merge — extension framework facts flow through this path naturally.
- Provider manifest infrastructure already records provider id, kind, inputs, outputs, language scope, cache policy, schema versions, and precision ceiling. Phase 35 adds a new manifest entry following the established pattern.
- `InputSnapshot` extension components from Phase 34 carry real extension digests that participate in cache invalidation. Framework facts from extensions are automatically quarantine-eligible.
- The eval harness has `FixtureArea::Extension`, accepted/rejected statuses, and default-vs-extension delta invariants from Phases 22/34. These support framework extension eval fixtures.

### Established Patterns

- New analysis families stay crate-private unless a phase intentionally promotes a supported surface with tests, docs, and no-leak coverage.
- Provider output is normalized deterministically by sorted stable keys, assigned metadata, validated before use, exposed to eval/debug through test-facing JSON, and kept out of normal public check JSON.
- Cache identities include provider/schema/config/lifecycle/upstream digests plus absent future slots.
- Setup gaps and unsupported capabilities produce diagnostics and block dependent execution rather than running with placeholder facts.
- Recognizers should be cheap (Tier 0-1) and emit explicit unknowns for anything beyond the scoped first tier.
- Extension facts merge after native facts; native facts are authoritative; conflicts produce diagnostics.

### Integration Points

- Insert `polint.entrypoints` provider in the kernel run sequence after `polint.calls` (position 10) and before `polint.extensions` (position 12, renumbered to 13).
- Extend `FactFamily` enum with framework-related families (Entrypoint, TrustBoundary, DispatchEdge, UnresolvedFramework, or equivalent names).
- Extend `KernelRunReport` with entrypoint provider output metadata (counts, output digest, cache stats).
- Extend metadata validation with framework fact checks (binding refs, spans, precision ceilings, duplicate keys).
- Extend eval observation with framework fact coverage (entrypoint kinds, trust boundary source kinds, dispatch edges, unknowns).
- Go recognizers consume: Go syntax facts (imports, functions, packages), symbol graph (definitions, references), and call facts (call sites matching known framework registration patterns).
- TS/JS recognizers consume: TS syntax facts (imports, functions, classes), symbol graph, call facts, and resolved imports for framework package detection.

</code_context>

<specifics>
## Specific Ideas

- Start with the simplest vertical: Go net/http `HandleFunc` + chi router method registration → EntrypointFact + TrustBoundaryFact. This exercises the full pipeline without complex framework semantics.
- Express `app.get("/path", handler)` is the TS/JS equivalent minimal vertical. Literal route path + handler function binding → entrypoint + trust boundaries for req.params, req.query, req.body.
- MCP TypeScript SDK `server.tool("name", handler)` is a high-value recognizer because MCP tools are the AI-agent-era entrypoint and trust boundary; having this in the first tier differentiates polint.
- Test entrypoints are cheap wins: Go `Test*` naming convention and TS/JS `describe/it/test` imports are Tier 0 linear scans that add immediate value for analysis reachability.
- Use the Phase 34 extension fixture pattern to prove that a repo-local extension can emit an entrypoint fact for a custom framework not covered by default recognizers, with validation and unknown reduction evidence.

</specifics>

<deferred>
## Deferred Ideas

- Type/value/place/alias facts and framework-aware type narrowing: Phase 36.
- Refined call graph providers consuming framework dispatch edges: Phase 37.
- Data-flow source/sink wiring from trust boundary facts, sanitizer/barrier modeling: Phase 38.
- Slicing, evidence bundles, and diagnostic evidence for framework-originated paths: Phase 39.
- External benchmark adapters for framework entrypoint precision/recall measurement: Phase 40.
- Public `Entrypoints<'_>` SDK view, agent scaffolding for framework providers, and stable public framework ergonomics: Phase 41.
- Fastify, Nest, Koa, Hapi, gin, echo, gorilla/mux recognizers beyond chi: future phase or extension overlay.
- Middleware ordering, lifecycle composition, and inter-file builder summaries: later phases when component graph modeling is justified.
- FrameworkComponentFact, RegistrationFact, LifecycleFact, FrameworkSourceFact, FrameworkSinkBoundaryFact: later phases that need richer component/lifecycle models.

</deferred>

---

*Phase: 35-framework-entrypoints-and-trust-boundaries*
*Context gathered: 2026-05-23*
