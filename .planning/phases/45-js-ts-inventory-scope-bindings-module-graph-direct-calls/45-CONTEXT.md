# Phase 45: JS/TS Inventory, Scope, Bindings, Module Graph & Direct Calls - Context

**Gathered:** 2026-05-31
**Status:** Ready for planning
**Mode:** `/gsd-discuss-phase 45 --auto`

<domain>
## Phase Boundary

Phase 45 is the JS/TS frontend slice for the v1.3 graph engine. It delivers exactly the JS-01, JS-02, and JS-03 requirements:

1. Enumerate JS/TS functions and callsites with benchmark-grade Jelly-shaped spans.
2. Build private lexical scope, binding, import/export, and module-graph facts for ESM, CommonJS, and TypeScript path aliases.
3. Emit direct JS/TS binding evidence into the private semantic graph as `CopyEdge` and `CallConstraint` constraints.

This phase is a producer phase, not a solver phase. It does not implement JS/TS function-token propagation (Phase 49), object/property/prototype/`this` modeling (Phase 50), the unified solver core (Phase 47), Go semantic frontend work (Phase 46), adaptation models (Phase 51), unknown-taxonomy CLI surface (Phase 52), or any public SDK promotion. All new JS/TS modules and facts stay `pub(crate)`, and the public-surface-leak gate from Phase 42 must remain green.

</domain>

<decisions>
## Implementation Decisions

### JS/TS Inventory And Jelly Span Parity

- **D-01:** Add a private `crates/polint/src/ts/inventory/` module for richer JS/TS inventory facts. Reuse Oxc parsing and `SourceType` handling already present in `ts::adapter` and `symbol_graph::ts`; do not create a second parser stack or benchmark-specific scanner.
- **D-02:** Inventory must cover the roadmap's full function set: declarations, function expressions, arrow functions, methods, constructors, accessors, class static blocks, and any generated implicit functions needed for honest Jelly identity. It must also cover the callsite set: calls, `new`, tagged templates, optional calls, dynamic import, and `require`.
- **D-03:** Use Oxc byte spans converted through the existing `span_from_byte_range` / `span_from_oxc` pattern, then rely on the Phase 42 Jelly renderer for final `file:start_line:start_col:end_line:end_col` output. Do not normalize spans by hand in the benchmark adapter.
- **D-04:** If an AST form cannot be inventoried with stable source identity, emit an explicit unsupported or unresolved row rather than fabricating a callsite/function identity. The phase should reduce "unknown location" failures, not trade them for false positives.
- **D-05:** Dense IDs are assigned only after stable-key sorting. Stable keys must include file identity, Oxc span, lexical parent/scope identity, syntactic form, and display name where available; never use run-local insertion order as a persisted identity input.

### Scope And Binding Facts

- **D-06:** Add a private `crates/polint/src/ts/scope/` module for lexical scope and binding facts. Prefer `oxc_semantic::SemanticBuilder` / Oxc scoping data where it is available; use AST fallback only for forms Oxc semantic does not expose cleanly.
- **D-07:** Scope/binding coverage must include `var`, `let`, `const`, function declarations, function expressions, classes, imports, destructuring, parameters, catch bindings, re-exports, namespace imports, default imports, named imports, and aliases. Hoisting-sensitive behavior (`var` vs `let`/`const`, function declarations vs expressions) should be represented explicitly enough for Phase 49 to consume.
- **D-08:** Binding facts should have honest statuses such as present, unresolved, unsupported dynamic, and external. Computed property names, `eval`, non-string dynamic imports, and broad native/library behavior should stay unresolved/unsupported unless this phase can prove a direct static binding.
- **D-09:** Direct binding is intentionally narrow: `f()`, `ns.f()`, imported aliases, re-exported aliases, local aliases, and statically resolved module members. Do not implement token propagation through parameters, returns, closures, arrays, object properties, prototypes, or `this`; those are Phase 49/50.

### Module Graph And Import Resolution

- **D-10:** Reuse the existing `module_graph::ts`, `module_graph::model`, `oxc_resolver`, topology, `ResolvedImportFact`, and module topology cache machinery. Do not build a parallel JS/TS resolver inside `ts/inventory` or `ts/scope`.
- **D-11:** ESM, CommonJS, dynamic import, `require`, package entrypoints, workspace packages, and TypeScript path aliases should be represented through the existing module graph where possible, then bridged into binding facts by ID/reference. Module graph facts remain the authority for file/package/module resolution.
- **D-12:** Cache identity for new JS/TS scope/binding layers must include source syntax output, semantic/scope output, module graph output, tsconfig/jsconfig and extends inputs, package/workspace/lockfile inputs, relevant config/lifecycle inputs, and provider/schema versions. Follow the v1.2/v1.3 dependency-index style; do not hide resolver inputs outside digests.

### Semantic Graph Constraint Emission

