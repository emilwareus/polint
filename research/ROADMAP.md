# Static Analysis Research Roadmap

Date: 2026-05-15

## Research TODO

- [x] Semantic index baseline implemented: `Symbols<'_>` and `References<'_>` are available through the SDK.
- [x] Call graph research completed: see `research/call-graphs/`.
- [x] Data-flow research completed: see `research/data-flow/`.
- [x] Agent extension surface research completed: see `research/agent-extension-surface/`.
- [x] Analysis kernel research completed: fact layers, scheduling, provenance, precision, validation, extension merges, cache keys, and invalidation. See `research/analysis-kernel/`.
- [x] Evaluation harness research completed: external-benchmark-first strategy, default-vs-agent-extended metrics, fixtures, ground truth, graph/path quality, runtime, memory, regression gates, and benchmark adapters. See `research/evaluation-harness/`.
- [x] Framework, lifecycle, and entrypoint modeling research completed: routes, jobs, queues, CLIs, MCP tools, serverless handlers, callbacks, decorators, generated dispatch, native Rust provider path, and validation strategy. See `research/framework-entrypoints/`.
- [x] Semantic index deep research completed: scopes, aliases, generated symbols, type-aware resolution, unresolved references, extension-provided resolution facts, and export identity. See `research/semantic-index/`.
- [x] Module/package/dependency/repo topology graph research completed: package managers, lockfiles, workspaces, import-to-package resolution, source sets, build targets, repo topology overlays, and extension facts. See `research/module-graph/`.
- [x] CFG and control dependence research completed: operation nodes, basic blocks, typed normal/abrupt/exceptional edges, dominance/postdominance, control dependence, path evidence, and extension overlays. See `research/cfg-control-flow/`.
- [x] Type, value, points-to, and alias analysis research completed: native type/value/place facts, flow narrowing, summaries, bounded Andersen-style points-to, alias provider stack, precision/cost ladder, and extension hooks. See `research/type-alias-points-to/`.
- [ ] Revisit call graph implementation details against the analysis kernel and evaluation harness.
- [ ] Revisit data-flow implementation details against the analysis kernel, call graph, CFG, and evaluation harness.
- [ ] Research function effects and summaries.
- [ ] Research abstract interpretation domains.
- [ ] Research program slicing, path explanation, and evidence.
- [ ] Research incremental query engine and caching beyond the first kernel design.
- [ ] Research rule SDK, query ergonomics, and AI-agent authoring.
- [ ] Start implementation only after the kernel/evaluation/entrypoint research gives a stable first vertical slice.

This roadmap orders the remaining research needed to turn polint into a native, multi-language static-analysis engine that AI agents can write high-value repo-local rules and analysis extensions on top of.

The order is dependency-driven. Later topics should consume the facts, precision labels, and implementation lessons from earlier topics instead of reinventing their own substrate.

## Product Thesis: Agent-Extensible Static Analysis

Most traditional static-analysis tools were designed around a hard constraint:

> The analyzer must be a mostly black-box product that works across arbitrary codebases with little or no codebase-specific help.

That assumption drives a huge amount of classic design:

- auto-discover every endpoint;
- infer every framework convention;
- ship universal source/sink models;
- hide solver internals;
- keep the configuration surface small;
- make conservative guesses that are acceptable across many users;
- treat repo-specific knowledge as a last-mile configuration problem.

polint has a different operating model.

The primary user is not only a human clicking through a UI. The primary user is an AI coding agent such as Claude Code, Codex, and future agentic development systems that can read the repository, inspect conventions, run commands, update files, and generate repo-local code. Current agent tooling is explicitly built around reading codebases, editing files, running commands/tests, and integrating with developer tooling. polint should use that fact as a first-principles design input.

The product is therefore less like a sealed universal analyzer and more like:

```text
sane native analysis defaults
  + typed facts
  + explicit uncertainty
  + repo-local rules
  + repo-local analysis models
  + agent-authored extensions
  + validation/benchmark feedback
```

This is a fundamental shift. polint does not need to perfectly auto-discover every API endpoint, framework lifecycle, route, source, sink, sanitizer, or domain convention in every codebase. It needs enough default analysis to expose the right facts and uncertainty, then a powerful extension surface so an agent can add accurate repo-specific knowledge.

Examples:

- Instead of hardcoding every web framework route pattern, expose `Entrypoints<'_>` and let an agent add a repo-local model for `apps/api/router.ts`.
- Instead of pretending a generic source/sink model is complete, let an agent define the codebase's actual untrusted boundaries, privileged APIs, sanitizers, and guard functions.
- Instead of forcing one global call graph algorithm, let an agent choose or extend the relevant provider tier for the repo's language/framework style.
- Instead of making data flow a closed taint engine, let an agent add function summaries, additional flow steps, barriers, framework dispatch edges, and domain-specific return-side flows.
- Instead of reporting "unknown" as a dead end, make unknowns actionable: the agent can inspect unresolved calls and write a model that removes that uncertainty.

The research goal is therefore not just "find the best static-analysis algorithms." The goal is to design an analysis framework where algorithms have **agent-extensible lifecycle hooks** and every extension remains testable, cached, provenance-labeled, and honest about precision.

## Design Consequences

This thesis changes how we should research and implement every analysis family.

### 1. Extension Points Are Product Surface

Classic analyzers hide most modeling behind tool internals. polint should expose carefully designed extension points for:

- entrypoints and framework lifecycle;
- call resolution;
- source/sink/sanitizer/barrier models;
- additional data-flow steps;
- function summaries and effects;
- dependency/module boundaries;
- type/value/alias hints;
- generated-code conventions;
- domain-specific evidence formatting.

These extension points can be more sophisticated than a normal linter config because the main author may be an AI agent that can inspect code, write Rust rule/model code, run tests, and iterate.

### 2. Defaults Should Be Sane, Not Maximal

The default engine should be conservative and useful:

- parse files;
- emit symbols/references/imports;
- emit syntactic call sites;
- resolve high-confidence direct edges;
- emit local CFG/data-flow facts;
- mark unresolved or unsupported behavior explicitly.

It should not try to become a fully universal black-box SAST product by default. Expensive or risky inference should be opt-in, rule-requested, or agent-modeled.

### 3. Unknowns Are Integration Tasks

In old tools, unresolved calls or unknown framework behavior often become hidden false negatives. In polint, they should become visible facts:

```text
unresolved call: dynamic router dispatch
missing source model: request context wrapper
unknown sanitizer: project-specific validator
unsupported lifecycle: custom job scheduler
```

The agent can then decide whether to write a model, add a summary, or accept lower precision.

### 4. More Complex APIs Are Acceptable If They Are Typed And Testable

Many static-analysis products avoid complex extension APIs because humans will not maintain them. polint can support a higher-capability integration surface if it is:

- strongly typed;
- generated from templates;
- validated through fixtures;
- explained through diagnostics;
- versioned and cache-keyed;
- benchmarked for precision/cost impact;
- isolated from unstable internal implementation details.

The public SDK should stay ergonomic for simple rules, but the advanced model/extension SDK can be richer.

### 5. The Evaluation Target Changes

We should evaluate two modes:

1. **Default mode:** how good are the built-in facts without repo-specific help?
2. **Agent-extended mode:** how much precision/recall improves after an agent adds repo-local models/extensions?

This second mode is the product differentiator. Research should measure the delta from agent-authored integrations, not only generic algorithm quality.

## Context Sources For This Thesis

This roadmap is grounded in the local product definition and current agent tooling:

- `README.md`: polint is "AI-agent-native" and turns repo-specific engineering instructions into executable lint rules.
- `docs/INITIAL_PROMPT.md`: polint is a framework for custom, codebase-specific static-analysis rules in an AI-native development world, not a packaged universal ruleset.
- OpenAI Codex documentation: Codex is a coding agent that can read, modify, and run code in repositories, answer codebase questions, and work in cloud/local coding environments: <https://platform.openai.com/docs/codex>
- Anthropic Claude Code documentation: Claude Code is an agentic coding tool that lives in the terminal, can build features, debug issues, and navigate codebases: <https://docs.anthropic.com/en/docs/claude-code/overview>

These sources support the core assumption: the user of polint's advanced analysis surface can be an agent capable of inspecting the repository and generating repo-local integration code.

## Current Research Baseline

