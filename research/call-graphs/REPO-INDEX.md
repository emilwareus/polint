# Repository Index

Third-party code was cloned under `research/call-graphs/repos/`, which is ignored by git.

| Repo | Commit | URL | Language | Why Inspected |
|---|---:|---|---|---|
| `golang-tools` | `a3954b5c7496` | https://go.googlesource.com/tools | Go | Canonical Go SSA and call graph implementations: static, CHA, RTA, VTA. |
| `go-callvis` | `67a26605e208` | https://github.com/ondrajz/go-callvis | Go | Practical executable use of Go x/tools call graph algorithms. |
| `jelly` | `b799ed4f0d68` | https://github.com/cs-au-dk/jelly | JS/TS | Modern practical JS/TS call graph and value-flow implementation. |
| `tajs` | `3bdf55a411d6` | https://github.com/cs-au-dk/TAJS | JavaScript | Abstract interpretation reference for JS call graphs. |
| `codeql` | `a84332ac150e` | https://github.com/github/codeql | Multi-language | Production query/dataflow call resolution for Go, JS/TS, Java, Python. |
| `wala` | `bd2e8d172542` | https://github.com/wala/WALA | Java/JVM/JS | Mature SSA, CHA, CFA, points-to, and JS field-based call graphs. |
| `soot` | `135d1ead8c89` | https://github.com/soot-oss/soot | Java/JVM | Classic Soot and Spark points-to call graph builder. |
| `sootup` | `c148177e9a26` | https://github.com/soot-oss/SootUp | Java/JVM | Modern Soot-family CHA/RTA API and Java analysis input model. |
| `doop` | `3cb3ae54e7d9` | https://github.com/plast-lab/doop | Java/JVM | Datalog/Souffle points-to and context-sensitive call graph analyses. |
| `opal` | `8870593d34f3` | https://github.com/opalj/opal | Java/JVM | Modern modular Java call graph design, TypeIterator abstraction, OPAL analyses. |
| `tai-e` | `6b6169c178f` | https://github.com/pascal-lab/Tai-e | Java/JVM | Modern Java static-analysis framework with CHA and PTA-based call graphs. |
| `pycg` | `8d5dc408378` | https://github.com/vitsalis/PyCG | Python | Research-grade Python call graph baseline. |
| `jarvis` | `8871b86494a1` | https://github.com/pythonJaRvis/pythonJaRvis.github.io | Python | Python call graph research artifact and PyCG successor direction. |
| `pyan` | `3663440089b1` | https://github.com/Technologicat/pyan | Python | Practical Python AST/symtable/import graph implementation. |
| `pyre-check` | `34af3721bc04` | https://github.com/facebook/pyre-check | Python | Pyre/Pysa typed Python call graph data model and builder. |

## Key Files

### Go

- `repos/golang-tools/go/callgraph/static/static.go`
- `repos/golang-tools/go/callgraph/cha/cha.go`
- `repos/golang-tools/go/callgraph/rta/rta.go`
- `repos/golang-tools/go/callgraph/vta/vta.go`
- `repos/go-callvis/analysis.go`

### JavaScript / TypeScript

- `repos/jelly/src/analysis/astvisitor.ts`
- `repos/jelly/src/analysis/operations.ts`
- `repos/jelly/src/analysis/fragmentstate.ts`
- `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/Nodes.qll`
- `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/internal/CallGraphs.qll`
- `repos/tajs/src/dk/brics/tajs/analysis/FunctionCalls.java`
- `repos/tajs/src/dk/brics/tajs/solver/CallGraph.java`
- `repos/wala/cast/js/src/main/java/com/ibm/wala/cast/js/callgraph/fieldbased/FieldBasedCallGraphBuilder.java`

### Java / JVM

- `repos/sootup/sootup.callgraph/src/main/java/sootup/callgraph/AbstractCallGraphAlgorithm.java`
- `repos/sootup/sootup.callgraph/src/main/java/sootup/callgraph/ClassHierarchyAnalysisAlgorithm.java`
- `repos/sootup/sootup.callgraph/src/main/java/sootup/callgraph/RapidTypeAnalysisAlgorithm.java`
- `repos/soot/src/main/java/soot/jimple/toolkits/callgraph/CallGraphBuilder.java`
- `repos/soot/src/main/java/soot/jimple/spark/SparkTransformer.java`
- `repos/wala/core/src/main/java/com/ibm/wala/ipa/callgraph/cha/CHACallGraph.java`
- `repos/wala/core/src/main/java/com/ibm/wala/ipa/callgraph/propagation/SSAPropagationCallGraphBuilder.java`
- `repos/doop/souffle-logic/main/full-call-graph.dl`
- `repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/TypeIterator.scala`
- `repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/CallGraphAnalysis.scala`
- `repos/tai-e/src/main/java/pascal/taie/analysis/graph/callgraph/CHABuilder.java`
- `repos/tai-e/src/main/java/pascal/taie/analysis/pta/core/solver/DefaultSolver.java`

### Python

- `repos/pycg/pycg/pycg.py`
- `repos/pycg/pycg/processing/cgprocessor.py`
- `repos/jarvis/Jarvis/tool/Jarvis/jarvis.py`
- `repos/jarvis/Jarvis/tool/Jarvis/processing/extProcessor.py`
- `repos/pyan/pyan/analyzer.py`
- `repos/codeql/python/ql/lib/semmle/python/dataflow/new/internal/DataFlowDispatch.qll`
- `repos/codeql/python/ql/lib/semmle/python/pointsto/CallGraph.qll`
- `repos/pyre-check/source/interprocedural/callGraph.ml`
- `repos/pyre-check/source/interprocedural/callGraphBuilder.ml`

