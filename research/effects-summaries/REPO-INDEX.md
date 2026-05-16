# Repository Index

Reference repositories were cloned under `research/effects-summaries/repos/`.
The clone directory is ignored by git. Commit ids below are the inspected local
snapshots.

## Cloned Repositories

| Repository | Local path | Snapshot | Why inspected |
|---|---|---:|---|
| github/codeql | `repos/codeql` | `a84332ac150e` | Models-as-data, summary models, barriers, data-flow library. |
| semgrep/semgrep | `repos/semgrep` | `2940ecd09a1f` | Taint-mode ergonomics, propagators, by-side-effect, exactness flags. |
| joernio/joern | `repos/joern` | `da77724000f5` | CPG and custom flow semantics. |
| facebook/pyre-check | `repos/pyre-check` | `34af3721bc04` | Pysa summary-first taint engine. |
| facebook/infer | `repos/infer` | `01a41d72f5fd` | Pulse, biabduction lineage, RacerD summaries, on-disk summaries. |
| golang/tools | `repos/golang-tools` | `a3954b5c7496` | `go/analysis` facts, `buildssa`, nilness, package provider model. |
| golang/go | `repos/golang-go` | `9e0467b174f0` | Official Go compiler/toolchain semantics context. |
| microsoft/TypeScript | `repos/typescript` | `f350b5233149` | Type predicates, assertions, control-flow narrowing, `never`. |
| microsoft/pyright | `repos/pyright` | `b13157b0fac4` | Python type narrowing and flow-sensitive checker reference. |
| astral-sh/ruff | `repos/ruff` | `a7ab646e3e2a` | Rust-native Python parser/type-system direction via Ty/Ruff ecosystem. |
| wala/WALA | `repos/wala` | `bd2e8d172542` | JVM summaries, bypass models, mod/ref, reflection options. |
| soot-oss/soot | `repos/soot` | `135d1ead8c89` | JVM side-effect analysis and RWSet summaries. |
| soot-oss/SootUp | `repos/SootUp` | `7caccae57e9c` | Modern Soot call graph/input-location architecture. |
| plast-lab/doop | `repos/doop` | `3cb3ae54e7d9` | Datalog points-to/call graph, reflection/native/open-program model packs. |
| opalj/opal | `repos/opal` | `bfef3aae2630` | FPCF property store, purity/allocation/exceptions properties. |
| typetools/checker-framework | `repos/checker-framework` | `0be8d5a60400` | Purity annotations, dataflow framework, stubs. |
| llvm/llvm-project | `repos/llvm-project` | `199e750a9013` | LLVM memory effects/mod-ref, Attributor, MLIR side-effect resources. |
| Sable/heros | `repos/heros` | `091e3bd58505` | IFDS/IDE solver interfaces, flow and edge functions. |
| secure-software-engineering/phasar | `repos/phasar` | `0f4c0bf34e9e` | LLVM IFDS/IDE framework and library summaries. |
| secure-software-engineering/FlowDroid | `repos/FlowDroid` | `73cee57ab532` | Android lifecycle/source-sink data-flow benchmark/reference. |

## High-Signal Source Paths

### CodeQL

- `javascript/ql/lib/semmle/javascript/dataflow/FlowSummary.qll`
- `javascript/ql/lib/semmle/javascript/dataflow/Configuration.qll`
- `javascript/ql/lib/semmle/javascript/dataflow/internal/DataFlowPrivate.qll`
- `javascript/ql/lib/ext/*.model.yml`
- `python/ql/lib/modeling/ModelEditor.qll`

Relevant observations:

- `FlowSummary.qll` exposes `SummarizedCallable::Range` and `propagatesFlow(input, output, preservesValue, provenance, isExact, model)`.
- JS model YAML extends `summaryModel` with access paths such as `Argument[0].Awaited` and `ReturnValue`.
- The data-flow library treats a step through a summary as a level step, which is exactly the kind of boundary polint needs.

### Pysa/Pyre

- `source/interprocedural_analyses/taint/model.ml`
- `source/interprocedural_analyses/taint/taintFixpoint.ml`
- `source/interprocedural_analyses/taint/domains.ml`
- `source/analysis/taintAccessPath.ml`
- `source/interprocedural_analyses/taint/modelVerifier.ml`

