# Final Report: Type, Value, Points-To, And Alias Analysis

## Executive Decision

Build a native layered analysis stack, not a single "full alias analysis" pass.

```text
semantic index + module graph + CFG
  -> language-owned type facts
  -> place/access-path facts
  -> local value facts
  -> local flow and narrowing facts
  -> summary facts
  -> bounded points-to constraints
  -> alias query service
  -> call graph, data flow, effects, slicing, and rule SDK views
```

The state of the art is not one universal algorithm. Mature tools combine several layers:

- type facts and narrowing for most high-value precision;
- allocation/value tokens for objects, functions, classes, modules, literals, and closures;
- places/access paths to name memory-like locations;
- local flow-sensitive facts where the control-flow cost is manageable;
- summaries for interprocedural scaling;
- flow-insensitive or sparse flow-sensitive points-to only where the query needs it;
- alias answers derived from points-to, ownership, field sensitivity, and language semantics;
- model and extension mechanisms for dynamic frameworks and project-specific APIs.

For polint, the most important product insight is that an AI agent can write repo-local Rust extension code. That changes the implementation target. We do not need a closed black-box analyzer that guesses every framework convention. We need a high-capability fact framework with sane defaults, honest unknowns, and validated extension hooks that can add precision for the specific repository.

## What "Full Call Graphs" Means Here

Yes, this research enables fuller call graphs, but call graphs should remain a separate fact family that consumes these layers.

Direct and syntactic calls come from the semantic index. Method, interface, closure, callback, virtual, dynamic-property, reflection, dependency-injected, and framework-dispatched calls need type/value/points-to facts plus extension-provided summaries. In other words:

```text
call graph precision = semantic resolution
  + type facts
  + value/function-object facts
  + points-to/capture facts
  + framework/entrypoint models
  + extension-provided call targets
```

A call graph that does not expose which edges are exact, heuristic, extension-provided, or unknown will be dangerous for rules. The recommended design makes call edges provenance-labeled consumers of the underlying facts.

## Major Findings

### 0. Official Language Toolchains Are Allowed Integration Points

The "native implementation" goal should not be interpreted as "never call or reuse official language functionality." It means polint should own the normalized fact model, scheduling, provenance, cache keys, SDK views, validation, and extension merges.

Official language tooling is allowed when it is the compatibility authority for the language:

- Go: `go list`, `go env`, `go/types`, `go/packages`, `go/ssa`, and official callgraph behavior can be used as provider inputs, compatibility checks, or even delegated semantic phases when that is the most correct path.
- JVM/Java: `javac`, JDK classfile/module metadata, bytecode attributes, and JVM resolution semantics are valid language-native inputs.
- TypeScript: the TypeScript compiler can be used as a semantic oracle for type/narrowing behavior, even if polint's first TS/JS frontend remains Oxc-based.
- Python: official import metadata, packaging metadata, and runtime/library specifications are valid inputs, while third-party type checkers remain references.

The line is not "no external programs." The line is: do not make polint's core engine depend on arbitrary OSS analyzers whose APIs, semantics, release cadence, or precision model we do not control. If an official toolchain provides the language truth, use it pragmatically and wrap its output into polint facts.

### 1. Types Carry More Precision Than Traditional Alias Analysis In Dynamic Languages

For Python and TS/JS, the highest return comes from type and flow-narrowing facts before deep points-to:

- Ty uses intersection types, top/bottom materializations, and type-based reachability/narrowing.
- Pyright and TypeScript use flow graphs to compute reference-specific narrowed types.
- Pyrefly uses module-centric binding graphs, flow types, `Var` placeholders, and recursive solving.
- CodeQL JS/Python relies heavily on type tracking, API models, and extensible flow steps rather than trying to prove exact heap aliasing everywhere.

The implication: polint should implement `TypeFacts`, `NarrowingFacts`, and `ValueFacts` before global points-to. This will already resolve many call targets, guards, literal switches, null checks, source/sink classifications, and framework conventions.

### 2. Alias Analysis Should Be A Query Layer, Not The Primary Storage Layer

LLVM's alias-analysis infrastructure is a provider stack. It asks multiple analyses for `NoAlias`, `MayAlias`, `PartialAlias`, or `MustAlias` and stays conservative when unsure. SVF builds richer pointer/value-flow graphs, but even there alias answers are derived from memory regions and points-to/value-flow facts.

For polint, store points-to and value-flow facts. Derive alias answers on demand:

```python
def alias(a, b):
    if same_stable_place(a, b):
        return MustAlias(reason="same place")
    if ownership_or_scope_proves_disjoint(a, b):
        return NoAlias(reason="disjoint locals/allocations")
    pts_a = points_to(a)
    pts_b = points_to(b)
    if pts_a.is_unknown or pts_b.is_unknown:
        return Unknown(reason="missing points-to")
    if disjoint(pts_a, pts_b):
        return NoAlias(reason="disjoint points-to sets")
    if pts_a.singleton_equal(pts_b) and mutation_model_supports_identity(a, b):
        return MustAlias(reason="same singleton object")
    return MayAlias(reason="overlapping points-to")
```

This keeps the rule-facing API honest and avoids pretending that a single global alias graph is exact.

### 3. Andersen Is The Right Baseline Points-To Solver, But Only As A Bounded Internal Provider

