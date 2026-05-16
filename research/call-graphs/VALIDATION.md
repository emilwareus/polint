# Validation Report

Date: 2026-05-15

## Scope

This pass validates the research package in `research/call-graphs/` for:

- repository remotes and commit hashes;
- local source-path references;
- downloaded paper/document integrity and titles;
- the strongest algorithm and implementation claims in the language notes;
- residual uncertainty that should stay explicit.

## Summary

The core findings are supported by the inspected source code and downloaded papers. I found no broken concrete `repos/...` references in the markdown notes. I tightened `PAPER-INDEX.md` with validated titles and page counts because the first index was too terse for later auditing.

The main conclusion remains valid: polint should implement layered `CallSite` and `CallEdge` facts with explicit algorithm and uncertainty metadata, not a single claimed-exact multi-language call graph.

## Repository Validation

Local clone remotes and commits were checked with `git remote get-url origin` and `git rev-parse --short=12 HEAD`.

| Repo | Commit | Remote |
|---|---:|---|
| `codeql` | `a84332ac150e` | `https://github.com/github/codeql.git` |
| `doop` | `3cb3ae54e7d9` | `https://github.com/plast-lab/doop.git` |
| `go-callvis` | `67a26605e208` | `https://github.com/ondrajz/go-callvis.git` |
| `golang-tools` | `a3954b5c7496` | `https://go.googlesource.com/tools` |
| `jarvis` | `8871b86494a1` | `https://github.com/pythonJaRvis/pythonJaRvis.github.io.git` |
| `jelly` | `b799ed4f0d68` | `https://github.com/cs-au-dk/jelly.git` |
| `opal` | `8870593d34f3` | `https://github.com/opalj/opal.git` |
| `pyan` | `3663440089b1` | `https://github.com/Technologicat/pyan.git` |
| `pycg` | `8d5dc4083780` | `https://github.com/vitsalis/PyCG.git` |
| `pyre-check` | `34af3721bc04` | `https://github.com/facebook/pyre-check.git` |
| `soot` | `135d1ead8c89` | `https://github.com/soot-oss/soot.git` |
| `sootup` | `c148177e9a26` | `https://github.com/soot-oss/SootUp.git` |
| `tai-e` | `6b6169c178fb` | `https://github.com/pascal-lab/Tai-e.git` |
| `tajs` | `3bdf55a411d6` | `https://github.com/cs-au-dk/TAJS.git` |
| `wala` | `bd2e8d172542` | `https://github.com/wala/WALA.git` |

All concrete markdown references of the form `repos/...` resolve relative to `research/call-graphs/`.

## Paper Validation

`pdfinfo` and first-page `pdftotext` were used for PDFs. The downloaded files are valid PDF or HTML documents. Important titles verified:

- `pycg-icse-2021.pdf`: "PyCG: Practical Call Graph Generation in Python", 12 pages.
- `jarvis-python-call-graph-2023.pdf`: "JARVIS: Scalable and Precise Application-Centered Call Graph Construction for Python", 12 pages.
- `static-js-call-graphs-comparative-2024.pdf`: "Static JavaScript Call Graphs: a Comparative Study", 10 pages.
- `java-call-graph-unsoundness-2026.pdf`: "Detecting Call Graph Unsoundness without Ground Truth", 21 pages.
- `opal-unimocg-issta-2024.pdf`: "Unimocg: Modular Call-Graph Algorithms for Consistent Handling of Language Features", 12 pages.
- `opal-totalrecall-issta-2024.pdf`: "Total Recall? How Good Are Static Call Graphs Really?", 12 pages.
- `sootup-tacas-2024.pdf`: "SootUp: A Redesign of the Soot Static Analysis Framework", 18 pages.
- `spark-points-to-2003.pdf`: "Scaling Java Points-To Analysis using SPARK", 16 pages.
- `soot-framework.pdf`: "The Soot framework for Java program analysis: a retrospective", 8 pages.

## Research Metrics Extracted

The improved reports use the following metrics extracted with `pdftotext` from the downloaded papers:

- PyCG: approximately 99.2% precision, 69.9% recall, and 0.38 seconds per 1 KLOC reported in `pycg-icse-2021.pdf`.
- JARVIS: average 8.16 seconds for application-centered whole-program call graph generation, at least 67% faster runtime, 84% higher precision, and at least 20% higher recall over PyCG in the reported setting in `jarvis-python-call-graph-2023.pdf`.
- Static JavaScript call graph comparison: ACG around 99% precision / 91% recall, TAJS around 98% / 71%, Closure around 81% / 89%, WALA around 87% / 49%, ACG+TAJS around 98% / 99%, and all tools combined around 74% precision at full union recall in `static-js-call-graphs-comparative-2024.pdf`.
- Unimocg: Soot/WALA feature-test support between 41% and 53%; Unimocg between 79% and 81% across tested algorithms in `opal-unimocg-issta-2024.pdf`.
- Java call graph unsoundness: low cross-framework similarity, expected partial-order violations, WALA object-sensitivity reshaping graphs to about 39.1% baseline similarity in one configuration family, and 600 second timeout for Doop object-sensitive comparisons in `java-call-graph-unsoundness-2026.pdf`.
- Total Recall: graph size and reachable-method counts are not reliable proxies for edge precision/recall in `opal-totalrecall-issta-2024.pdf`.

