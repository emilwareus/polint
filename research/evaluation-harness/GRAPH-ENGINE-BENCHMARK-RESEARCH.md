# Graph Engine Benchmark Research

This document describes what to implement in polint's scanner engine to target
higher graph benchmark scores. It focuses on the two supported graph suites:

- Go: Go x/tools RTA call graph testdata.
- TypeScript / JavaScript: Jelly call graph micro suite.

The goal is not to add benchmark-specific hacks. The goal is to make the core
engine capable enough that benchmark improvements also represent better
repo-local policy analysis.

## Current Baseline

Generated report source: `.context/graph-benchmarks/summary.md`.

| Suite | Mode | TP | FP | FN | Precision | Recall | Unknowns | Runtime ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Go x/tools RTA callgraph | polint baseline | 1 | 9 | 36 | 0.1000 | 0.0270 | 26 | 309 |
| Jelly callgraph micro | polint baseline | 2 | 6 | 313 | 0.2500 | 0.0063 | 28 | 674 |

The results are low for two different reasons:

- Go x/tools is an RTA oracle. It expects reachability, function values,
  interface dispatch, init edges, generics behavior, and limited reflection
  modeling. polint currently mostly emits syntactic and refined heuristic call
  facts.
- Jelly is a JS/TS call graph oracle. It expects exact callsite/function source
  locations plus function-token propagation through variables, properties,
  callbacks, constructors, prototypes, modules, and native/library models.
  polint currently has only a narrow direct/refined call layer.

## Existing Engine Shape

polint already has several pieces of an internal representation:

- MIR operations: storage, binding, assignment, reads, writes, branches, calls,
  returns, and unsupported semantic facts.
- CFG facts: block, edge, entry, exit, reachability, and terminator facts.
- Call facts: call sites, direct targets, unresolved calls.
- Refined call facts: framework, extension, Go, TS/JS, summary, and internal
  refinement sources.
- Type, value, points-to, data-flow, summary, and evidence fact families.

The gap is that these facts are not yet used as a single graph solving system.
The current pipeline has useful local facts, but benchmark-grade call graph
accuracy requires a shared semantic graph where frontends add constraints and
solvers derive reachable call edges.

## Metric Terms

- Precision = TP / (TP + FP). Higher precision means fewer invented edges.
- Recall = TP / (TP + FN). Higher recall means more expected benchmark edges are
  recovered.
- Unknowns = explicit unresolved or unsupported facts. Reducing unknowns is good
  only when the replacement is correct; converting unknowns into bad guesses
  raises recall only by damaging precision.

Complexity notation:

- `N`: AST or MIR operations.
- `F`: functions.
- `C`: callsites.
- `E`: graph edges.
- `P`: packages or modules.
- `V`: variables, places, or abstract locations.
- `T`: tokens, abstract function values, or abstract objects.
- `A`: address-taken functions.
- `D`: dynamic function-call sites.
- `R`: runtime concrete types.
- `I`: interface or method invocation sites.
- `M`: external model or adaptation facts.

The metric projections below are estimates from the current fast-tier reports.
They should be remeasured after each implementation slice.

## 1. Benchmark Identity And Normalization Layer

### What To Implement

Strengthen the benchmark-facing identity layer before deeper analysis work:

- Give every function and callsite a stable internal identity with file, span,
  language, package/module path, lexical container, and display name.
- Preserve exact callsite and function declaration spans in analysis output.
- Add per-benchmark renderers from internal identities to expected identities:
  Go x/tools `RelString`-style names and Jelly
  `file:start_line:start_col:end_line:end_col` spans.
- Deduplicate observed edges by semantic identity before scoring.
- Split wrong identity, unsupported edge, unresolved edge, and package-load
  limitation into distinct report categories.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Go | Precision should improve by removing duplicated or mismapped heuristic edges. Recall may rise from 2.7% to roughly 5-15% if some existing edges are currently lost only because names do not normalize to x/tools expectations. |
| Jelly | Recall may rise from 0.6% to roughly 3-10% if span mismatches hide otherwise-correct edges. Precision should improve because false positives caused by unstable span identities become visible and fixable. |

This is mostly an enabler. It will not solve dynamic dispatch, function values,
or JS property flow by itself.

### Complexity

- Build identities: `O(N)`.
- Normalize expected and observed edges: `O(F + C + E)`.
- Matching with hash maps: `O(E_expected + E_observed)`.
- Memory: `O(F + C + E)`.

