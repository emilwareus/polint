# Repository Index

Reference repositories were cloned under `research/program-slicing-evidence/repos/`.
The clone directory is ignored by git. Commit ids below are the inspected local
snapshots.

## Cloned Repositories

| Repository | URL | Local path | Snapshot | Why inspected |
|---|---|---|---:|---|
| github/codeql | <https://github.com/github/codeql> | `repos/codeql` | `a84332ac150e` | Path-query graph model, data-flow path nodes, hidden nodes, summary subpaths. |
| wala/WALA | <https://github.com/wala/WALA> | `repos/WALA` | `bd2e8d17254` | Mature JVM SDG/PDG slicer, context-sensitive tabulation, data/control dependence knobs. |
| joernio/joern | <https://github.com/joernio/joern> | `repos/joern` | `da77724000f` | CPG-based slicing, data-flow paths, call-site stack path validation, semantics overlays. |
| semgrep/semgrep | <https://github.com/semgrep/semgrep> | `repos/semgrep` | `2940ecd09a1` | Practical taint trace output, shape/taint signatures, SARIF/text/JSON trace rendering. |
| Frama-C/Frama-C-snapshot | <https://github.com/Frama-C/Frama-C-snapshot> | `repos/frama-c` | `639a364773` | C slicing plugin, PDG marks, selection propagation, slicing CLI modes. |
| mistupv/JavaSlicer | <https://github.com/mistupv/JavaSlicer> | `repos/JavaSlicer` | `8ecbf79b6f` | Java SDG slicer architecture, ICFG call/return expansion, source-to-slice workflow. |

## WALA

Key source paths:

- `core/src/main/java/com/ibm/wala/ipa/slicer/Slicer.java`
- `core/src/main/java/com/ibm/wala/ipa/slicer/SliceFunctions.java`
- `core/src/main/java/com/ibm/wala/ipa/slicer/SDG.java`
- `core/src/main/java/com/ibm/wala/ipa/slicer/PDG.java`
- `core/src/main/java/com/ibm/wala/ipa/slicer/ThinSlicer.java`
- `core/src/main/java/com/ibm/wala/dataflow/IFDS/TabulationSolver.java`

Relevant algorithms/domains:

- Program Dependence Graph and System Dependence Graph construction.
- Backward and forward slicing.
- Partially balanced tabulation for interprocedural slicing.
- Data-dependence and control-dependence options.
- Thin slicing as a cheap human-oriented mode.

Source observations:

- `Slicer.java` exposes `DataDependenceOptions` such as `FULL`,
  `NO_BASE_PTRS`, `NO_BASE_NO_HEAP`, `NO_HEAP`, `NO_EXCEPTIONS`, and `NONE`.
  This is exactly the kind of precision/cost ladder polint needs.
- `ControlDependenceOptions` supports `FULL`, `NONE`,
  `NO_EXCEPTIONAL_EDGES`, `NO_INTERPROC_EDGES`, and
  `NO_INTERPROC_NO_EXCEPTION`.
- Slice computation builds an `SDG`, wraps it in a supergraph, creates a
  `SliceProblem`, and solves with `PartiallyBalancedTabulationSolver`.
- `SDG.java` builds from call graph, pointer analysis, mod/ref, data-dependence
  options, and control-dependence options. Heap dependence can be disabled by
  option.
- `PDG.java` adds control-dependence edges from a control-dependence graph and
  data-dependence edges from def-use/mod-ref facts.
- `ThinSlicer.java` is a low-cost backward-only slicer that uses no heap
  dependence and no control dependence.

Line anchors inspected:

- `Slicer.java`: data-dependence options around lines 49-64; control options
  around 124-143; public slice entry points around 177-200; main slice driver
  around 240-273; `SliceProblem` and tabulation setup around 296-375.
- `SliceFunctions.java`: reachability-oriented flow functions around 19-87.
- `SDG.java`: construction inputs around 107-143; interprocedural predecessors
  around 300-375.
- `PDG.java`: control-dependence and method-entry edges around 241-329.
- `ThinSlicer.java`: thin-slicer entry around 20-38.
- `TabulationSolver.java`: solver lineage/options around 45-54; summary-edge
  handling around 320-338, 605-640, and 934-945.

Caveats:

- WALA is JVM-centric and depends on mature class hierarchy, call graph,
  pointer analysis, mod/ref, and IR infrastructure. polint should copy the graph
  architecture and knobs, not the implementation or JVM assumptions.

## CodeQL

Key source paths:

- `shared/dataflow/codeql/dataflow/DataFlow.qll`
- `shared/dataflow/codeql/dataflow/internal/DataFlowImpl.qll`
- `javascript/ql/lib/semmle/javascript/dataflow/Configuration.qll`
- `javascript/ql/lib/semmle/javascript/dataflow/FlowSteps.qll`

Relevant algorithms/domains:

- Path-query APIs over data-flow and taint graphs.
- `PathNode` model for source, sink, and intermediate path nodes.
- Hidden nodes and path graph compression.
- `subpaths` for summary expansion.
- Call/return level tracking in JavaScript data flow.

Source observations:

- `DataFlow.qll` defines a path graph signature with `edges`, `nodes`, and
  `subpaths`. The subpath concept is important: a rendered edge can stand for a
  compressed summary that can be expanded.
- `PathNode` exists only when it is reachable from a source and can reach a
  sink. This avoids materializing irrelevant path graph regions.
- The implementation emits edge provenance/labels for path graph output.
- JavaScript flow summaries carry unmatched call/return information to avoid
  invalid call/return concatenation.
- Configuration includes hidden nodes, access-path limits, field-flow branch
  limits, and call-context options.

Line anchors inspected:

- `DataFlow.qll`: `neverSkipInPathGraph` around 336-340; configuration knobs
  around 430-479; selected source/sink locations around 498-516; `PathGraphSig`
  around 680-694; `PathNode` and `flowPath` around 710-723.
- `DataFlowImpl.qll`: `flowPath` and path graph emission around 2554-2597;
  `flow` delegation around 3609-3612.
- `Configuration.qll`: interprocedural path node concepts around 31-64; path
  node kinds and path summary labels around 1661-1728; hidden node path graph
  successors around 1767-1875.
- `FlowSteps.qll`: path summary call/return append logic around 548-624.

Caveats:

- CodeQL is a database/Datalog engine. polint should adapt the path graph and
  explainability design, not import a database dependency or public QL model.

## Joern

Key source paths:

- `dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/slicing/DataFlowSlicing.scala`
- `dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/package.scala`
- `dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/Engine.scala`
- `dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/language/ExtendedCfgNode.scala`
- `dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/passes/reachingdef/DataFlowSolver.scala`
- `dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/passes/reachingdef/DdgGenerator.scala`
- `dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/semanticsloader/Semantics.scala`
- `dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/semanticsloader/DefaultSemantics.scala`

Relevant algorithms/domains:

- Code Property Graph data-flow slicing.
- Backward path search from sinks to sources.
- DDG generation from reaching definitions.
- Call-site stack tracking and task fingerprints.
- Custom semantics for calls/operators/libraries.

Source observations:

- Data-flow slicing creates tasks for sinks, walks incoming DDG edges with a
  configured max depth, and collects `REACHING_DEF` edges among slice nodes.
- `TaskFingerprint(sink, callSiteStack, callDepth)` is the practical cache key
  and path-validity unit.
- `callSiteStack` prevents impossible paths across mismatched call sites.
- Joern adds broad DDG edges around calls and filters invalid behavior at query
  time through semantics.
- `Semantics.after` composes semantics layers, allowing a later provider to
  override or extend earlier semantics.

Line anchors inspected:

- `DataFlowSlicing.scala`: slice task construction and `ddgIn` traversal around
  19-58.
- `queryengine/package.scala`: task fingerprints, path elements, and path
  metadata around 7-104.
- `Engine.scala`: backward solving around 40-54; parallel task solving around
  67-94; DDG path expansion around 199-240; incoming DDG edge filtering around
  252-269; semantic fallback checks around 290-306.
- `ExtendedCfgNode.scala`: `reachableBy` and `reachableByFlows` around 31-63.
- `DataFlowSolver.scala`: forward and backward worklists around 7-73.
- `DdgGenerator.scala`: DDG edge creation around 14-30; call-site edges around
  81-105; usage analysis around 253-330.
- `Semantics.scala`: composition and nil/no-cross-taint semantics around 8-70.
- `DefaultSemantics.scala`: default operator/Java flows around 14-36 and
  118-145.

Caveats:

- Joern's CPG model is powerful, but polint's current architecture is a typed
  fact and semantic operation store. polint should adapt the task fingerprint,
  semantics-overlay, and visible-path ideas.

## Semgrep

Key source paths:

- `src/tainting/Shape_and_sig.ml`
- `src/tainting/Taint.ml`
- `src/reporting/Core_json_output.ml`
- `src/reporting/Core_text_output.ml`
- `cli/src/semgrep/formatter/sarif.py`