## Source-Claim Validation

### Go

Validated claims:

- Static call graph uses `StaticCallee`: `repos/golang-tools/go/callgraph/static/static.go:25`, `:38`.
- CHA has `CallGraph(prog *ssa.Program)` and still handles static callees directly: `repos/golang-tools/go/callgraph/cha/cha.go:36`, `:64`.
- RTA entrypoint is `Analyze(roots []*ssa.Function, buildCallGraph bool)`: `repos/golang-tools/go/callgraph/rta/rta.go:308`.
- VTA is experimental and builds a global type-propagation graph: `repos/golang-tools/go/callgraph/vta/vta.go:5-17`.
- VTA intersects type-propagation results with initial call graph callees: `repos/golang-tools/go/callgraph/vta/vta.go:113-134`.
- VTA SCC/trie propagation is in `propagate`: `repos/golang-tools/go/callgraph/vta/propagation.go:121-178`.
- `go-callvis` uses `packages.LoadAllSyntax`, `ssautil.AllPackages`, then static/CHA/RTA/VTA: `repos/go-callvis/analysis.go:106`, `:126`, `:136-166`.

Conclusion: the Go notes are source-backed.

### JavaScript / TypeScript

Validated claims:

- Jelly visits `CallExpression`, `OptionalCallExpression`, and `NewExpression` and forwards them to `Operations.callFunction`: `repos/jelly/src/analysis/astvisitor.ts:347-367`.
- Jelly registers call edges when a `FunctionToken` is bound: `repos/jelly/src/analysis/operations.ts:433-454`.
- Jelly stores both function-to-function and call-to-function edges: `repos/jelly/src/analysis/fragmentstate.ts:377-403`.
- CodeQL JS exposes `InvokeNode.getACallee()` and `getACallee(int imprecision)`: `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/Nodes.qll:193-217`.
- CodeQL JS exposes `isImprecise`, `isIncomplete`, and `isUncertain`: `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/Nodes.qll:228-261`.
- CodeQL JS call graph internals include type/value-flow, class/member references, bound functions, static resolved callees, and super constructor handling: `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/internal/CallGraphs.qll:23-187`.
- TAJS has a context-aware `CallGraph.addTarget` and function call modeling: `repos/tajs/src/dk/brics/tajs/solver/CallGraph.java:85-145`, `repos/tajs/src/dk/brics/tajs/analysis/FunctionCalls.java:45-132`.

Conclusion: the JS/TS notes are source-backed. The "best practical reference" ranking is an engineering judgment, not a measured benchmark claim.

### Java / JVM

Validated claims:

- SootUp CHA resolves calls in `ClassHierarchyAnalysisAlgorithm.resolveCall`: `repos/sootup/sootup.callgraph/src/main/java/sootup/callgraph/ClassHierarchyAnalysisAlgorithm.java:81`.
- SootUp RTA tracks `instantiatedClasses` and `ignoredCalls`: `repos/sootup/sootup.callgraph/src/main/java/sootup/callgraph/RapidTypeAnalysisAlgorithm.java:52-55`.
- SootUp RTA collects instantiated classes and includes ignored calls when a class becomes instantiated: `repos/sootup/sootup.callgraph/src/main/java/sootup/callgraph/RapidTypeAnalysisAlgorithm.java:107-116`, `:321-354`.
- Classic Soot call graph builder processes reachable methods and receiver points-to: `repos/soot/src/main/java/soot/jimple/toolkits/callgraph/CallGraphBuilder.java:119`, `:207`.
- Soot Spark entrypoint exists at `repos/soot/src/main/java/soot/jimple/spark/SparkTransformer.java:77`.
- Soot on-the-fly call graph updates on points-to changes: `repos/soot/src/main/java/soot/jimple/spark/solver/OnFlyCallGraph.java:168`.
- WALA CHA closure and target lookup are present: `repos/wala/core/src/main/java/com/ibm/wala/ipa/callgraph/cha/CHACallGraph.java:147-190`, `:342-346`.
- WALA has Zero/ZeroOne/NObj builder factories: `repos/wala/core/src/main/java/com/ibm/wala/ipa/callgraph/impl/Util.java:434`, `:514`, `:855`.
- Doop unifies normal, reflection, Tamiflex, proxy, and opaque call edges in `AnyCallGraphEdge`: `repos/doop/souffle-logic/main/full-call-graph.dl:3-22`.
- OPAL's `TypeIterator` explicitly provides type information to call-graph clients: `repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/TypeIterator.scala:92`.
- OPAL's `CallGraphAnalysis` resolves virtual calls using the configured `typeIterator`: `repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/CallGraphAnalysis.scala:55`, `:260-311`.
- Tai-e CHA and PTA-based call graph paths exist: `repos/tai-e/src/main/java/pascal/taie/analysis/graph/callgraph/CHABuilder.java:55`, `:150`; `repos/tai-e/src/main/java/pascal/taie/analysis/pta/core/solver/DefaultSolver.java:467-497`.

