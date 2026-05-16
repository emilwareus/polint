# OSS Implementation Comparison

This file applies the standard vocabulary from `../STANDARD.md` to the implementations cloned under `../repos/`.

## CodeQL

**What it builds:** language-specific data-flow nodes, local flow, taint flow, global flow configurations, summaries, and path explanations.

**Key code inspected:**

- `repos/codeql/shared/dataflow/codeql/dataflow/DataFlow.qll`
- `repos/codeql/shared/dataflow/codeql/dataflow/TaintTracking.qll`
- `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/*`
- `repos/codeql/go/ql/lib/semmle/go/dataflow/*`
- `repos/codeql/python/ql/lib/semmle/python/dataflow/new/*`

**Algorithm shape:** declarative graph reachability over extracted database facts. Users provide a configuration with sources, sinks, barriers, and optional additional flow steps. Local data flow is cheaper and usually more precise. Global flow is source/sink scoped instead of "compute every possible path in the whole program."

```python
class FlowConfig:
    def is_source(node): ...
    def is_sink(node): ...
    def is_barrier(node): return False
    def additional_step(a, b): return False

def global_flow(config, graph):
    worklist = deque(config.sources())
    seen = set(worklist)

    while worklist:
        n = worklist.popleft()
        if config.is_sink(n):
            yield path_to(n)

        for m in graph.successors(n) + config.additional_successors(n):
            if config.is_barrier(m):
                continue
            if m not in seen:
                seen.add(m)
                worklist.append(m)
```

**Polint lesson:** copy the API shape: typed nodes, local/global split, query-scoped global flow, barriers, additional steps, and path explanations. Do not copy the database/query-language dependency.

## Semgrep and OpenGrep

**What they build:** language AST to a common IL, intraprocedural data-flow, constant propagation, symbolic propagation, taint sources/sinks/sanitizers/propagators, and taint traces. OpenGrep exposes more active OSS-side taint internals.

**Key code inspected:**

- `repos/semgrep/src/il/IL.ml`
- `repos/semgrep/src/tainting/Shape_and_sig.ml`
- `repos/opengrep/src/analyzing/Dataflow_core.ml`
- `repos/opengrep/src/tainting/Dataflow_tainting.ml`
- `repos/opengrep/src/tainting/Taint_signature_extractor.ml`
- `repos/opengrep/src/tainting/Graph_from_AST.ml`

**Algorithm shape:** rule-scoped taint propagation over a common IL. It accepts false positives and false negatives as part of a lightweight, multi-language product. The docs explicitly call out no path sensitivity, no soundness guarantee, and limited pointer/shape modeling.

```python
def taint_rule(il_function, rule):
    graph = build_cfg_and_flow(il_function)
    tainted = sources_matching(rule.pattern_sources, graph)

    for edge in graph.forward_edges():
        if edge.src in tainted and not rule.sanitizes(edge):
            tainted.add(edge.dst)
            if rule.is_sink(edge.dst):
                emit_trace(edge.dst)
```

**Polint lesson:** pattern-authored source/sink/sanitizer rules are ideal for AI agents. The warning is that the engine must expose precision/status labels so heuristic lightweight matches are not confused with semantic proof.

## Joern

**What it builds:** a code property graph with AST, CFG, call graph, type nodes, reaching definitions, data-dependence overlays, slicing, and `reachableBy` queries.

**Key code inspected:**

- `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/Engine.scala`
- `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/TaskCreator.scala`
- `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/TaskSolver.scala`
- `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/semanticsloader/DefaultSemantics.scala`
- `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/passes/reachingdef/*`

**Algorithm shape:** build a unified property graph, add overlays for data-flow facts, then answer reachability/path queries with semantics that model library/framework behavior.

```python
def reachable_by(sinks, sources, cpg, semantics):
    graph = cpg.overlay("dataflow")
    graph.add_edges(semantics.extra_flow_edges(cpg))

    for sink in sinks:
        for path in reverse_reachability(graph, sink, stop_at=sources):
            yield path
```

**Polint lesson:** a graph-shaped internal substrate is powerful, but the public API should remain typed SDK views. Joern also shows that semantics/model files become as important as the solver.

## Pysa

**What it builds:** Python taint summaries for sources, sinks, and TITO ("taint in taint out"), propagated to a global fixed point over the call dependency graph.

**Key code inspected:**

- `repos/pyre-check/source/interprocedural_analyses/taint/taintAnalysis.ml`
- `repos/pyre-check/source/interprocedural_analyses/taint/taintFixpoint.ml`
- `repos/pyre-check/source/interprocedural_analyses/taint/forwardAnalysis.ml`
- `repos/pyre-check/source/interprocedural_analyses/taint/backwardAnalysis.ml`
- `repos/pyre-check/source/interprocedural_analyses/taint/domains.ml`
- `repos/pyre-check/source/interprocedural_analyses/taint/model.ml`

**Algorithm shape:** infer function summaries, iterate callers when callee summaries change, and use explicit models for sources/sinks/sanitizers.

```python
summaries = {fn: empty_summary(fn) for fn in functions}
worklist = all_functions_in_dependency_order()

while worklist:
    fn = worklist.pop()
    old = summaries[fn]
    new = analyze_with_callee_summaries(fn, summaries)

    if new != old:
        summaries[fn] = widen(old, new)
        worklist.extend(callers(fn))
```

**Polint lesson:** this is the best first global architecture for polint. It is native, cacheable, language-extensible, and depends naturally on call graph facts.

## FlowDroid and Heros

