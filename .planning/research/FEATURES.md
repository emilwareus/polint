# Feature Research — v1.3 Graph Engine Precision

**Domain:** Multi-language static-analysis call-graph and points-to engine (Go + TS/JS), embedded inside the polint Rust framework.
**Researched:** 2026-05-27
**Confidence:** HIGH (architecture and decomposition derived from existing in-repo research: `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md`, `research/call-graphs/FINAL-REPORT.md`, `research/type-alias-points-to/FINAL-REPORT.md`, `research/agent-extension-surface/FINAL-REPORT.md`, `research/data-flow/FINAL-REPORT.md`, `research/effects-summaries/FINAL-REPORT.md`, and the v1.2 phase ledger in `.planning/PROJECT.md`).

> v1.3 is a *subsequent* milestone. The v1.2 substrate is already in place: kernel facade, MIR, CFG, direct/refined calls, abstract domains, summaries, type/value/place/alias, data flow, slicing/evidence, benchmark adapters (Go x/tools RTA + Jelly), and bounded public SDK query views. v1.3 turns those isolated providers into a single semantic graph with a unified solver, and adds the language-specific frontends (Go `go/packages`/SSA-equivalent; JS/TS scope + token + property model) that benchmark recall actually requires.

---

## Existing v1.2 Facts Consumed By v1.3 (Not Re-Researched)

Every v1.3 feature is a *consumer* or *upgrader* of the following private fact families that already exist (do not re-implement):

| v1.2 substrate | v1.3 consumer |
|----------------|---------------|
| Kernel facade, provider manifests (SAE-FND-01) | Solver core scheduler; new provider manifests for RTA/VTA/token/property |
| Provenance / precision / validation metadata (SAE-FND-02) | Every new edge / token / object carries provenance, precision tier, confidence |
| Input snapshot + cache identity (SAE-FND-04) | New digest inputs: module roots, build tags, `tsconfig`, model files, solver budgets |
| Persistent layer cache (SAE-FND-05) | Per-family caches: package load, types, SSA, scope, token sets, property maps, points-to, solved graph |
| Semantic index deepening (SAE-SEM-01) | Re-used for binding lookup; v1.3 adds JS scope hardening on top |
| Module / package / topology graph (SAE-SEM-02) | Consumed by Go package loader and JS/TS module resolver |
| Semantic MIR + `PlaceId` (SAE-SEM-03) | Solver constraints are emitted in MIR; token/points-to facts use `PlaceId` |
| Local CFG + control dependence (SAE-SEM-04) | Reachability traversal; loop/branch budgets for solver |
| Direct call facts (SAE-SEM-05) | Static-edge seed for unified solver; reachability frontier |
| Abstract-domain kernel (SAE-INT-01) | Token-set and property-set lattices reuse worklist + transfer scaffolding |
| Summary kernel + direct summaries (SAE-INT-02) | RTA reads summaries; token solver writes/reads param→return summaries |
| Demand queries + SCC cache (SAE-INT-03) | Solver fixed-point organised by SCC; query-scoped invocation for rules |
| Rust extension / provider sink (SAE-INT-04) | Adaptation models flow through existing extension sinks; no new public surface |
| Framework entrypoints + trust boundaries (SAE-INT-05) | Root-set for reachability; JS/TS framework defaults seed token sources |
| Type / value / place / alias substrate (SAE-PREC-01) | Go receiver-type filter for RTA; JS allocation tokens reuse `ValueFact` shapes |
| Refined call graph providers (SAE-PREC-02) | Replaced internally by unified solver; refined-provider seam stays as model overlay |
| Data-flow facts (SAE-PREC-03) | Solver-derived call edges sharpen interprocedural data flow downstream |
| Slicing / paths / evidence (SAE-PREC-04) | Evidence renderer consumes new provenance fields (algorithm, validation, model_id) |
| Benchmark adapters + promotion gates (SAE-PROM-01) | Adapted-mode reporting (prompt hash, accepted/rejected model facts, deltas) |
| Public SDK query views (SAE-PROM-02) | Only the bounded query builders that already passed promotion gates; v1.3 does **not** promote new public views |

---

## Feature Landscape

### Table Stakes (Required for benchmark recall and trustworthy reporting)

These features are non-negotiable: without them, either the benchmark scores stay near baseline (Go RTA 2.7% / Jelly 0.63%) or the scores are statistically untrustworthy because identity bugs hide real recall/precision behaviour. They map 1:1 onto the v1.3 milestone target features.