- **D-13:** Phase 45 emits direct JS/TS binding evidence into the existing `analysis::semantic_graph` vocabulary from Phase 44. Use `CopyEdge` for static aliases/binding copies and `CallConstraint` for direct call obligations anchored at callsite nodes.
- **D-14:** Constraints reference existing semantic graph node IDs or existing stable fact identities; they must not duplicate function, callsite, module, or package identity payloads. This continues the Phase 44 composition-over-duplication rule.
- **D-15:** If direct call targets are projected into the existing calls/refined-calls contract, preserve the v1.2 fact shapes and label the algorithm/source honestly as direct JS/TS binding. Otherwise, keep this phase limited to constraints and private debug/snapshot rows. In either case, no public whole-program call graph view is introduced.
- **D-16:** No solver-derived edges are emitted in this phase. A binding that needs token flow, callback propagation, property flow, prototype lookup, or `this` modeling remains unresolved/unsupported with a reason that later phases can consume.

### Verification And Acceptance

- **D-17:** Add native fixtures covering every required function/callsite form, including optional calls, tagged templates, `new`, dynamic import, `require`, class methods, constructors, accessors, class static blocks, arrow functions, and nested function expressions.
- **D-18:** Add module/binding fixtures for ESM, CommonJS, TypeScript path aliases, namespace imports, default/named imports, re-exports, destructuring, local aliases, and unresolved dynamic imports.
- **D-19:** Add semantic-graph snapshot fixtures asserting emitted `CopyEdge` and `CallConstraint` rows are stable, sorted, and reference resolvable graph nodes. The Phase 43 determinism gate and Phase 42 public-surface-leak gate are required acceptance checks.
- **D-20:** Measure Jelly oracle-span coverage after the inventory layer changes. The goal is to preserve or improve the Phase 42 >=99% oracle-span coverage standard and make any misses explicit by fixture/case.

### Agent's Discretion

- The exact fact type names and file layout inside `ts/inventory/` and `ts/scope/` are planner decisions, provided visibility stays `pub(crate)` and the modules mirror existing analysis fact/store/provider/cache patterns.
- The planner may decide whether inventory facts are stored as a new layer, integrated into existing TS syntax payloads, or projected through a derived provider, provided cache/determinism behavior is explicit and test-covered.
- The planner may choose whether to emit direct call target facts in addition to semantic graph constraints. If it does, it must preserve existing public/SDK contracts and label precision honestly.
- The planner may split Phase 45 naturally into inventory/span coverage, scope/binding facts, module resolution bridge, semantic graph constraint emission, and verification/public-boundary proof.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 45 goal, JS-01/JS-02/JS-03 success criteria, parallel eligibility with Phase 46, and v1.3 no-public-SDK-promotion rule.
- `.planning/REQUIREMENTS.md` - JS-01, JS-02, JS-03 requirements plus JS-04/JS-05 boundaries that must remain deferred.
- `.planning/PROJECT.md` - v1.3 graph engine goal, Jelly baseline, product/private-analysis discipline, and benchmark-driven milestone framing.
- `.planning/STATE.md` - current milestone state and open repo-admin leak-gate action.

### Immediate Upstream Phase Context

- `.planning/phases/44-semantic-graph-skeleton-constraint-vocabulary/44-CONTEXT.md` - `analysis::semantic_graph`, `ConstraintKind`, `CopyEdge`, `CallConstraint`, composition-over-duplication, provider/cache/validation discipline, and public-boundary rules.
- `.planning/phases/43-reachability-roots-per-suite-scoring-mode/43-CONTEXT.md` - determinism gate inheritance and scoring-mode/reachability constraints.
- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` - Jelly span renderer, identity records, dedup, CRLF/LF normalization, and public-surface-leak gate.

### v1.3 Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` - primary source for JS/TS exact inventory, scope/binding/module graph, expected Jelly metric impact, and shared frontend-to-solver architecture.
- `research/call-graphs/FINAL-REPORT.md` - JS/TS call graph direction: Oxc call-site and binding facts first, then bounded function-token value flow; unresolved/dynamic facts stay first-class.
- `research/evaluation-harness/STANDARD.md` - suite-native and unified benchmark report requirements.
- `research/evaluation-harness/decisions/decision-log.md` - external-benchmark-first and hidden/internal-first evaluation decisions.

### Existing Implementation Touch Points