### Implementation Cost

Small to medium. This is the first thing to harden because otherwise later
scanner improvements can look worse than they are.

## 2. Reachability And Root Semantics

### What To Implement

Add a language-neutral reachability pass over the internal call graph:

- Represent roots explicitly: `main`, `init`, exported entrypoints, tests, suite
  roots, and configured repo entrypoints.
- Mark functions reachable from roots.
- Score benchmark rows in the mode the oracle expects. Go x/tools RTA excludes
  dead functions from the reachable call graph, so dead-code syntactic calls
  should not become baseline observed edges for that suite.
- Keep unreachable direct calls as facts, but mark them as outside the scored
  reachable graph.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Go | Precision should rise materially. The current Go result has 9 FP against 1 TP; several false positives are syntactic calls in dead or mismodeled contexts. A reachability filter could plausibly move precision from 10% to 30-60% before improving recall. Recall may stay flat until dynamic edges are added. |
| Jelly | Precision should improve where microcases contain unused functions or calls outside entrypoints. Recall should not change much unless currently-reachable edges are filtered incorrectly. |

### Complexity

- Graph traversal: `O(F + E)`.
- Root discovery: `O(F + P)`.
- Memory: `O(F)` for reachability bits plus existing graph storage.

### Implementation Cost

Small to medium. It should reuse CFG and call facts, but it needs a clear
per-suite root policy so benchmark scoring is honest.

## 3. Go Semantic Frontend: Packages, Types, SSA

### What To Implement

Add a Go semantic frontend behind the existing Go lifecycle contract:

- Load packages with module-root inference and temporary `go.work` behavior.
- Type-check selected packages and tests according to `[languages.go]`.
- Build package-level symbols, receiver types, function objects, init functions,
  and method sets.
- Build an SSA-like function body representation for Go or import the facts
  needed from `go/packages` plus `x/tools/go/ssa`.
- Emit semantic callsite references instead of relying on tree-sitter names.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Go | This is the first major recall step. It should recover package-qualified calls, receiver methods, init edges, generic instantiations that still have static targets, and exact function identities. A realistic target before RTA is 25-45% recall with 50-80% precision on the fast x/tools cases. |
| Jelly | No direct effect. |

This does not by itself solve function values, interface dispatch, or reflection.
It gives the RTA/VTA provider the facts it needs.

### Complexity

- Package loading and parsing: roughly `O(source + deps selected by package_patterns)`.
- Type checking: `O(N)` expected for normal code, with compiler-specific costs
  for generics and imports.
- SSA/fact extraction: `O(N)`.
- Memory: `O(P + N + symbols + types + SSA values)`.

Compared with tree-sitter-only parsing, this is more expensive and must be
cached by module roots, package patterns, build tags, include-tests, Go version,
and file digests.

### Implementation Cost

Large. This is a core Go engine upgrade, not a benchmark adapter patch.

## 4. Go RTA/VTA Provider

### What To Implement

Implement a Go reachability and type-flow call provider inspired by x/tools RTA:

- Track reachable functions from roots.
- Track address-taken functions.
- Track dynamic function-call sites by signature.
- Track runtime concrete types created or passed through interfaces.
- Resolve interface invokes by method set compatibility.
- Iterate to a fixed point: newly reachable functions can introduce new dynamic
  calls, address-taken functions, and runtime types.
- Report unsupported reflection edges explicitly instead of inventing them.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Go | This is the main path to high Go benchmark recall. After semantic Go facts exist, RTA should plausibly move the fast x/tools suite to 70-90% recall with high precision. Remaining misses will likely be reflection synthetic edges, generics edge identity differences, and unsupported runtime behavior. |
| Jelly | No direct effect. |

### Complexity

RTA fixed point can be described as:

- Base graph work: `O(F + E_static)`.
- Dynamic function matching: up to `O(A * D)` without indexing, where `A` is
  address-taken functions and `D` is dynamic callsites.
- Interface invoke matching: up to `O(R * I * method_lookup)` without indexing.
- With signature and method-set indexes, expected work is closer to
  `O(F + E_static + inserted_dynamic_edges + inserted_invoke_edges)`.
- Memory: `O(F + E + A + D + R + I)`.

### Implementation Cost