| Track | Status | Folder | What It Gives Us |
|---|---|---|---|
| R-1. Semantic Index | Deep research complete, implement vertical slice next | `docs/facts/symbols-and-references.md`, `crates/polint/src/sdk/facts.rs`, `research/semantic-index/` | Stable symbol/reference baseline exists today. Research now recommends deepening it into `ScopeFact`, `ImportFact`, `AliasFact`, `ResolutionFact`, stable keys, xref indexes, explicit unknowns, and validated extension-provided semantic facts. |
| R0. Call Graphs | Done, deepen during implementation | `research/call-graphs/` | Call-site/call-edge fact model, algorithm tiers, unresolved-call model, repo-local call model layer, default-vs-extended evaluation, cost/accuracy tradeoffs across Go, TS/JS, Java, Python. |
| R1. Data Flow | Done, deepen during implementation | `research/data-flow/` | Data-flow fact model, local/sparse/summarized/global flow strategy, IFDS/IDE timing, source/sink/sanitizer/summary model layer, call-graph dependency, agent-era domain lessons, default-vs-extended evaluation. |
| R2. Agent Extension Surface | Done, implement first vertical slice | `research/agent-extension-surface/` | Recommended Rust-code extension lifecycle for agent-authored engine improvements: process-isolated extension crates, typed sinks, provenance, validation, extension-aware capability planning, default-vs-extended deltas. |
| R3. Analysis Kernel | Done, implement before call graph/data flow | `research/analysis-kernel/` | Hybrid internal kernel recommendation: deterministic provider DAG, typed fact layers, sidecar provenance, validation/merge gates, layer-specific cache keys, relation/fixpoint sub-engine for recursive analyses, explicit unknowns, and extension-aware capability support. |
| R4. Evaluation Harness | Done, implement before call graph/data flow | `research/evaluation-harness/` | External-benchmark-first evaluation strategy, suite adapters, canonical expected/observed schema, default-vs-extension deltas, graph/path/fact/diagnostic metrics, performance/cache baselines, and native fixtures for engine invariants. |
| R5. Framework, Lifecycle, And Entrypoint Modeling | Done, implement first fact-family vertical slice | `research/framework-entrypoints/` | Native framework boundary layer recommendation: `Entrypoints<'_>`, trust-boundary facts, framework dispatch overlays, explicit unknowns, Go and TS/JS first recognizers, MCP as a first-class boundary, repo-local Rust providers, validation fixtures, and default-vs-extension metrics. |
| R6. Module, Package, Dependency, And Repo Topology Graph | Done, implement before serious call graph/data-flow integration | `research/module-graph/` | Layered native topology model: workspace roots, packages/projects/source sets, declared requirements, lockfile/native/tool-reported resolved edges, import-to-package facts, build target overlays, repo topology, package-manager coverage, precision labels, cache keys, and extension merge rules. |
| R7. CFG And Control Dependence | Done, implement before type/value/alias and serious data-flow integration | `research/cfg-control-flow/` | Native CFG model: operation nodes, basic blocks, typed normal/abrupt/exceptional/cleanup edges, graph views, reachability, dominators, postdominators, control dependence, path evidence, extension overlays, Go/TS first implementation path, and differential validation plan. |
| R8. Type, Value, Points-To, And Alias Analysis | Done, implement type/value/place substrate before global call/data-flow precision | `research/type-alias-points-to/` | Native layered analysis plan: places/access paths, declared/inferred/narrowed type facts, abstract values/allocation tokens, local flow, summaries, bounded Andersen-style points-to, alias provider stack, precision/cost ladder, and agent-authored Rust extension sinks. |

These tracks are not implementation endpoints. They are inputs to the next research tracks.

## Recommended Research Order

### 1. Analysis Kernel: Fact Layers, Scheduling, Provenance, And Invalidation

**Folder:** `research/analysis-kernel/`

Status: researched in `research/analysis-kernel/`.

This was the highest-priority research topic before implementation. If polint is going to become the most capable static-analysis engine we can build, it must not grow as disconnected feature slices. Entrypoints, CFG, call graphs, data flow, effects, and rules should all plug into one shared analysis kernel.

The kernel is the substrate:

```text
typed fact families
  + fact layers
  + provenance/confidence/precision
  + extension-emitted facts
  + validation gates
  + dependency tracking
  + fixpoint scheduling
  + incremental invalidation
  + evidence paths
```

Research questions:

- What is the right internal model for fact families, fact layers, derived facts, extension-emitted facts, and native facts?
- How should polint schedule analyses that depend on each other without hardcoding one-off phase order for every feature?
- Should the engine use a demand-driven query model, an eager pipeline, a Datalog-like relation engine, Salsa-like incremental queries, or a hybrid?
- How should provenance, confidence, precision, and validation status attach to every fact without bloating rule ergonomics?
- How should extension facts merge with native facts: augment, override, suppress, replace, or conflict?
- How should cache keys and invalidation work when source code, config, rule code, extension code, SDK versions, and input facts change?
- What graph/storage shape supports cross-language facts without leaking AST/parser internals?
- How should unknowns, low-confidence facts, and unsupported behavior be represented as first-class facts?

Deliverables completed:

- Analysis kernel architecture recommendation.
- Fact layer model: native, derived, extension, synthetic, heuristic, validated.
- Scheduling model for native analyzers, extension providers, and rules.
- Provenance/confidence/precision schema shared by call graph, data flow, effects, and diagnostics.
- Cache and invalidation strategy for source files, rules, extensions, and derived facts.
- Extension merge/conflict policy.
- Minimal internal APIs needed for the first implementation without freezing a too-small public API.
- Decision on whether to research/use Salsa, Datalog/Souffle-like relations, petgraph-backed relations, or a custom query scheduler.

Core decision: use a hybrid internal kernel. Start with a deterministic provider DAG and typed fact layers, add sidecar provenance/validation/merge gates, split cache keys by layer, and add an internal relation/fixpoint sub-engine for recursive analyses. Copy Salsa/rust-analyzer invalidation concepts and Souffle/CodeQL relation concepts without adopting either as the first public or storage architecture.

### 2. Evaluation Harness And Ground Truth

**Folder:** `research/evaluation-harness/`

Status: researched in `research/evaluation-harness/`.

This must be implemented before or in parallel with the first serious call graph/data-flow implementation. If the goal is "most capable," every architecture choice must be measurable. The harness should measure both default analysis and agent-extended analysis.

Research questions:

- How should polint measure precision, recall, runtime, memory, unresolved facts, graph deltas, path quality, and false positives/false negatives?
- What benchmark suites, fixtures, or synthetic corpora should be imported or recreated for Go, TS/JS, Java, and Python?
- How should dynamic traces, tests, coverage, or framework runtime metadata be used as partial ground truth?
- How should we measure whether an agent-authored extension improved analysis instead of hiding uncertainty?
- What fixture format should assert facts, graph edges, data-flow paths, evidence paths, and diagnostics?
- How should evaluation represent default mode versus extension-enabled mode?
- How should benchmark results stay reproducible in CI and useful to agents?

Deliverables completed:

- Standard benchmark schema.
- Fixture taxonomy by language and analysis family.
- Metrics report template.
- Default-vs-agent-extended delta report.
- Ground-truth strategy for call edges, entrypoints, data-flow paths, and effects.
- Regression harness recommendation.
- Acceptance thresholds for introducing new fact families or public SDK views.
- Repository index for benchmark/code repositories inspected.
- Paper/source index for external benchmark and harness research.
- Pseudo-code for diagnostic/fact/graph/path scoring, tier scheduling, baselines, determinism, cache invalidation, and extension safety gates.

Core decision: use an external-benchmark-first harness, not an external-only harness. OWASP, SecBench.js, RealVuln, gosec, CodeQL tests, Pyre/Pysa, DroidBench, SecuriBench Micro, CryptoAPI-Bench, Juliet/SARD, Jelly, CrossCommitVuln-Bench, and SecCodeBench should supply independent outcome evidence where they fit. Native polint fixtures are still required for provenance, provider scheduling, cache invalidation, extension validation/merge behavior, stable fact keys, unknown facts, and typed SDK invariants.

### 3. Semantic Index: Symbols, References, Scopes, Imports

**Folder:** `research/semantic-index/`

Status: researched in `research/semantic-index/`.

The baseline symbol/reference layer has already been implemented. This deepening track is now complete enough to guide the next implementation slice because framework entrypoint recovery, module graph, call graphs, data flow, type inference, effects, and AI-authored extensions all depend on improving scopes, aliases, generated symbols, type-aware resolution, and explicit unresolved-resolution facts over time.

Research questions:

- How should polint model declarations, symbols, references, lexical scopes, imports, exports, aliases, fields, methods, packages, modules, and generated symbols?
- What precision tiers are needed for Go, TS/JS, Java, and Python?
- How do CodeQL, rust-analyzer, TypeScript, Go `x/tools`/gopls, Pyright, Ty, Pyrefly, JDT, Soot/SootUp/WALA, Semgrep, SCIP, LSIF, and Kythe store, query, and export symbol/reference facts?
- How should unresolved or ambiguous references be represented?

Deliverables completed:

- `SymbolFact`, `ReferenceFact`, `ScopeFact`, `ImportFact`, and `ResolutionFact` proposal.
- Language-specific resolution ladders.
- Accuracy/cost report for syntactic, binder, type-aware, and package-aware resolution.
- SDK shape for `Symbols<'_>`, `References<'_>`, `Scopes<'_>`, `Imports<'_>`.
- Extension-hook proposal for agent-supplied aliases, generated symbols, framework-specific references, and resolution overrides with provenance.