Classic inclusion-based Andersen constraints are the right first native points-to substrate:

```text
x = &o   => o in Pt(x)
x = y    => Pt(y) subset Pt(x)
x = *y   => for o in Pt(y): Pt(o) subset Pt(x)
*x = y   => for o in Pt(x): Pt(y) subset Pt(o)
x = y.f  => for o in Pt(y): Pt(o.f) subset Pt(x)
x.f = y  => for o in Pt(x): Pt(y) subset Pt(o.f)
```

But it should not run as one mandatory whole-repo pass. Use it as a requestable provider with budgets, SCC/delta propagation, field sensitivity, type filters, summary boundaries, and explicit `Unknown` on timeout or unsupported constructs.

Steensgaard/unification is useful as an emergency coarse mode or pre-pass, but too imprecise as the main engine for a product whose value is tailored high accuracy.

### 4. Sparse Flow Sensitivity Comes Later, But The Fact Model Must Allow It Now

LLVM MemorySSA and SVF show the scalable pattern:

```text
local CFG/SSA
  -> memory defs/uses/phis
  -> alias/points-to-aware clobber/value-flow queries
  -> sparse value-flow graph
```

Dense flow-sensitive points-to across every program point is too expensive. polint should first implement:

- local flow-sensitive value/narrowing facts;
- flow-insensitive interprocedural points-to summaries;
- sparse memory/value-flow hooks in the IDs and cache keys;
- later, a MemorySSA/SVFG-like refinement provider for queries that need path precision.

### 5. Java/JVM Has The Clearest Algorithm Ladder

JVM tools converge on a tiered ladder:

```text
CHA -> RTA -> VTA/type propagation -> Andersen/Spark/Doop/WALA -> selective context sensitivity
```

Doop, WALA, Soot/Spark, SootUp/Qilin, OPAL, and Checker Framework all show the same lesson: separate classpath/type facts, IR/CFG facts, call graph construction, points-to relations, and framework/reflection models. Reflection, class loaders, incomplete classpaths, Android/JVM lifecycle, and generated code require model facts and precision labels.

For polint, this ladder should become the cross-language pattern even before Java support lands.

### 6. Go Needs A Native Long-Term Path, But The Official Go Toolchain Is The Best Oracle

The Go reference stack is `go/types`, `go/packages`, `go/ssa`, and `x/tools/go/callgraph/{static,cha,rta,vta}`. The current `golang.org/x/tools` snapshot inspected here includes static, CHA, RTA, and VTA call graph packages, but no current `go/pointer` package under `x/tools/go`. Older Andersen-style Go pointer analysis exists historically, but this research should not describe it as current state of `x/tools`.

For a full native polint implementation, Go tools are allowed because they are the official language authority. The key is not to expose Go tool internals as polint's product model. Use them through a controlled provider boundary:

- use `go list`, `go/packages`, and `go/types` where exact Go lifecycle/type compatibility is worth delegating;
- normalize every output into polint-owned type/place/value/call facts;
- implement native fact layers and solvers around those facts;
- validate native behavior against `go/types`/`go/ssa`/VTA fixtures;
- avoid depending on non-official Go analysis libraries as the core implementation.

### 7. The Extension Surface Is Not Optional

The most capable engine will not be the one with the largest built-in heuristic set. It will be the one whose extension points let agents remove uncertainty in a controlled way.

Extension providers should be able to add:

- type hints and refined type facts;
- framework allocation tokens;
- call-target facts;
- function and method summaries;
- object/field/property model facts;
- points-to constraints;
- no-alias/disjointness facts with validation;
- dynamic API models;
- source/sink/sanitizer/barrier summaries;
- dependency-injection and route/lifecycle wiring.

These must not be loose config strings. They should be Rust-code providers that emit typed facts through validated sinks. Every extension fact needs provenance, cache participation, precision ceilings, conflict handling, and test fixtures.

## Recommended First Vertical Slice

Do not start with global alias analysis. Start with the minimum substrate that future precision can build on:

1. `PlaceFact` and `AccessPathFact`.
2. `TypeFact` and `NarrowedTypeFact`.
3. `ValueFact` and `AllocationTokenFact`.
4. `LocalFlowFact` over the existing CFG research model.
5. `SummaryFact` for functions/methods/modules.
6. `PointsToConstraintFact` and a bounded Andersen-style solver.
7. `AliasQuery` internal service returning `NoAlias`, `MayAlias`, `MustAlias`, or `Unknown`.
8. Extension sinks for type/value/summary/points-to/call-target facts.
9. Evaluation fixtures against Ty/Pyright/Pyrefly/TypeScript/Go VTA/Doop/SVF-style cases.

## Final Recommendation

Implement the analysis as a native Rust fact engine with pluggable providers and a precision ladder:

```text
Tier 0: syntax + semantic index + CFG
Tier 1: declared/resolved type facts
Tier 2: local places, values, allocation tokens, and narrowing
Tier 3: summaries and call-target/value-flow models
Tier 4: bounded flow-insensitive points-to
Tier 5: selective context sensitivity
Tier 6: sparse flow-sensitive refinement
```

This is the strongest path for polint's goal: maximum long-run capability, high tailored scan accuracy, and an engine that AI agents can extend with repo-specific Rust code instead of asking a universal analyzer to guess everything.