Relevant algorithms/domains:

- Practical taint shape and signature representation.
- Taint traces to sources, intermediate variables, and sinks.
- JSON/text/SARIF-oriented data-flow trace rendering.

Source observations:

- Shapes approximate objects/data structures and track taint through fields and
  indexes. This is the lightweight alternative to complete heap modeling.
- `taint_to_sink_item` carries sink trace data in the analysis result.
- Internal comments warn that path-insensitive analysis should capture all
  potential paths, but output formats may force a single trace.
- JSON conversion currently picks the first trace when multiple traces are
  available because of external format limitations.
- Text output shows source, intermediate variables, and how taint reaches the
  sink.

Line anchors inspected:

- `Shape_and_sig.ml`: shape representation around 67-100; sink and function
  result signatures around 274-360.
- `Taint.ml`: tainted tokens and call trace around 67-81; comparison/perf
  warning around 25-60.
- `Core_json_output.ml`: data-flow trace conversion around 276-312; attaching
  trace to output around 420-464.
- `Core_text_output.ml`: source/intermediate/sink text trace around 70-109.
- `sarif.py`: `show_dataflow_traces` integration around 58-63.

Caveats:

- Semgrep's engine optimizes for broad practical rule writing. polint should
  copy the idea of helpful traces, but keep a richer internal evidence model so
  it does not collapse multiple alternatives too early.

## Frama-C

Key source paths:

- `src/plugins/slicing/slicingActions.mli`
- `src/plugins/slicing/slicingParameters.ml`
- `src/plugins/slicing/slicingCmds.ml`
- `src/plugins/pdg/marks.mli`

Relevant algorithms/domains:

- PDG-backed slicing for C.
- Selection marks and propagation modes.
- CLI-driven criteria for calls, returns, assertions, read/write zones, and
  dependency kind.
- Interprocedural propagation of marks between callers and callees.

Source observations:

- Selection modes distinguish direct nodes, address dependencies, data
  dependencies, control dependencies, and node-plus-deps selection.
- Slicing parameters expose calls, returns, assertions, loop invariants,
  read/write/value zones, caller propagation, slicing level, and undefined
  functions.
- The command path propagates selections through the project call graph in
  reverse topological order.
- PDG marks translate from callee inputs to callers and from call outputs to
  called functions.

Line anchors inspected:

- `slicingActions.mli`: selection modes and dependency propagation kinds around
  27-48.
- `slicingParameters.ml`: CLI and slicing-level options around 31-172.
- `slicingCmds.ml`: callgraph propagation around 142-150; statement/data
  selections around 155-208; transparent control selection around 283-299.
- `marks.mli`: caller/callee mark translation around 25-69.

Caveats:

- Frama-C can emit transformed C slices. That is valuable but not the right
  first target for polint. polint's first target should be diagnostic evidence
  and queryable graph regions, not code generation.

## JavaSlicer

Key source paths:

- `README.md`
- `iacfg/src/main/java/es/upv/mist/slicing/graphs/threaded/TSDG.java`
- `iacfg/src/main/java/es/upv/mist/slicing/graphs/icfg/ICFG.java`
- `iacfg/src/main/java/es/upv/mist/slicing/graphs/scrs/AbstractSCRAlgorithm.java`

Relevant algorithms/domains:

- Java SDG construction.
- Interprocedural CFG construction.
- Call/return/actual-in/actual-out expansion.
- SCC/condensation graph ordering.

Source observations:

- The README describes the workflow: configure JavaParser, parse compilation
  units, create SDG, define slicing criterion, obtain slice, convert/dump.
- Source dependencies improve slice quality; missing libraries degrade results.
- `ICFG.java` expands calls into actual-in nodes, a call node, return node, and
  actual-out nodes. This mirrors the parameter-in/out structure needed by SDG
  slicing.
- SCC and condensation logic is used to organize interprocedural graph
  computation.

Line anchors inspected:

- `TSDG.java`: threaded SDG construction around 13-50.
- `ICFG.java`: graph build and SCC metadata around 41-120; interprocedural
  definition/usage finders around 564-568; actual-in/call/return/actual-out
  expansion around 646-720.
- `AbstractSCRAlgorithm.java`: SCC/condensation wrapper around 10-63.

Caveats:

- The project is smaller and less mature than WALA/CodeQL/Joern, but useful as
  a readable Java SDG architecture reference.