Core decision: implement semantic indexing as language-owned native Rust providers that emit normalized typed facts with stable identities, provenance, precision, explicit resolution status, and validated extension merges. Use compiler/LSP systems as semantic truth references, CodeQL-like fixpoints for recursive derived relations, and SCIP/Kythe-like identities for export only.

### 4. Agent Extension Surface And Model Lifecycle

**Folder:** `research/agent-extension-surface/`

This product-specific research track is now complete enough to guide implementation. It should remain active during implementation because it changes how all later algorithms expose hooks.

Research questions:

- What is the right split between simple repo-local rules and advanced repo-local analysis models?
- Should analysis extensions be Rust code, config/TOML, generated model files, declarative facts, or a combination?
- How can agents add call edges, entrypoints, source/sink models, summaries, data-flow steps, and framework semantics without corrupting engine invariants?
- How should extension facts be validated, tested, cache-keyed, versioned, and attributed?
- What guardrails prevent an agent-generated model from silently converting unknown behavior into false certainty?
- How do CodeQL model packs, Semgrep rules, Pysa models, TypeScript language service plugins, ESLint custom rules, rust-analyzer assists, and MCP-style tool integrations inform the design?

Deliverables:

- Extension API taxonomy: rule-only, model-only, provider-hook, generated facts, and debug-only extensions.
- Proposed repo layout for `.polint/models/` or equivalent.
- Validation lifecycle: generate -> bind to facts -> test fixtures -> benchmark delta -> activate.
- Capability model for extension-provided facts.
- Provenance and trust model for AI-authored extensions.

### 5. Framework, Lifecycle, And Entrypoint Modeling

**Folder:** `research/framework-entrypoints/`

Status: researched in `research/framework-entrypoints/`.

This is the first domain-specific research track after the kernel/evaluation work. Call graphs and data flow are often wrong because entrypoints are wrong. This is especially important for web apps, tests, CLIs, background jobs, MCP tools, serverless functions, decorators, annotations, and framework routers.

For polint, the target is not to auto-discover every framework pattern in the world. The target is:

```text
default recognizers for common patterns
  + visible unknowns
  + repo-local agent-authored framework models
```

Research questions:

- How should polint discover externally reachable functions?
- How should frameworks declare custom entrypoint/source/sink/route semantics?
- What should the agent extension format look like for a repo's actual routes, handlers, jobs, queues, tool definitions, tests, and generated dispatch?
- How do CodeQL model packs, Semgrep framework rules, FlowDroid lifecycle modeling, Pysa models, MCP-BiFlow, Spring analyzers, Go HTTP analyzers, and JS router analyzers approach this?
- How can AI agents author or refine framework models without making unsound claims?

Deliverables completed:

- `Entrypoints<'_>` and framework boundary fact proposal.
- Model format for routes, handlers, callbacks, decorators, lifecycle methods, MCP tools/resources/prompts, CLIs, jobs, tests, and generated dispatch.
- Trust-boundary model for HTTP, MCP, CLI/env/stdin, queues/jobs, request/response, and return-side agent-visible outputs.
- Native Rust implementation path for built-in providers plus repo-local agent-authored providers.
- Go and TS/JS first-scope recommendations, with Python and Java/JVM kept as future adapter research input.
- Validation plan for fact precision/recall, unknown reduction, extension delta, cache determinism, and provider cost.

Core decision: implement a native framework boundary layer before full call graph and data flow. Recover framework/protocol boundary facts with provenance; do not claim exact runtime behavior. Feed validated facts into call graph and data flow as optional synthetic dispatch and trust-boundary overlays.

### 6. Module, Package, Dependency, And Repo Topology Graph

**Folder:** `research/module-graph/`

Status: researched in `research/module-graph/`.

This should follow semantic indexing because imports and package roots are part of name resolution, but it deserves its own research track because repo-local policies often target architecture boundaries.

Research questions:

- How should polint represent Go modules/packages, TS/JS npm/workspace modules, Java packages/classpaths, Python packages/import paths, and monorepo workspace roots?
- How do we model dependency direction, layer boundaries, public/private APIs, test-only imports, generated code, vendored code, and external dependencies?
- How should lifecycle inputs participate in cache digests?

Deliverables completed:

- Layered fact model for `WorkspaceRootFact`, `PackageFact`, `SourceSetFact`, `DependencyRequirementFact`, `ResolvedDependencyFact`, `ImportToPackageFact`, and `RepoTopologyFact`.
- Language reports for Go, TS/JS, Python, Java/JVM, and Cargo.
- Package manager coverage matrix for Go modules, npm, pnpm, Yarn, Bun, pip, uv, Poetry, PDM, conda, Maven, Gradle, Bazel, Pants, and Cargo.
- OSS repository index with local source evidence from Go, gopls, TypeScript, Oxc resolver, npm Arborist, pnpm, Yarn Berry, Bun, uv, pip/resolvelib, Poetry, PDM, conda, Maven Resolver, Gradle, Bazel, Pants, Nx, Turborepo, and Cargo.
- Research paper/source index covering dependency solving, package-calculus models, lockfile design, and build-system theory.
- Native Rust implementation path with parsers, providers, validation, cache keys, extension merge rules, and SDK promotion timing.
- Benchmark and validation plan for roots, packages, declared edges, resolved edges, import ownership, source sets, unknowns, and extension deltas.

Core decision: build layered topology facts, not one universal dependency graph. Parse manifests and lockfiles natively first, implement Go MVS as the first full native resolver, support TS/JS/Python/JVM common managers through exact static facts and lockfile readers, and represent Gradle/Bazel/Pants dynamic build logic as conservative or tool-reported/extension-provided facts until exact native modeling exists.

### 7. CFG: Control Flow Graphs And Control Dependence

**Folder:** `research/cfg-control-flow/`

Status: researched in `research/cfg-control-flow/`.

Data-flow precision depends on CFG quality. This must happen before serious abstract interpretation and before high-confidence interprocedural data flow.

Research questions:

- How should polint represent basic blocks, statements, branches, loops, returns, panic/throw, exceptions, `defer`, `finally`, short-circuiting, async/await, generators, closures, and early exits?
- What is the smallest cross-language CFG that supports local data-flow without leaking unstable AST internals?
- How do Oxc, TypeScript, Go SSA, Rust MIR, Checker Framework, Soot/Jimple, WALA, Pyre, and CodeQL represent control flow?
- How should control dependence be exposed for diagnostics and rules?

Deliverables completed:

- `Cfg<'_>` SDK view proposal.
- Local CFG algorithms in Python-ish pseudocode.
- Precision limits per language.
- Fixture plan for branches, loops, exceptions, async, defer/finally.
- Extension points for framework-specific control transfers such as routers, schedulers, callbacks, generated handlers, and test harnesses.
- OSS repository index covering Go SSA/cfg, Oxc, TypeScript, ESLint, CodeQL, Pyright, Pyre, CPython, mypy, Soot, SootUp, WALA, Checker Framework, OPAL, LLVM/MLIR, Joern, Semgrep, TAJS, and Jelly.
- Research paper/source index covering control dependence, SSA/control-dependence construction, Java exception CFGs, Checker Framework dataflow, CodeQL/QL, TAJS, LLVM/MLIR, JLS/JVMS, and Python bytecode semantics.
- Native Rust implementation path for internal fact schema, shared builder, Go/TS providers, derived dominance/postdominance/control-dependence, extension overlays, cache keys, and SDK promotion.

Core decision: build layered native CFG facts, not one universal AST-walk graph. Represent operation nodes, basic blocks, typed normal/abrupt/exceptional/cleanup edges, and virtual exits. Compute reachability, dominators, postdominators, and control dependence as derived facts over explicit graph views. Keep call graph/framework dispatch separate from local CFG, and allow agent-authored extension overlays only through validated, provenance-labeled sinks.

### 8. Type, Value, Points-To, And Alias Analysis

**Folder:** `research/type-alias-points-to/`

Status: researched in `research/type-alias-points-to/`.

This research feeds call graphs and data flow. It should not be treated as a data-flow subchapter; alias/points-to precision determines whether method dispatch, heap flow, field flow, and summaries are useful.

Research questions:

- Which type facts can be extracted cheaply per language?
- How should possible values, allocation sites, object identities, function tokens, receiver types, and aliases be represented?
- Where do Andersen, Steensgaard, RTA/VTA, object sensitivity, field sensitivity, access paths, shape analysis, and type narrowing fit?
- How do TypeScript, Go `x/tools`, Pyright/Pyre, WALA, Doop, Soot Spark, OPAL, TAJS, Jelly, and PoTo handle the precision/cost frontier?

Deliverables completed:

- `Types<'_>`, `Values<'_>`, `Aliases<'_>`, and `PointsTo<'_>` research recommendation.
- Cost/accuracy table for type inference, value facts, points-to, alias, context sensitivity, and sparse refinement tiers.
- Native implementation path for place/access-path facts, type facts, value/allocation facts, local narrowing, summaries, bounded Andersen-style points-to, and alias provider queries.
- How points-to/value facts should feed call graph and data-flow providers without making call graphs/data flow submodules of alias analysis.
- Agent-supplied type/value/summary/points-to/alias fact model with strict provenance, validation, precision ceilings, cache keys, and conflict handling.
- OSS implementation reports for Ty/Ruff, Pyrefly, Pyright, Pyre/Pysa, mypy, pytype, TypeScript, Oxc, Flow, TAJS, Jelly, CodeQL, Go tools, Staticcheck, Doop, WALA, Soot/SootUp, OPAL, Checker Framework, LLVM, SVF, Rust borrowck/Polonius, rust-analyzer, Souffle, and Joern.
- Paper/source index covering Andersen/Steensgaard/Shapiro-Horwitz, SVF, Doop, TAJS, Flow, IFDS, abstract interpretation, PoTo, CodeQL, LLVM AliasAnalysis/MemorySSA, Ty, Pyrefly, Oxc, WALA, and current trend preprints.

Core decision: do not build a mandatory whole-repo alias graph first. Implement layered native facts: `PlaceFact`, `TypeFact`, `NarrowedTypeFact`, `ValueFact`, `AllocationTokenFact`, `SummaryFact`, requestable bounded points-to constraints, and an alias provider stack that returns `NoAlias`, `MayAlias`, `MustAlias`, `PartialAlias`, or `Unknown` with evidence. Use Ty/Pyright/Pyrefly/TypeScript/Go/WALA/Soot/SVF/LLVM as references and validation oracles, not runtime dependencies.

### 9. Function Effects And Summaries

**Folder:** `research/effects-summaries/`

Effects are the bridge between local facts and practical repo rules. They also provide a compact substrate for interprocedural data flow and agent explanations.

Research questions:

- How should polint summarize reads, writes, mutations, calls, allocations, returns, throws, panics, logging, network, file system, process execution, environment access, database calls, and external tool calls?
- What is the relationship between effect summaries, data-flow summaries, and call graph edges?
- How can agents add or correct project-specific summaries for wrappers, adapters, generated clients, RPC layers, and framework APIs?
- How do Pysa, CodeQL summaries, CFTaint, Infer, Checker Framework, WALA slicing, and effect systems model this?

Deliverables:

- `Effects<'_>` SDK view proposal.
- Function summary schema.
- Query examples: "this handler writes to disk", "this function logs secrets", "this utility is pure", "this path can execute shell".
- Model format for agent-authored summaries and effect overrides, including benchmark-required proof before high-confidence activation.

### 10. Abstract Interpretation Domains

**Folder:** `research/abstract-interpretation/`

This is where polint gets beyond call/data-flow and into precise semantic rule facts.

Research questions:

- Which domains are most valuable first: nullness/nilness, constants, string values, numeric ranges, initializedness, resource state, typestate, permissions, path conditions, option/result state?
- How do Checker Framework, Infer, Rust MIR analyses, TypeScript narrowing, Pyre, Clang Static Analyzer, Goblint, and abstract interpretation literature balance precision and cost?
- What domains can be local-only, summary-based, or interprocedural?
- Which domains benefit from agent-supplied invariants or repo-specific typestate definitions?

Deliverables:

- Domain priority list.
- Generic lattice/transfer interface recommendation.
- `Nilness<'_>`, `Constants<'_>`, `StringValues<'_>`, `Typestate<'_>` candidate fact views.
- Safe model format for repo-specific invariants, guard functions, state transitions, and domain-specific validators.

### 11. Program Slicing, Path Explanation, And Evidence

**Folder:** `research/program-slicing/`

AI agents and humans need explanations, not just true/false answers. Slicing connects diagnostics to evidence.

Research questions:

- How should polint compute backward slices, forward slices, chop queries, control dependence, data dependence, and call/data-flow path explanations?
- How do Joern, CodeQL path queries, WALA slicer, System Dependence Graphs, and modern SAST tools present evidence?
- How can slices stay small enough to be useful for AI agents?
- How should evidence distinguish engine-derived facts from agent-authored model facts?

Deliverables:

- `Slice<'_>` and `EvidencePath` proposal.
- Path compression and ranking strategy.
- Diagnostic evidence format for CLI JSON and SDK rules.
- Evidence provenance schema for "native", "heuristic", "agent-authored", "validated", and "unvalidated" path segments.

### 12. Incremental Analysis, Query Engine, And Caching

**Folder:** `research/incremental-query-engine/`

This should be researched before implementing expensive global analyses. Re-running full call/data-flow on every agent edit will not scale.

Research questions:

- How should facts, summaries, queries, and diagnostics be invalidated after file edits?
- How should agent-authored models invalidate dependent call graph, data-flow, effect, and diagnostic results?
- How do Salsa, rust-analyzer, FlowLog, Souffle, IncIDFA, CodeQL databases, TypeScript incremental compiler, and Pyre incremental checking work?
- Which parts of polint should be demand-driven versus eagerly materialized?

Deliverables:

- Dependency graph for facts and summaries.
- Cache key strategy for language lifecycle and rule options.
- Query scheduling model.
- Incremental benchmark plan.
- Model digest and extension dependency strategy.

### 13. Rule SDK, Query Ergonomics, And AI-Agent Authoring

**Folder:** `research/agent-rule-authoring/`

This should happen after the core fact family and extension-surface research are underway, but before public APIs harden. The SDK shape is the product surface.

Research questions:

- What typed fact views should agents use directly?
- How should rules express source/sink/sanitizer matchers, graph queries, summaries, evidence, and uncertainty budgets?
- How should generated rules declare required facts without manual capabilities?
- How does an agent decide whether to write a rule, a model, a summary, or a provider extension?
- How do CodeQL, Semgrep, Pysa models, Joern queries, ESLint, Rust Clippy, and custom policy engines make rules authorable?

Deliverables:

- Public SDK ergonomics report.
- Example generated rules using only `polint::sdk::prelude::*`.
- Capability derivation and diagnostics plan.
- Agent-facing documentation style guide.
- Decision tree for rule vs model vs analysis extension.

## Cross-Cutting Research Standard

Every new research folder should follow the same structure:

```text
research/<topic>/
  README.md
  FINAL-REPORT.md
  RECOMMENDED_IMPLEMENTATION.md
  RESEARCH-ANALYSIS.md
  REPO-INDEX.md
  PAPER-INDEX.md
  STANDARD.md
  VALIDATION.md
  algorithms/
  implementation/
  languages/
  oss/
  papers/
  repos/        # gitignored
```

Each report should explicitly cover:

- algorithms;
- accuracy and failure modes;
- time and memory complexity;
- OSS implementations inspected;
- research papers read;
- per-language differences;
- what polint should copy;
- what polint should avoid;
- public SDK implications;
- agent extension implications;
- default mode vs agent-extended mode;
- benchmark and validation plan.

## High-Level Dependency Map

```text
Analysis Kernel
  -> Evaluation Harness
  -> Semantic Index Deepening
  -> Agent Extension Surface
  -> Framework/Entrypoint Modeling
  -> Module Graph
  -> CFG
  -> Type/Alias/Points-To
  -> Call Graphs
  -> Data Flow
  -> Effects/Summaries
  -> Abstract Interpretation
  -> Slicing/Evidence
  -> Agent Rule SDK

Incremental Query Engine deepens the Analysis Kernel once expensive global analyses begin.
Evaluation Harness runs across all tracks and gates public precision claims.
Framework/Entrypoint Modeling feeds Call Graphs, Data Flow, Effects, and Slicing.
Agent-authored models and extensions feed every analysis family, but must bind to typed facts and validation results.
```

## Research Lens For Every Track

When researching a topic, answer two different questions:

1. **What can the native engine infer by default across many repositories?**
2. **What can an AI agent accurately add after inspecting this specific repository?**

The second question is where polint can outperform older static-analysis products. We should intentionally design for a workflow where an agent:

```text
observes unresolved/low-confidence analysis facts
  -> inspects the repo-specific framework/convention
  -> writes a rule/model/extension
  -> runs fixtures and benchmarks
  -> activates the model with provenance and cache digests
  -> improves future analysis for humans and agents
```

That means research should prefer architectures with clear typed extension points over architectures that are theoretically elegant but closed to repo-specific augmentation.

## Recommended Next Research Task

Start with:

```text
research/effects-summaries/
```

Reason: type/value/place/points-to research makes it clear that summaries are the scaling boundary. Without a strong effect and summary model, call graphs, data flow, alias queries, and agent-authored framework integrations will either stay local or become expensive whole-program guesses.

Then revisit:

```text
research/call-graphs/
```

Reason: call graph implementation should now consume the kernel, evaluation harness, semantic index, module graph, framework dispatch overlays, CFG, type/value facts, and points-to/value-token decisions rather than inventing its own lifecycle.

Then revisit:

```text
research/data-flow/
```

Reason: data flow should consume CFG, call graph, semantic index, module graph, type/value/alias facts, summaries, and extension-model decisions so source/sink/sanitizer/summary facts are accurate and explainable.