Large. This should be implemented as a provider over shared facts so later
repo-local rules can query the same call graph.

## 5. JS/TS Exact Function And Callsite Inventory

### What To Implement

Build a complete JS/TS inventory from Oxc:

- Function declarations, function expressions, arrow functions, methods,
  constructors, accessors, class static blocks, and generated implicit
  functions where needed.
- Calls, `new`, tagged templates, optional calls, dynamic import, require,
  getter/setter calls where modeled, and framework/native callback hooks.
- Exact spans matching Jelly expectations.
- Stable lexical parent and scope identity for every function and callsite.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Jelly | This is required for scoring. Alone, it may move recall to 3-10% if direct call edges exist but identities were missing. It should also reduce unknowns by converting "unknown location" into specific unresolved callsite facts. |
| Go | No direct effect. |

### Complexity

- AST traversal: `O(N)`.
- Scope and identity construction: `O(N)`.
- Memory: `O(F + C + scopes)`.

### Implementation Cost

Medium. The main risk is not traversal cost; it is exact location parity with
Jelly and stable identities across Oxc AST forms.

## 6. JS/TS Scope, Binding, Imports, And Module Graph

### What To Implement

Add a proper JS/TS binding layer:

- Lexical scopes for `var`, `let`, `const`, functions, classes, imports,
  destructuring, parameters, catch bindings, and re-exports.
- CommonJS `require`, ESM import/export, package entrypoint resolution, and
  TypeScript path aliases where configured.
- Direct function binding for calls like `f()`, `ns.f()`, imported functions,
  and local aliases.
- Conservative handling for unresolved dynamic import and computed property
  names.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Jelly | This should recover many simple direct and module-mediated edges. A realistic target after identity work is 10-25% recall with 50-80% precision on fast microcases, depending on how many cases are module/property heavy. Unknowns should fall materially. |
| Go | No direct effect. |

### Complexity

- Scope construction: `O(N)`.
- Import resolution: `O(P + import_edges)` with resolver cache; filesystem work
  dominates cold runs.
- Binding lookup: expected `O(1)` to `O(depth)` per reference depending on scope
  index.
- Memory: `O(V + scopes + module_edges)`.

### Implementation Cost

Medium to large. This should be shared with future repo-local policy rules that
need reliable references.

## 7. JS/TS Function-Token Propagation

### What To Implement

Implement a call graph solver for JS/TS based on function tokens:

- Treat functions as abstract tokens.
- Propagate tokens through assignments, aliases, parameters, returns, closures,
  arrays/objects where modeled, callbacks, promise continuations, and simple
  higher-order functions.
- Resolve calls by the token set flowing to the callee expression.
- Track provenance and confidence for every derived edge.
- Use widening or budgets to avoid unbounded token explosion.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Jelly | This is the main path to substantial JS/TS recall. After scope/module work, function-token propagation could move recall from the 10-25% range toward 35-60% while preserving useful precision if dynamic property flow is conservative. |
| Go | No direct effect. |

### Complexity

- Constraint construction: `O(N)`.
- Worklist propagation: proportional to inserted token facts. A practical bound
  is `O(flow_edges + token_insertions)`.
- Worst case: `O(V * T + flow_edges * T)` if many tokens can flow everywhere.
- Memory: `O(V * T)` worst case, usually much less with allocation-site
  abstraction, callsite budgets, and per-property caps.

### Implementation Cost

Large. This should be built as a reusable solver over MIR/value/points-to facts,
not as Jelly-specific logic.

## 8. JS/TS Object, Property, Prototype, Class, And `this` Model

### What To Implement

Add an object and property model that feeds the JS/TS call graph solver:

- Allocation-site abstraction for object literals, arrays, functions, classes,
  class instances, prototypes, modules, and selected native objects.
- Property writes and reads with exact string names when known.
- Conservative buckets for computed or unknown properties.
- `this` binding for method calls, constructors, bound functions, callbacks,
  arrow functions, and class methods.
- Prototype inheritance and class extension lookup.
- Accessor getter/setter call edges when modeled.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Jelly | This is likely the largest recall unlock after token propagation. Jelly microcases include classes, prototypes, property calls, dynamic object behavior, and native patterns. Expected additional gain is roughly 15-30 recall points after the token solver exists, with precision depending on property abstraction. |
| Go | No direct effect. |

### Complexity