Conclusion: the Java notes are source-backed. The report should continue saying Java precision depends heavily on classpath/build setup.

### Python

Validated claims:

- PyCG has `CallGraphGenerator`, `has_converged`, `do_pass`, and `analyze`: `repos/pycg/pycg/pycg.py:37`, `:79`, `:128`, `:161`.
- PyCG call edge emission is in `CallGraphProcessor.visit_Call`: `repos/pycg/pycg/processing/cgprocessor.py:126`.
- JARVIS has `CallGraphGenerator.do_pass`, `analyze_localfunction`, and call-stack-driven `pushStack`: `repos/jarvis/Jarvis/tool/Jarvis/jarvis.py:101-143`; `repos/jarvis/Jarvis/tool/Jarvis/processing/extProcessor.py:105`, `:1525`.
- Pyan's main visitor is `CallGraphVisitor`, with `visit_Call`: `repos/pyan/pyan/analyzer.py:68`, `:1307`.
- CodeQL Python central resolution predicate is `resolveCall`: `repos/codeql/python/ql/lib/semmle/python/dataflow/new/internal/DataFlowDispatch.qll:1232`.
- Pyre/Pysa `CallCallees` stores normal, `__new__`, `__init__`, decorated, higher-order, shim, unresolved, and recognized-call fields: `repos/pyre-check/source/interprocedural/callGraph.ml:645-664`.
- Pyre/Pysa explicitly says the builder is taint-tuned and may be unsound elsewhere: `repos/pyre-check/source/interprocedural/callGraphBuilder.ml:8-12`.

Conclusion: the Python notes are source-backed. The recommendation to label Python call graphs heuristic is strongly supported.

## Corrections And Tightening Applied

- Added this validation report.
- Linked it from `README.md`.
- Replaced the loose paper index topic column with validated titles and page counts.
- Later product-path update: added agent-extensible call graph modeling, repo-model provenance, validation status, and default-vs-extended evaluation requirements. This is an architectural synthesis from the validated research plus polint's product direction, not a new paper claim.

No broken repository paths or materially false core claims were found in the current markdown notes.

## Residual Uncertainty

- "State of the art" is not a formal benchmark ranking here. It is an engineering synthesis from current OSS implementations, papers, and source inspection.
- The cloned repositories are snapshots from 2026-05-15. Upstream HEAD may change.
- Some research papers are arXiv/preprint-style PDFs rather than final proceedings PDFs. The report uses them for technical direction, not as normative standards.
- The recommendations intentionally favor polint's product constraints: repo-local rules, typed facts, performance, and honest diagnostics. A different product, such as an IDE or vulnerability scanner, might choose heavier defaults.

## 2026-05-16 Bootstrap Integration Validation

The call graph implementation path was revalidated against the new
implementation-bootstrap research.

Additional checked source points:

- Current polint stores only string call hints on `FunctionFact`: `crates/polint/src/core/mod.rs:142-153`.
- Current Go adapter extracts sorted/deduped call names from tree-sitter call expressions: `crates/polint/src/go/adapter.rs:431-444`, `:549-562`.
- Current TS/JS adapter recursively collects call names from Oxc expressions: `crates/polint/src/ts/adapter.rs:1181-1303`.
- Go static call graph follows `StaticCallee`: `repos/golang-tools/go/callgraph/static/static.go:16-40`.
- Go RTA requires explicit roots and fixed-point processing: `repos/golang-tools/go/callgraph/rta/rta.go:300-354`.
- Go VTA is explicitly experimental and uses a global type-propagation graph: `repos/golang-tools/go/callgraph/vta/vta.go:5-55`.
- Jelly registers JS call edges when a function token binds: `repos/jelly/src/analysis/operations.ts:433-454`.
- CodeQL JS exposes potential callees plus imprecision/incompleteness predicates: `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/Nodes.qll:193-261`.
- Pyre/Pysa stores decorated, higher-order, shim, unresolved, and recognized-call fields separately: `repos/pyre-check/source/interprocedural/callGraph.ml:645-664`.
- OPAL separates type producers from call graph clients through `TypeIterator`: `repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/TypeIterator.scala:82-132`.
- OPAL records incomplete call sites for unresolved invokedynamic: `repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/CallGraphAnalysis.scala:267-289`.

Validation result:

- The old recommendation to implement public `Calls<'_>` / `CallGraph<'_>` as
  the next task was too early after the bootstrap research.
- The revised recommendation is internally consistent with the semantic
  bootstrap: derive call sites from MIR and places, emit direct targets and
  unresolved facts, feed direct summaries, validate extension sinks, and promote
  SDK views only after gates.