- `crates/polint/src/ts/adapter.rs` - existing Oxc parser usage, syntax extraction, `span_from_oxc`, imports/exports, CommonJS `require`, dynamic import, functions/classes/calls, and TS syntax layer cache.
- `crates/polint/src/symbol_graph/ts.rs` - Oxc semantic/scoping integration, symbol/reference extraction, scopes, imports, aliases, exports, and semantic stable-key patterns.
- `crates/polint/src/module_graph/ts.rs` - TS/JS topology, package/workspace discovery, `oxc_resolver`, tsconfig path aliases, CommonJS/ESM resolution behavior.
- `crates/polint/src/module_graph/model.rs` - module graph builder, `ModuleNodeId`, `ResolvedImportFact`, node/edge insertion and deterministic graph conventions.
- `crates/polint/src/analysis/semantic_graph/{facts.rs,constraints.rs,build.rs,provider.rs,store.rs,validate.rs}` - semantic graph node/edge/constraint vocabulary and validation/store patterns.
- `crates/polint/src/analysis/calls/{facts.rs,provider.rs,store.rs,validate.rs}` - existing callsite/target/unresolved fact contracts that Phase 45 must not break.
- `crates/polint/src/analysis/identity/render/jelly_span.rs` - canonical Jelly span rendering; Phase 45 should feed it accurate spans rather than duplicate renderer logic.
- `crates/polint/tests/public_surface_leak.rs` - v1.3 public API leak gate; new JS/TS inventory/scope/constraint types must stay unreachable from `polint::sdk::prelude::*`.
- `tests/eval-fixtures/semantic-graph/` and `tests/eval-fixtures/determinism/` - snapshot/determinism fixture precedents for new constraint output.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `ts::adapter` already parses TS/JS with Oxc, converts Oxc spans through `span_from_byte_range`, extracts imports/exports, CommonJS `require`, dynamic import, functions, classes, methods, calls, literals, JSX, and stores a TS syntax layer cache.
- `symbol_graph::ts` already uses `oxc_semantic::SemanticBuilder`, Oxc scoping, symbols, references, imports, aliases, exports, and scope stable keys. This is the best starting point for Phase 45 scope/binding rather than inventing textual binding resolution.
- `module_graph::ts` and `module_graph::model` already cover JS workspaces, package manifests, lockfile evidence, tsconfig alias/reference inputs, resolver context construction, `ResolvedImportFact`, and deterministic module nodes/edges.
- `analysis::semantic_graph` already has `NodeKind`, `EdgeKind`, `ConstraintKind`, `ConstraintFact`, `build_semantic_graph`, validation, store/indexes, provider wiring, and cache-key participation.
- `analysis::identity` already owns Jelly rendering and benchmark identity normalization. Phase 45 should improve source facts feeding identity, not fork benchmark identity logic.

### Established Patterns

- New analysis modules are crate-private and validated by public-surface-leak tests.
- Provider outputs are sorted by stable key before dense ID assignment, and stable keys are composed from existing identities, not dense run-local IDs.
- Cache keys include upstream provider output digests, lifecycle/config inputs, schema/provider versions, and relevant toolchain/config files.
- Unsupported/dynamic behavior is represented explicitly; the project does not inflate recall by guessing edges.
- Native eval fixtures and snapshot fixtures are the normal proof mechanism for private analysis families.

### Integration Points

- `crates/polint/src/ts/mod.rs` should expose only crate-private submodules for inventory/scope work.
- `AnalysisKernel::run` provider order must run any Phase 45 derived provider after TS syntax/symbol/module graph inputs and before solver/refined-call consumers that need the constraints.
- `AnalysisDb` / internal stores need append/replace paths for new private facts if the planner chooses a stored fact family.
- Semantic graph provider/build logic needs a bridge from TS inventory/binding/module facts into `CopyEdge` and `CallConstraint` rows.
- Eval fixtures should exercise both the native semantic graph snapshot path and Jelly external adapter span coverage.

</code_context>

<specifics>
## Specific Ideas

- Direct binding examples to cover in fixtures: `f()`, `alias()`, `ns.f()`, `import { f as g } from "./m"; g()`, `const f = require("./m").f; f()`, `export { f as g } from "./m"`, and local destructuring aliases.
- Span fixtures should include multi-line arrows, nested function expressions, optional calls, tagged templates, class methods, getters/setters, constructors, static blocks, dynamic import, and CommonJS `require`.
- Dynamic or computed forms should produce named unresolved reasons such as non-string dynamic import, computed property, eval, external package unresolved, and unsupported native callback.
- Keep one fixture intentionally unresolved for each major unsupported category so later Phase 49/50 work can prove it converted the right unknowns into constraints.

</specifics>

<deferred>
## Deferred Ideas

- JS/TS function-token propagation through assignments, parameters, returns, callbacks, closures, and token sets belongs to Phase 49 (JS-04).
- JS/TS object/property/prototype/class/`this` modeling belongs to Phase 50 (JS-05).
- Unified solver work, solver budgets, `SolverPolicy`, and derived-edge provenance belong to Phase 47.
- Go sidecar and Go semantic lowering belong to Phase 46.
- Adaptation model facts and `ModelEdge` producers belong to Phase 51.
- Public SDK views over v1.3 semantic graph/call graph facts are explicitly out of v1.3 and deferred to v1.4+.

</deferred>

---

*Phase: 45-JS/TS Inventory, Scope, Bindings, Module Graph & Direct Calls*
*Context gathered: 2026-05-31*