- Constraint construction: `O(N)`.
- Property propagation: worst-case `O(objects * properties * tokens)` plus flow
  edges.
- Prototype lookup: `O(proto_depth)` per lookup, or near `O(1)` with cached
  resolved property sets and invalidation.
- Memory: `O(objects * properties * token_sets)`.

The main engineering challenge is bounding precision/performance tradeoffs:
unknown computed properties can either miss edges or explode false positives.

### Implementation Cost

Large. This is the core of a serious JS/TS scanner engine.

## 9. Native, Framework, And Adaptation Model Layer

### What To Implement

Build a validated model layer that can be extended by an adaptation agent:

- Repo-local model files for framework lifecycle, callbacks, dependency
  injection, routing, test runners, and package-specific APIs.
- Strict schema for model facts: source pattern, target pattern, confidence,
  language, scope, and evidence.
- Validation before accepting model facts: target symbols must exist unless the
  model explicitly declares an external/native boundary.
- Benchmark adapted mode that records prompt hash, changed model files,
  accepted/rejected facts, unknown delta, precision/recall delta, and runtime
  delta.
- The adaptation agent must inspect unresolved facts and code, not expected
  benchmark labels.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Jelly | For micro benchmarks, adaptation may add 5-15 recall points once the solver can consume models. On real repos, this is likely much more valuable because framework conventions dominate call graph completeness. |
| Go | Adaptation can help with generated code, framework entrypoints, and local interface conventions, but x/tools RTA itself should mostly be solved by semantic Go plus RTA rather than repo-local models. |

Adaptation should improve recall more than precision. Precision must be protected
by model validation and by reporting accepted versus rejected extension facts.

### Complexity

- Model loading and validation: `O(M * lookup_cost)`.
- Solver impact: proportional to the number of accepted constraints and their
  token sets.
- Memory: `O(M + model_edges)`.

### Implementation Cost

Medium after the core solver exists. Premature adaptation before the solver is
capable will produce weak or misleading benchmark deltas.

## 10. Unsupported And Unknown Taxonomy

### What To Implement

Make every missed edge category explicit:

- Setup missing: package/module load failed.
- Unsupported semantic domain: reflection, eval, dynamic import, native boundary,
  generated code, unsupported framework.
- Unresolved because facts are missing: no semantic reference, no receiver type,
  no function token, no property identity.
- Intentionally out of scope for a suite mode: unreachable edge, excluded test
  package, ignored native model.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Go | Precision and recall may not move directly, but diagnosis improves. Reflection and RTA-only misses stop looking like generic scanner crashes. |
| Jelly | Unknown categories will identify whether the main blocker is span identity, scope binding, property flow, module resolution, or native models. |

This is necessary for trustworthy adapted-agent work because the agent needs a
high-quality unresolved-fact queue.

### Complexity

- Classification: `O(U)` where `U` is unknown or unsupported facts.
- Report aggregation: `O(U + E)`.
- Memory: `O(U)`.

### Implementation Cost

Small to medium. It should be done continuously as each provider is added.

## 11. Incremental Cache And Performance Budget

### What To Implement

The higher-accuracy engine will be more expensive, so cache and budgets should
be part of the design:

- Cache AST, MIR, scope, symbol, type, CFG, call, points-to, and solved graph
  families separately.
- Include language config, module roots, package patterns, build tags,
  TypeScript config, lockfiles, model files, and solver budgets in digests.
- Record cold and warm benchmark runtime separately.
- Add solver budgets for token set size, property abstraction, package depth,
  dynamic call fanout, and model expansion.

### Expected Metric Impact

| Suite | Expected effect |
|---|---|
| Go | No direct accuracy gain. It makes semantic Go and RTA practical enough to run repeatedly. |
| Jelly | No direct accuracy gain. It keeps function-token and property solvers usable on real repositories. |

### Complexity

- Digest construction: `O(inputs)`.
- Cache lookup: expected `O(1)` per fact family.
- Incremental invalidation: depends on dependency graph; expected
  `O(changed_files + affected_edges)`, worst-case full recompute.
- Memory/disk: proportional to cached fact families.

### Implementation Cost

Medium. It should land before the JS/TS solver becomes broad enough to be slow.

## Recommended Implementation Order