| # | Feature | Why Expected | Complexity | Dependencies (v1.2 + intra-v1.3) | Notes |
|---|---------|--------------|------------|----------------------------------|-------|
| 1 | **Stable internal identity for functions, callsites, scopes, packages** | Soufflé, CodeQL, Wala, Jelly, x/tools all keep canonical IDs separate from display names. Without this, every other recall improvement gets hidden by string mismatches. | LOW | v1.2 semantic MIR + module graph + symbol graph | Identity record: `(file, span, language, package/module, lexical container, display name, signature digest)`. Owns the seam between internal IDs and benchmark renderers. |
| 2 | **Benchmark identity renderers (Go `RelString`, Jelly `file:start_line:start_col:end_line:end_col`)** | Both oracles compare strings. x/tools uses `Func.RelString(from)` for cross-package edges; Jelly uses 1-based line/col spans with inclusive end. Identity bugs alone explain a large fraction of current FN. | LOW | Feature 1 | Dedicated render layer keeps the engine free of benchmark-specific shape. |
| 3 | **Observed-edge deduplication by semantic identity (before scoring)** | x/tools RTA and Jelly both emit unique edges; duplicates inflate FP. Standard practice in JCG, OPAL, Unimocg call-graph benchmarks. | LOW | Feature 1 | Dedup key = `(caller_id, site_id, callee_id, edge_kind)` after canonicalisation. |
| 4 | **Identity-vs-unsupported taxonomy in reports** | CodeQL, Joern, Pysa, Semgrep all separate "we don't model this" from "we mislabelled this". Without it, debugging recall regressions is guesswork. | LOW | v1.2 SAE-FND-02 (validation/provenance) | Categories: `wrong_identity`, `unsupported_edge`, `unresolved_edge`, `package_load_limitation`, `model_missing`. Lives next to evidence. |
| 5 | **Explicit reachability roots: `main`, `init`, exported entrypoints, tests, configured roots** | x/tools RTA is a *reachable* call graph — dead-code edges should not be scored. Jelly micro suite includes file-scope module execution; without the right root set both oracles report bogus FPs. | LOW–MEDIUM | v1.2 SAE-INT-05 (framework entrypoints) | Reuse trust-boundary facts; add per-mode root policy: `oracle-rta`, `oracle-jelly`, `whole-repo`. |
| 6 | **Reachable-graph scoring mode (filter unreachable direct calls from scored set, keep as facts)** | Same as #5; without per-mode scoring you cannot honestly compare to x/tools. Goes hand-in-hand with mode-aware oracle alignment. | LOW | Feature 5, v1.2 CFG | Score modes select which fact subsets are scored; unreachable facts remain queryable. |
| 7 | **Go semantic frontend: `go/packages` loading + module-root inference** | Every serious Go analyser (Soufflé/Glow, Wala-Go, x/tools, GoCG, golangci-lint's typed checks) uses `go/packages`. Tree-sitter alone cannot resolve identifiers across imports, build tags, generics, or generated code. | HIGH | v1.2 SAE-SEM-02 (module/package topology) | Sidecar-friendly boundary; cache by module root + build tags + Go toolchain + lockfile + include-tests + file digests. Honest fall-through to "setup_missing" when load fails. |
| 8 | **Go type checking + receiver types + method sets + init functions + generics** | RTA and VTA require type and method-set facts. Init-order edges and generic instantiations are part of x/tools' expected graph. | HIGH | Feature 7 | Use `go/types` directly; normalise into polint type facts so rules never see raw `types.Type`. |
| 9 | **Go RTA provider: reachable functions + address-taken + dynamic dispatch by signature + interface-invoke by method-set + fixed point** | This is the actual algorithm x/tools' oracle compares against. Without it, Go recall stays at the static-edge ceiling. | HIGH | Features 7, 8, unified solver | Index by signature and method set. Iterate to fixed point; new address-taken or new concrete type can introduce new edges. Reflection edges are **unsupported**, not invented. |
| 10 | **JS/TS function and callsite inventory: declarations, expressions, arrows, methods, constructors, accessors, class static blocks, calls, `new`, tagged templates, optional calls, dynamic `import()`** | Jelly's oracle expects every callsite by exact span. Missing inventory = silent FN. | MEDIUM | v1.2 Oxc adapter | All forms enumerated by AST kind; arrows and methods need stable lexical-parent identity; class static blocks are easy to miss. |
| 11 | **Exact JS/TS spans matching Jelly (1-based line/col, inclusive end)** | Jelly compares `file:start_line:start_col:end_line:end_col`. Oxc gives byte offsets; converting needs Jelly-compatible line counting (LF and CRLF normalisation, BOM handling). | LOW–MEDIUM | Feature 10 | Verify against the Jelly micro fixtures; add explicit conformance tests. |
| 12 | **JS/TS lexical scopes (var/let/const/functions/classes/imports/destructuring/params/catch/re-exports)** | Standard in TypeScript compiler, Oxc, Rome/Biome, Closure, Wala-JS. Required for any non-trivial binding lookup. | MEDIUM | v1.2 SAE-SEM-01 | Build on Oxc semantic; do not re-implement, but normalise into polint scope facts (similar to Pyrefly module-binding graphs). |
| 13 | **JS/TS module graph: ESM import/export, CommonJS `require`/`module.exports`, package entrypoints, `tsconfig` path aliases** | Without this, every cross-file edge in Jelly is unresolved. ESM/CJS interop is the default real-world case. | MEDIUM | v1.2 SAE-SEM-02 (topology), feature 12 | Use `oxc_resolver` for resolution; cache by lockfile + `tsconfig` + package.json `exports`. |
| 14 | **JS/TS direct binding for `f()`, `ns.f()`, imported functions, local aliases** | Lowest-precision still-useful Jelly edges. Direct binding is what every JS analyser ships first. | MEDIUM | Features 10, 12, 13 | Distinguish "bound-direct" from "syntactic"; carry `algorithm = binding` provenance. |
| 15 | **JS/TS function-token propagation (assignments, aliases, params, returns, closures, simple HOFs)** | Jelly *is* a function-token analyser. Wala-JS, Closure, ACG, CodeQL JS all use token/value-tracking. This is the main recall lever. | HIGH | Features 12–14, unified solver, v1.2 abstract-domain kernel + summaries | Allocation-site abstraction; tokens flow through MIR. Worklist over flow edges; budget = per-place token-set cap + per-callsite fanout cap. |
| 16 | **JS/TS object/property/prototype/class/`this` model** | Real Jelly cases call through `obj.method()`, `super.method()`, prototype chains, bound functions, and class methods. Without an object model, token propagation cannot resolve them. | HIGH | Features 15, 12 | Allocation-site objects; exact string property names where statically known; one bucket per object for computed/unknown names; `this` binding for methods/constructors/arrows/bound/`call`/`apply`. |
| 17 | **Shared semantic graph + unified call solver (single constraint-and-solve over MIR, values, points-to, calls, summaries, entrypoints)** | The architectural keystone from `GRAPH-ENGINE-BENCHMARK-RESEARCH.md`. Soufflé/Doop, Wala, OPAL, Unimocg, SVF all converge on a single constraint store with multiple frontends. | HIGH | All v1.2 substrate; replaces internal use of refined-call providers | Constraint store keyed by `PlaceId`/`CallSiteId`/`FunctionId`. Frontends contribute *constraints*, not edges. Solver returns edges with algorithm + confidence + provenance. |
| 18 | **Per-family caches + solver budgets (token-set size, property abstraction width, dynamic fanout, model expansion, package depth)** | Wala, Doop, SVF, CodeQL all expose budgets; without them the higher-precision solvers blow up on real repos. | MEDIUM | v1.2 SAE-FND-04 / SAE-FND-05; features 7–17 | Digest inputs: language config, module roots, build tags, `tsconfig`, lockfiles, model files, budget values. Cold-vs-warm timings reported separately. |
| 19 | **Unsupported / unknown taxonomy: `setup_missing`, `unsupported_semantic_domain`, `unresolved_by_missing_facts`, `out_of_scope_for_mode`** | Same as feature 4 but applied across every solver phase. Pysa, CodeQL, Joern, Wala all report unknown categories explicitly. Required for trustworthy diagnostic queues. | LOW–MEDIUM | v1.2 SAE-FND-02; pervasive | The first thing an AI agent reading the engine output sees; must be reliable. |

### Differentiators (Raise precision, UX, or agent leverage above standard practice)

These features are not required to clear the v1.3 recall targets, but they materially raise either precision, agent-extension leverage, or rule-author and benchmark-adapter experience. They map onto the "Recommended Polint Direction" sections of the call-graph and agent-extension research.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Adaptation model layer (schema for source→target with validation, accept/reject reporting, prompt hash + delta)** | Polint's product wedge per `agent-extension-surface/FINAL-REPORT.md`: AI agents add validated, repo-local models — not config strings — and the engine surfaces accepted-vs-rejected with precision/recall deltas. CodeQL Models-as-Data and Pysa model generators ship this; nobody else turns it into a measured product metric. | MEDIUM | Strict schema: `source_pattern`, `target_pattern`, `confidence`, `language`, `scope`, `evidence`. Validator must verify target symbols exist (or accept declared external boundary). Adapted benchmark mode records prompt hash, changed model files, accepted/rejected facts, unknown delta, P/R delta, runtime delta. |
| **Per-edge algorithm provenance ladder (`syntactic` → `bound` → `direct` → `cha` → `rta` → `vta` → `token` → `points_to` → `repo_model`)** | Rules and agents can pick a precision tier per query. x/tools and Unimocg both keep algorithm labels separate; Jelly does not. This is a competitive precision tool. | LOW | Already part of v1.2 metadata; v1.3 must extend the enum and surface it in evidence. |
| **Per-mode oracle alignment switch (`oracle-rta`, `oracle-jelly`, `whole-repo`)** | Benchmark adapters and rules want different scoring views without different engines. Mirrors x/tools' `-format` flag philosophy. | LOW | Hangs off the reachability root policy. |
| **JS/TS allocation-site abstraction with bounded property widening** | The same JS/TS recall, but at controlled precision cost. Without widening, real repos blow the token-set budget; with it, property-flow stays useful. Wala-JS, Closure, and ACG all widen; Jelly widens conservatively. | MEDIUM | Computed property buckets per object; configurable widen threshold. |
| **Go VTA experimental provider over the same solver** | Recovers a small set of edges RTA cannot, useful for high-precision rules. Optional — RTA already targets >70% recall. | MEDIUM | Shares constraint store with RTA; gated by budget. |
| **Cold-vs-warm benchmark runtime reporting + cache hit/miss per fact family** | Required by the original research: "make semantic Go and RTA practical enough to run repeatedly." Almost no analyser surfaces this cleanly. | LOW | Already partially present from SAE-FND-04/05; v1.3 adds new families. |
| **Confidence labels per edge (`exact`, `high`, `medium`, `low`, `unknown`)** | Rule authors can write "find me only `exact + high` callees"; agents can prioritise low-confidence unresolved facts for adaptation. | LOW | Already in v1.2 metadata; v1.3 binds it to algorithm + budget state. |
| **Demand-driven (query-scoped) graph slices** | The base solver runs fixed-point, but rules and agents often want a single seed → reachable cone. Mirrors CodeQL's `getACallee()`/`getEnclosingCallable()` and Pysa's per-issue resolution. | MEDIUM | Already in v1.2 SAE-INT-03 (demand queries); v1.3 wires the unified solver into the same surface. |
| **Identity-stable diagnostic evidence (path + edge + algorithm + confidence + model_id)** | Agent-consumable evidence chains. SAE-PREC-04 shipped the rendering; v1.3 enriches the metadata. | LOW | Pure metadata threading. |
| **JS native-callable shims for built-ins consumed by token propagation (`Array.prototype.map`, `forEach`, `Promise.then`, ...)** | Without a few native shims, the most common HOF patterns produce zero edges. Wala-JS, ACG, Closure all ship native models. Jelly itself includes them. | MEDIUM | Small built-in model set; shipped behind the same adaptation schema as repo-local models, so users can override. |
| **Reproducibility: deterministic ordering of constraint resolution, snapshot-friendly debug exports** | Already part of polint v1.2 ethos (deterministic-by-design). The solver must keep it. Tools like Soufflé and Doop emit deterministic relations; Wala does not by default. | MEDIUM | Solver iterates SCCs in topological order; ties broken by stable ID. |

### Anti-Features (Explicitly Rejected — recorded so the milestone resists them)

These features show up under recall pressure but damage either precision or product credibility. Rejected explicitly in `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` (Adaptation Boundary, Architectural Recommendation), `call-graphs/FINAL-REPORT.md` (Total Recall caveats), `type-alias-points-to/FINAL-REPORT.md` (alias-as-query layer), and `agent-extension-surface/FINAL-REPORT.md` (config-only extension).

| Anti-Feature | Why Requested | Why Problematic | Alternative |
|--------------|---------------|-----------------|-------------|
| **Recall-by-flooding (emit every plausible edge and rely on user filters)** | Easy way to push recall numbers up on a benchmark page. | Destroys precision; makes the call graph useless for repo-local policy enforcement. Total Recall (Java CG benchmark research) explicitly warns that graph size is not a recall proxy. Rules cannot tell a flood edge from a real edge. | Honest fixed-point solver with explicit `unresolved` / `unsupported` facts and per-algorithm precision labels. |
| **Benchmark-label adaptation (agents author models that mirror expected oracle edges)** | Looks like it raises adapted-mode recall fast. | Defeats the purpose of measuring adaptation; the model layer must reflect *repo facts*, not oracle answers. Explicitly called out in `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` §9 "Bad adaptation examples". | Adaptation agent inspects unresolved facts and code only. Validator rejects models that bypass identity / reachability / solver. Adapted-mode reports accepted-vs-rejected counts. |
| **Wildcard / broad-pattern models that match too many call sites** | Adapter wants to wipe out unresolved-call queues quickly. | Same flooding problem; precision collapses while unknowns fall. Pysa, CodeQL, Joern all reject this and require qualified patterns. | Model schema requires concrete `source_pattern` and `target_pattern`. Confidence ceiling for fuzzy patterns. Validation status + accepted/rejected reporting. |
| **Single monolithic "the call graph" (no algorithm label, no confidence, no unresolved)** | Simpler downstream API. | Java CG research (Unimocg, Total Recall, 2026 unsoundness paper) shows that without algorithm/confidence/unresolved labels, real-tool comparisons drift up to 30–60% even between "equivalent" algorithms. Rules cannot pick a precision budget. | Layered call fact family with `algorithm`, `confidence`, `status`, `reason`, `provider`, `provenance`, `model_id`, `validation`. |
| **Whole-program closed-world points-to as the default** | Maximum precision claims. | Andersen O(n³) worst case; Doop variants timeout at 600s in real benchmarks; SVF burns memory. Not viable on the medium repos polint targets. | Andersen-style points-to as an *opt-in*, budgeted provider with SCC/delta propagation, field sensitivity flags, and explicit `Unknown` on budget exhaustion. |
| **Auto-modeled reflection / dynamic-import edges** | Recall in dynamic patterns. | Reflection edges are intrinsically unsoundness-prone (Java CG unsoundness paper §3); auto-modelling creates phantom edges. x/tools issue #61160 ("model reflective calls soundly") is still open. | Report reflection / dynamic-import as `unsupported_semantic_domain`. Provide adaptation hook so repo agents can model their specific reflection patterns. |
| **Public broad SDK surface for the new graph engine in v1.3** | Faster external adoption. | v1.2 promotion gate philosophy: public surfaces only after fixtures + cache tests + benchmark gates prove them. Premature promotion locks API shape before precision/confidence semantics settle. | Keep new providers crate-private. Promote bounded query views only after promotion gates pass (mirror SAE-PROM-02). |
| **One mode of scoring used across both Go and Jelly suites** | One report shape is simpler. | x/tools RTA expects reachable-graph scoring; Jelly expects all-callsite scoring including unreachable file-scope. A single mode gives bogus numbers for one of them. | Per-mode oracle alignment (`oracle-rta`, `oracle-jelly`, `whole-repo`). |
| **Dynamic plugin loading for adaptation models (e.g. `dlopen`)** | Power-user appeal. | Same reasoning as v1.0 — process isolation beats in-process plugins for stability. Dylint precedent: in-process plugins can crash the driver. | Models loaded as data (validated schema), or repo-local Rust extension code compiled and run through the existing `SAE-INT-04` extension sink. |
| **Tree-sitter-only Go pipeline (no `go/packages`)** | Avoids the Go toolchain dependency. | Cannot resolve cross-package identifiers, interfaces, generics, or init order. Recall ceiling is the static-edge subset (≈3% on x/tools RTA). | `go/packages` + `go/types` as the language oracle, normalised into polint facts; honest `setup_missing` when toolchain is absent. |
| **Span identity inferred from display name / fully qualified path** | Avoid plumbing Oxc/Go spans through. | Display names collide (overloaded methods, anonymous functions, generic instantiations); Jelly's oracle compares exact spans. Recall regressions stay invisible. | Identity record must carry `(file, span, language, package/module, lexical container, display name, signature digest)`. Renderers project to oracle shapes from identity. |
| **Hiding unresolved / unsupported calls from output ("clean graph") by default** | Looks tidier in rule output. | CodeQL JS, Pysa, Joern, Wala all expose unresolved facts because they are required to estimate FN risk. Hiding them turns the unknown queue invisible to adaptation agents. | First-class unresolved facts with reasons; rules opt in/out via tier filter. |

---

## Feature Dependencies

```
[ 1 Stable identity ]
   ├─required-by─> [ 2 Renderers ] ──required-by─> [ 3 Dedup ] ──required-by─> [ 4 Identity taxonomy ]
   ├─required-by─> [ 5 Roots ] ──required-by─> [ 6 Reachable scoring ]
   ├─required-by─> [10 JS inventory] ──required-by─> [11 Exact JS spans]
   └─required-by─> [17 Unified solver]

[ 7 Go packages load ]
   └─required-by─> [ 8 Go types + method sets ]
                       └─required-by─> [ 9 Go RTA provider ]
                                           └─requires─> [17 Unified solver]

[12 JS scopes] ─required-by─> [13 JS module graph] ─required-by─> [14 JS direct binding]
                                                                       └─required-by─> [15 Token propagation]
                                                                                         └─required-by─> [16 Object/property/this]

[17 Unified solver] ─consumed-by─> [ 9 Go RTA ], [15 Tokens], [16 Object/property], [Adaptation models]

[18 Caches + budgets] ──enhances──> [ 7, 9, 15, 16, 17 ]   (without budgets, 9/15/16 are not viable on real repos)

[19 Unknown taxonomy] ──pervasive──> [ all ]               (every provider contributes to it)

[Adaptation models] ──consumed-by──> [17 Unified solver]   (only after the solver is capable; premature adaptation produces misleading deltas)

[Recall-by-flooding] ──conflicts-with──> [Precision-as-first-class target, 4, 19]
[Benchmark-label adaptation] ──conflicts-with──> [Adaptation model layer]
```

### Dependency Notes

- **#1 (identity) is the keystone.** Five other features become noise without it; this is why GRAPH-ENGINE-BENCHMARK-RESEARCH.md recommends doing identity + renderers first.
- **#7 → #8 → #9 forms the Go critical path.** None of them produce benchmark recall in isolation. They must ship together (or with #9 stubbed against `setup_missing`).
- **#12 → #13 → #14 → #15 → #16 is the JS/TS critical path.** Each layer roughly doubles recoverable Jelly recall; reversing any order produces partial results that look like regressions in the previous step.
- **#17 (unified solver) is the architectural keystone.** Without it, RTA, token propagation, and object/property modelling become parallel engines duplicating work and disagreeing on edge confidence. With it, future precision layers (VTA, points-to, k-CFA) plug in as constraint contributors.
- **#18 (cache + budgets) must land before #15 / #16 in production use.** Token-set and property-set propagation are the budget-sensitive workloads. v1.2's cache substrate already exists; v1.3 only adds new digest inputs and per-family caches.
- **Adaptation models depend on #17 being already capable** (the engine must be able to consume model facts and surface accepted/rejected). Premature adaptation produces flooding incentives.
- **Anti-feature "recall-by-flooding" structurally conflicts with #4 and #19.** Honest identity and unknown taxonomy make flooding visible. The mitigation is design-time, not policy.
- **Anti-feature "benchmark-label adaptation" structurally conflicts with the Adaptation model layer.** The validator must verify that model targets exist as repo symbols, not as oracle expected edges.

---

## MVP Definition

Polint v1.3 has only one customer-visible milestone exit criterion (Go RTA ≥25-30% recall, Jelly ≥25-30% recall, both with precision held high). The MVP is the minimum feature set that hits that bar honestly.

### Launch With (v1.3 MVP — required to clear benchmark gates)

- [ ] **#1 Stable identity** — keystone; nothing else works without it.
- [ ] **#2 Benchmark renderers (Go `RelString`, Jelly spans)** — recall is invisible without them.
- [ ] **#3 Observed-edge dedup** — precision is invisible without it.
- [ ] **#4 Identity-vs-unsupported taxonomy** — required for trustworthy reporting.
- [ ] **#5 Explicit roots + #6 reachable scoring** — Go precision floor; required for x/tools mode.
- [ ] **#7 Go `go/packages` loader + module-root inference** — Go critical path.
- [ ] **#8 Go types + receiver types + method sets + init + generics** — Go critical path.
- [ ] **#9 Go RTA provider with fixed point** — primary Go recall lever.
- [ ] **#10 JS/TS function/callsite inventory** — Jelly critical path; required before any JS recall is countable.
- [ ] **#11 Exact JS/TS spans matching Jelly** — Jelly critical path.
- [ ] **#12 JS/TS scopes + bindings** — JS critical path.
- [ ] **#13 JS/TS module graph (ESM + CJS + path aliases)** — JS critical path.
- [ ] **#14 JS/TS direct binding for `f()`, `ns.f()`, imports, aliases** — Jelly's first measurable recall layer.
- [ ] **#15 JS/TS function-token propagation (assignments / aliases / params / returns / closures)** — main Jelly recall lever.
- [ ] **#16 JS/TS object/property/prototype/class/`this` model** — pushes Jelly recall past the token-only ceiling.
- [ ] **#17 Shared semantic graph + unified call solver** — architectural keystone; allows #9, #15, #16 to share constraints.
- [ ] **#18 Per-family caches + solver budgets** — required for #9, #15, #16 to run on real repos.
- [ ] **#19 Unsupported / unknown taxonomy across providers** — required for honest scoring and for the adaptation queue.

### Add After Validation (v1.3.x — once gates pass)

- [ ] **Adaptation model layer (schema + validation + accept/reject reporting + adapted-mode benchmark)** — depends on the solver being capable enough that adapted mode produces meaningful (non-flooding) deltas. Premature shipping rewards bad adaptation.
- [ ] **Native callable shims for common JS built-ins** (`Array.prototype.map`, `forEach`, `Promise.then`, ...) — measurable once token propagation is stable.
- [ ] **Confidence label tightening + algorithm provenance ladder surfacing** — once base recall lands, expose tiers to rule authors.
- [ ] **Demand-driven (query-scoped) graph slices wired to the unified solver** — wraps existing SAE-INT-03 onto the new solver outputs.

### Future Consideration (v1.4+ — outside v1.3 scope)

- [ ] **Go VTA provider** — small additional recall over RTA; not required for the gate.
- [ ] **Bounded Andersen-style points-to provider for Go and JS** — opt-in, budgeted; only useful for specific rule families.
- [ ] **Context sensitivity (k-CFA / object-sensitive)** — explicit future precision tier; the v1.2 substrate already keeps IDs that allow it.
- [ ] **Public SDK surface for the new graph engine** — only after promotion gates pass (mirrors SAE-PROM-02 sequencing).
- [ ] **Python and Java parity** — explicitly out of scope per `PROJECT.md`; Go + TS/JS must prove the model first.
- [ ] **Reflection / dynamic-import auto-modelling** — rejected as default; future research only if model-based shims prove robust.

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|-----------|---------------------|----------|
| #1 Stable identity | HIGH | LOW | **P1** |
| #2 Benchmark renderers | HIGH | LOW | **P1** |
| #3 Observed-edge dedup | HIGH | LOW | **P1** |
| #4 Identity-vs-unsupported taxonomy | HIGH | LOW | **P1** |
| #5 Reachability roots | HIGH | LOW–MEDIUM | **P1** |
| #6 Reachable scoring | HIGH | LOW | **P1** |
| #7 `go/packages` loader | HIGH | HIGH | **P1** |
| #8 Go types + method sets | HIGH | HIGH | **P1** |
| #9 Go RTA provider | HIGH | HIGH | **P1** |
| #10 JS function/callsite inventory | HIGH | MEDIUM | **P1** |
| #11 Exact JS spans | HIGH | LOW–MEDIUM | **P1** |
| #12 JS scopes | HIGH | MEDIUM | **P1** |
| #13 JS module graph | HIGH | MEDIUM | **P1** |
| #14 JS direct binding | HIGH | MEDIUM | **P1** |
| #15 JS token propagation | HIGH | HIGH | **P1** |
| #16 JS object/property/`this` | HIGH | HIGH | **P1** |
| #17 Unified solver | HIGH | HIGH | **P1** |
| #18 Caches + budgets | HIGH | MEDIUM | **P1** |
| #19 Unknown taxonomy | HIGH | LOW–MEDIUM | **P1** |
| Adaptation model layer | HIGH | MEDIUM | P2 |
| Native callable shims | MEDIUM | MEDIUM | P2 |
| Algorithm provenance ladder | MEDIUM | LOW | P2 |
| Demand-scoped graph slices | MEDIUM | MEDIUM | P2 |
| Go VTA provider | MEDIUM | MEDIUM | P3 |
| Bounded Andersen points-to | LOW–MEDIUM | HIGH | P3 |
| Context sensitivity | MEDIUM | HIGH | P3 |

**Priority key:**
- **P1**: Must have for v1.3 (benchmark gate cannot pass without it; honest reporting requires it).
- **P2**: Should have once P1 lands and benchmark gates pass.
- **P3**: Defer to v1.4+; valuable but outside the v1.3 precision-and-recall envelope.

---

## Competitor / Reference Feature Analysis

How production static-analysis frameworks treat each v1.3 feature, and the polint stance.

| Capability | Soufflé / Doop | CodeQL | Wala (Java/JS) | Jelly (JS/TS) | golang.org/x/tools (Go) | polint v1.3 stance |
|-----------|----------------|--------|---------------|---------------|--------------------------|--------------------|
| **Stable identity** | Datalog relation keys, content-stable | Database-extracted identities, span-preserving | `IClass` / `IMethod` IDs, signature-keyed | File + span tuple is the identity | `*ssa.Function` + `Func.RelString()` | Identity record with `(file, span, language, package, container, display, signature digest)`; renderers project to Go `RelString` and Jelly span shapes. |
| **Benchmark-shape renderers** | Custom per-benchmark | QL projections | Java-style FQN; JS uses spans | Owns the Jelly format | `RelString` built in | Polint owns a renderer layer separate from the engine. |
| **Reachability roots** | Configured root relations | `Callable`s with explicit entrypoint classes | Explicit entrypoint list | File-scope module execution treated as root | `mains`, `inits`, `tests` per package + RTA seed | Roots are first-class facts; per-mode policy. |
| **Reachable scoring** | Mode-dependent | Mode-dependent | Mode-dependent | All callsites scored | Reachable-graph by design | Per-mode `oracle-rta` / `oracle-jelly` / `whole-repo`. |
| **Type / package frontend** | Doop ingests Soot facts | Custom extractor over javac | Heavy semantic frontend | Oxc + own scope/type tracking | `go/packages` + `go/types` | `go/packages` + `go/types` as the Go oracle; Oxc + own scope/binding for JS/TS. |
| **RTA equivalent** | Yes (Datalog) | Yes (`getViableCallable`) | Yes | N/A | Yes — canonical | Yes, written against the unified solver. |
| **VTA equivalent** | Yes | Yes | Yes | N/A | Yes (experimental) | Optional v1.3 differentiator; v1.4 default-on candidate. |
| **JS/TS function-token propagation** | N/A | Yes (type tracking) | Yes (`PointerAnalysis`-based) | Yes — canonical | N/A | Yes; main Jelly recall lever. |
| **JS/TS object/property/`this`** | N/A | Yes (heavy ML-assisted in some queries) | Yes (`AstJavaScriptLoader` + prototype model) | Yes — canonical | N/A | Yes; allocation-site + bounded computed buckets. |
| **Reflection / dynamic** | Modelled / sometimes unsound | Modelled per-language | Modelled with caveats | Conservative; some patterns missed | Open issue #61160 | **Explicit `unsupported_semantic_domain`** in v1.3; adaptation hook for repo-specific patterns. |
| **Unresolved / unknown facts** | Datalog `unknown_*` relations | Predicates expose `getEnclosingCallable` + nulls | Diagnostics + warnings | Reports "unresolved-call" facts | RTA's `unknown` set; surfaced through API | First-class facts with taxonomy: `setup_missing`, `unsupported_semantic_domain`, `unresolved_by_missing_facts`, `out_of_scope_for_mode`. |
| **Repo-local model extension** | Datalog edits | Models-as-Data | Static configuration | None | None | Validated Rust extension sinks + data-only adaptation models with accept/reject reporting + adapted-mode benchmark. **Differentiator**. |
| **Per-algorithm provenance** | Implicit (separate Datalog passes) | Implicit (separate queries) | Implicit | Single algorithm | Algorithm flag per package | Explicit `algorithm` field on every edge: `syntactic` / `bound` / `direct` / `cha` / `rta` / `vta` / `token` / `points_to` / `repo_model`. **Differentiator**. |
| **Cache / incrementality** | Mostly batch | Database snapshots | Mostly batch | Batch | Per-call | Per-fact-family layered cache with digest inputs (lockfile, tsconfig, build tags, model files, budgets). |
| **Solver budgets** | Datalog rule limits | Default + per-query | Heap caps | Configurable | RTA bounded by reachable set | Per-token-set / per-property / per-fanout / per-package-depth / per-model-expansion. |
| **Public SDK surface** | Datalog | QL | Java API | JS/TS API | Go API | Crate-private in v1.3; promote bounded views only after promotion gates pass (mirror SAE-PROM-02). |

### Polint's combined positioning

The competitor table shows polint v1.3 is not trying to *out-recall* any single tool on its home benchmark. It is trying to:

1. **Match the algorithm ladder** that Unimocg / x/tools / Wala-JS converged on (CHA / RTA / VTA / type-tracking / token / points-to, all separately labelled).
2. **Beat the standard "configuration-only" extension surface** with Rust extensions + validated data-models + accept/reject deltas — a stronger version of CodeQL Models-as-Data targeted at AI agents.
3. **Keep the public API smaller and more honest** than Wala or Soot — promote only what is proven, surface unresolved facts as first-class.
4. **Make benchmarks honest by default** — exact identity, dedup, per-mode scoring, cold-vs-warm runtime, accepted-vs-rejected model facts.

---

## Expected User-Visible Behavior

### For repo-local rule authors (`#[polint::rule]` consumers)

- Rules continue to consume the **existing** v1.2 public surface; v1.3 does **not** ship new public APIs.
- Edge precision becomes consistently better when rules run in cached mode (warm cache); first-run cold mode advertises the runtime delta.
- Diagnostic evidence carries algorithm + confidence + model_id when applicable (rules need not consume them, but they appear in JSON / SARIF).
- Rules that depend on call targets see fewer false unresolved facts in Go (because `go/packages` loads) and fewer false unresolved facts in JS/TS (because scope + module graph + token propagation resolve more).

### For benchmark adapters (Go x/tools RTA, Jelly micro)

- New per-mode root policy: an adapter can ask for `oracle-rta` or `oracle-jelly` scoring without engine forks.
- Observed edges arrive deduplicated by semantic identity.
- Unknown-category reports tell adapter operators *why* an edge is missing: `setup_missing`, `unsupported_semantic_domain`, `unresolved_by_missing_facts`, `out_of_scope_for_mode`. This is the operator's adaptation queue.
- Adapted-mode reports include prompt hash, changed model files, accepted-vs-rejected model facts, unknown delta, P/R delta, runtime + cache delta.

### For AI agents consuming the SDK (the strategic customer)

- Unresolved-fact queue is high-quality and categorised; the agent can prioritise model authoring against the highest-impact unresolved buckets.
- Adaptation models are written as data (validated schema) or as Rust extension code through the existing extension sinks (SAE-INT-04). Validator rejects models that bypass identity / reachability / solver — explicit feedback.
- Engine reports accepted-vs-rejected model facts and the precision/recall delta they produced; the agent can iterate by measurement rather than by hope.
- Engine does **not** auto-model frameworks, reflection, or dynamic imports. The agent is the right entity to add repo-specific knowledge.

---

## Sources

In-repo research (read directly):

- `.planning/PROJECT.md` — milestone context and v1.2 phase ledger.
- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — primary specification of v1.3 feature scope, expected metric deltas, complexity bounds, and rejected anti-features.
- `research/call-graphs/FINAL-REPORT.md` and `research/call-graphs/STANDARD.md` — algorithm ladder, provenance model, JS/Python/Java/Go call-graph research lessons.
- `research/type-alias-points-to/FINAL-REPORT.md` — type/value/places/alias substrate and Andersen-as-bounded-provider stance.
- `research/agent-extension-surface/FINAL-REPORT.md` — extension architecture, why Rust-code extensions beat config-only, and validation invariants.
- `research/data-flow/FINAL-REPORT.md` — interaction between call graph and data flow, summary kernel reuse.
- `research/effects-summaries/FINAL-REPORT.md` — summary kernel design that v1.3 RTA and token solver consume.

External (verified via web search; LOW–MEDIUM confidence unless otherwise noted):

- [golang.org/x/tools `rta` package documentation](https://pkg.go.dev/golang.org/x/tools/go/callgraph/rta) — RTA algorithm shape; address-taken × dynamic-call cross-product, fixed-point iteration (HIGH confidence — official documentation).
- [golang/tools `cmd/callgraph/main.go`](https://github.com/golang/tools/blob/master/cmd/callgraph/main.go) — `-format` flag and per-algorithm selection; Go-side reference for v1.3 oracle alignment (HIGH confidence — official source).
- [golang/go issue #61160 — model reflective calls soundly](https://github.com/golang/go/issues/61160) — open issue confirming reflection edges remain hard; supports the "explicit unsupported, not invented" stance (HIGH confidence — upstream issue).
- [cs-au-dk/jelly](https://github.com/cs-au-dk/jelly) — JS/TS function-token analyser used as the v1.3 Jelly oracle; `file:start_line:start_col:end_line:end_col` span format and call-graph JSON output. (MEDIUM — confirmed JSON output but exact schema not transcribed; verify against fixtures in repo.)
- [opalj/JCG](https://github.com/opalj/JCG) — JVM call-graph benchmark methodology that informs polint's per-mode scoring stance (LOW — referenced for shape, not directly consumed).

---

*Feature research for: v1.3 Graph Engine Precision*
*Researched: 2026-05-27*