**What they build:** FlowDroid applies a high-precision Android taint analysis on Soot/Jimple. Heros provides the generic IFDS/IDE solver machinery used by Soot-family analyses.

**Key code inspected:**

- `repos/heros/src/heros/IFDSTabulationProblem.java`
- `repos/heros/src/heros/IDETabulationProblem.java`
- `repos/heros/src/heros/FlowFunctions.java`
- `repos/heros/src/heros/solver/IFDSSolver.java`
- `repos/FlowDroid/soot-infoflow/src/soot/jimple/infoflow/problems/InfoflowProblem.java`
- `repos/FlowDroid/soot-infoflow/src/soot/jimple/infoflow/data/AccessPath.java`
- `repos/FlowDroid/soot-infoflow/src/soot/jimple/infoflow/data/Abstraction.java`

**Algorithm shape:** IFDS/IDE over an interprocedural CFG, finite facts, jump functions, access paths, alias problems, and carefully modeled lifecycle/callback entrypoints.

```python
def tabulate(icfg, seeds, flow_functions):
    reached = set()
    worklist = seed_path_edges(seeds)

    while worklist:
        edge = worklist.pop()

        for kind, successor in icfg.successors(edge.node):
            facts = flow_functions[kind](edge.fact, edge.node, successor)
            for fact in facts:
                add_path_edge(edge.start, successor, fact)
```

**Polint lesson:** IFDS/IDE is the correct advanced engine for finite flow problems, but it should come after CFG, call graph, local flow, summaries, and access paths are stable.

## WALA

**What it builds:** call graphs, pointer analysis, slicing, IFDS, Java and JS analyses.

**Key code inspected:**

- `repos/WALA/core/src/main/java/com/ibm/wala/dataflow/IFDS/*`
- `repos/WALA/core/src/main/java/com/ibm/wala/ipa/slicer/*`
- `repos/WALA/core/src/main/java/com/ibm/wala/ipa/callgraph/propagation/*`

**Algorithm shape:** framework-oriented analysis stack. The data-flow pieces rely on call graph and pointer analysis quality.

**Polint lesson:** call graph, points-to, and data-flow cannot be independent feature silos. They must share symbol, type, package, and module facts.

## Checker Framework

**What it builds:** intraprocedural abstract interpretation over Java CFGs using transfer functions, stores, abstract values, and joins.

**Key code inspected:**

- `repos/checker-framework/dataflow/src/main/java/org/checkerframework/dataflow/analysis/*`
- `repos/checker-framework/dataflow/src/main/java/org/checkerframework/dataflow/cfg/*`
- `repos/checker-framework/dataflow/src/main/java/org/checkerframework/dataflow/cfg/node/*`

**Algorithm shape:** classic local data-flow framework. Each checker supplies a domain and transfer functions.

```python
class Transfer:
    def visit_assignment(state, lhs, rhs):
        return state.set(lhs, abstract_eval(rhs, state))

    def visit_condition(state, cond):
        return assume_true(state, cond), assume_false(state, cond)
```

**Polint lesson:** this is the clean model for non-taint local facts such as nilness, definite assignment, constant values, type refinements, and guard-sensitive facts.

## Doop and Souffle

**What they build:** relation-based points-to/data-flow analysis using Datalog rules, semi-naive evaluation, relation scheduling, SCC planning, and magic-set-style optimization.

**Key code inspected:**

- `repos/doop/souffle-logic/main/main.dl`
- `repos/doop/souffle-logic/addons/information-flow/*`
- `repos/souffle/src/ast/analysis/SCCGraph.*`
- `repos/souffle/src/ast/transform/MagicSet.*`
- `repos/souffle/src/ast2ram/seminaive/*`

**Algorithm shape:** facts plus rules to a fixed point.

```python
while changed:
    for relation_group in strongly_connected_rule_groups:
        delta = evaluate_rules_incrementally(relation_group)
        changed |= bool(delta)
```

**Polint lesson:** use the Datalog architecture idea internally: typed relation facts, dependency SCCs, semi-naive fixed points. Do not require users to write Datalog for v1.

## TypeScript Compiler

**What it builds:** AST-attached flow nodes used by the checker for demand-driven narrowing.

**Key code inspected:**

- `repos/TypeScript/src/compiler/types.ts`
- `repos/TypeScript/src/compiler/binder.ts`
- `repos/TypeScript/src/compiler/checker.ts`

**Algorithm shape:** binder creates flow nodes around assignments, conditions, calls, mutations, and branch labels. The checker asks for the flow type of a reference at a use site.

```python
def type_at_reference(reference):
    flow = reference.flow_node
    typ = declared_type(reference.symbol)

    while flow:
        typ = narrow_or_widen(typ, flow)
        flow = flow.antecedent

    return typ
```

**Polint lesson:** demand-driven APIs are essential for editor/agent usage. Do not force all expensive global analyses to run before simple rules can ask local questions.

## Go-Specific OSS

**Go taint** uses `golang.org/x/tools/go/ssa` and call graph utilities to trace SSA values from sinks back to sources. It is small and useful as a concrete Go reference, but it is not a complete multi-language architecture.

**gosec** provides AST/SSA security checks. It is useful for rule patterns and Go package lifecycle expectations.

**NilAway** is valuable for package-level summary ergonomics and precise diagnostic traces in Go.

**Polint lesson:** native Go data-flow should eventually use semantic package information, method sets, interfaces, and SSA-like lowering. The first polint version can start syntax-based but must mark unsupported semantic gaps honestly.