| Step | Work | Primary metric goal |
|---:|---|---|
| 1 | Identity, span, normalization, deduplication | Make scores trustworthy and reduce false positives caused by identity bugs. |
| 2 | Reachability and root semantics | Raise Go precision before adding more edges. |
| 3 | JS/TS function and callsite inventory | Make Jelly failures diagnosable by exact source identity. |
| 4 | JS/TS scope, binding, imports, module graph | Recover simple direct and module-mediated Jelly edges. |
| 5 | Go semantic frontend | Recover static Go identities and prepare for RTA. |
| 6 | Go RTA provider | Target high Go x/tools recall. |
| 7 | JS/TS function-token solver | Target substantial Jelly recall. |
| 8 | JS/TS object/property/prototype/class model | Target high Jelly recall on dynamic/object cases. |
| 9 | Adaptation model layer | Measure `polint_agent_adapted` honestly after the core solver can use models. |
| 10 | Cache and budgets throughout | Keep cold and warm runtime acceptable. |

## Target Score Bands

These are planning targets, not commitments.

| Milestone | Go precision | Go recall | Jelly precision | Jelly recall |
|---|---:|---:|---:|---:|
| Current baseline | 10% | 3% | 25% | 1% |
| Identity + reachability | 30-60% | 5-15% | 40-70% | 3-10% |
| Go semantic frontend / JS scope module layer | 50-80% | 25-45% | 50-80% | 10-25% |
| Go RTA / JS token solver | 70-90% | 70-90% | 50-80% | 35-60% |
| JS object/property model + adaptation | N/A for Go unless models are needed | N/A for Go unless models are needed | 55-85% | 55-80% |

Precision should stay a first-class target. A scanner that gets recall by
emitting every possible edge is not useful for repo-local policy enforcement.

## Architectural Recommendation

The scanner should use a shared code representation and solver core, with
language-specific frontends feeding it.

The shared core should own:

- Stable identities for files, symbols, functions, scopes, callsites, places,
  abstract objects, and graph edges.
- MIR-like operation facts.
- CFG and reachability.
- Scope/reference/binding facts.
- Value and points-to constraints.
- Call graph constraints.
- Model/adaptation constraints.
- Provenance, precision, and unsupported/unknown taxonomy.

The language frontends should own:

- Parsing.
- Language-specific AST lowering.
- Language-specific package/module lifecycle.
- Language-specific semantic facts such as Go method sets or JS prototype
  behavior.
- Mapping back to benchmark and source-code identities.

This keeps benchmark improvement work aligned with the product. Repo-local rules
should eventually consume typed views over this same representation rather than
calling benchmark-specific adapters.

## Adaptation Boundary

The adaptation agent should not compensate for missing core semantics by writing
arbitrary expected edges. It should be allowed to add repo-local model facts that
the solver validates and consumes.

Good adaptation examples:

- "This framework calls exported `handler` functions from these route files."
- "This dependency injection container constructs implementations of this
  interface."
- "This test runner invokes functions matching this registration API."
- "This package API invokes the callback passed as argument 2."

Bad adaptation examples:

- Copying benchmark expected labels.
- Adding direct edge files that bypass identity, reachability, and solver logic.
- Adding broad wildcard models that improve recall by flooding false positives.

The adapted benchmark should report both accuracy delta and model quality:

- prompt hash;
- changed model files;
- accepted and rejected facts;
- unknown count before and after;
- precision and recall delta;
- runtime and cache delta.

## Near-Term Research Questions

- How close can Jelly location identity get with Oxc spans alone, and where do
  source maps or generated spans matter?
- Which x/tools RTA expected edges remain after semantic Go plus reachability but
  before RTA?
- Should Go SSA be built in-process through a sidecar, or should polint ingest
  serialized semantic facts from a Go helper?
- What solver budget keeps JS/TS token propagation useful on medium repos
  without collapsing precision?
- Which repo-local model schema is expressive enough for adaptation while still
  validating facts before they affect benchmark scores?

## Bottom Line

The fastest path to better benchmark numbers is not a single bigger normalizer.
It is a staged engine upgrade:

1. Make identities and reachability exact enough that the score is meaningful.
2. Add Go semantic facts and RTA to match the Go benchmark's oracle.
3. Add JS/TS binding, function-token propagation, and object/property modeling
   to match the Jelly oracle.
4. Add adaptation only as validated model facts consumed by the same solver.

That path raises benchmark scores while also building the core representation
needed for real repo-local policy rules.