Relevant observations:

- A model contains returned sources, sinks reached by parameters, and taint-in-taint-out.
- The global fixpoint iterates forward and backward analysis until no sources/sinks propagate.
- Access paths use roots such as local result, positional parameter, named parameter, starred parameters, variables, and captured variables.

### Infer

- `infer/src/backend/Summary.ml`
- `infer/src/pulse/PulseSummary.ml`
- `infer/src/pulse/PulseAbductiveDomain.ml`
- `infer/src/pulse/PulseBaseAddressAttributes.ml`
- `infer/src/concurrency/RacerDDomain.ml`
- `infer/src/concurrency/RacerDProcAnalysis.ml`

Relevant observations:

- On-disk summaries are keyed by procedure and analysis request, with summary metadata and dependencies.
- Pulse summaries store disjunctive pre/post execution states plus non-disjunctive data.
- Pulse address attributes model allocation, invalidation, awaitable/resource obligations, taint, initialization, and copy provenance.

### Go Tools

- `go/analysis/analysis.go`
- `go/analysis/doc.go`
- `go/analysis/checker/checker.go`
- `go/analysis/passes/buildssa/buildssa.go`
- `go/analysis/passes/nilness/nilness.go`
- `go/ssa/create.go`

Relevant observations:

- `go/analysis` has analyzer dependencies, package/object facts, imported/exported facts, and result maps.
- `buildssa` is the official x/tools bridge to SSA-backed local analysis.
- `SetNoReturn` allows official no-return knowledge to shape SSA construction.

### LLVM And MLIR

- `llvm/include/llvm/Support/ModRef.h`
- `llvm/include/llvm/Analysis/AliasAnalysis.h`
- `llvm/include/llvm/Transforms/IPO/Attributor.h`
- `llvm/lib/Transforms/IPO/Attributor.cpp`
- `mlir/include/mlir/Interfaces/SideEffectInterfaces.h`
- `mlir/include/mlir/Interfaces/SideEffectInterfaces.td`
- `mlir/lib/Interfaces/SideEffectInterfaces.cpp`

Relevant observations:

- LLVM `MemoryEffects` is a compact product lattice: mod/ref kind crossed with memory-location kind.
- MLIR effects are resource-scoped `EffectInstance`s with `Read`, `Write`, `Allocate`, and `Free`, plus resource hierarchy/disjointness.

### JVM Systems

- WALA: `core/src/main/java/com/ibm/wala/ipa/summaries/MethodSummary.java`
- WALA: `core/src/main/java/com/ibm/wala/ipa/summaries/XMLMethodSummaryReader.java`
- WALA: `core/src/main/java/com/ibm/wala/ipa/modref/ModRef.java`
- Soot: `src/main/java/soot/jimple/toolkits/pointer/SideEffectAnalysis.java`
- Soot: `src/main/java/soot/jimple/toolkits/pointer/RWSet.java`
- OPAL: `OPAL/br/src/main/scala/org/opalj/br/fpcf/analyses/L0AllocationFreenessAnalysis.scala`
- OPAL: `OPAL/br/src/main/scala/org/opalj/br/fpcf/analyses/L1ThrownExceptionsAnalysis.scala`

Relevant observations:

- WALA models library behavior as synthetic SSA-like method summaries.
- Soot computes transitive read/write sets from points-to and call graph targets.
- OPAL models effects as separate properties over a property store with eager/lazy fixed-point scheduling.

### IFDS/IDE And Data-Flow Frameworks

- Heros: `src/heros/FlowFunctions.java`
- Heros: `src/heros/EdgeFunctions.java`
- Heros: `src/heros/solver/PathEdge.java`
- PhASAR: `include/phasar/Utils/FunctionDataFlowFacts.h`
- PhASAR: `include/phasar/DataFlow/IfdsIde/FlowFunctions.h`
- PhASAR: `include/phasar/DataFlow/IfdsIde/EdgeFunctions.h`

Relevant observations:

- IFDS/IDE expose explicit normal/call/return/call-to-return flow functions.
- PhASAR adds a specific summary-flow hook and a serializable library summary mapping parameters to return/parameter facts.
