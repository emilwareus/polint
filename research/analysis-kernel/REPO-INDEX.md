# Repository Index

Third-party repositories were cloned under `research/analysis-kernel/repos/` where needed, or reused from previous research under `research/data-flow/repos/`, `research/call-graphs/repos/`, and `research/agent-extension-surface/repos/`. All `research/*/repos/` folders are gitignored.

| System | Local path | Commit checked | Why it matters for the kernel |
|---|---|---:|---|
| Salsa | `research/data-flow/repos/salsa` | `7e77c49f2721` | Rust incremental computation: red-green algorithm, revisions, dependency tracking, durability, backdating, snapshots, LRU tuning. |
| rust-analyzer | `research/analysis-kernel/repos/rust-analyzer` | `1a68212c5683` | Large production Salsa architecture with source roots, semantic layers, invalidation barriers, and typed query databases. |
| Souffle | `research/data-flow/repos/souffle` | `c3861e0d3b82` | Datalog engine internals: typed relations, indexes, SCC scheduling, semi-naive fixpoint, optional provenance. |
| Doop | `research/data-flow/repos/doop` | `3cb3ae54e7d9` | Large Java points-to/call-graph/data-flow analysis as fact extraction plus Datalog rules. |
| CodeQL | `research/data-flow/repos/codeql` | `a84332ac150e` | Relational extraction database, typed QL object views, recursion/fixpoints, data-flow path graph, models-as-data provenance. |
| FlowLog | `research/data-flow/repos/FlowLog` | `388bd4518840` | Rust/Differential Dataflow Datalog-like system with typed language, strata, recursive fixpoints, and transactional incremental mode. |
| WALA | `research/data-flow/repos/WALA` | `bd2e8d172542` | Mature typed analysis products: IR, SSA, call graph, pointer analysis, IFDS, slicing, analysis cache views. |
| Joern | `research/data-flow/repos/joern` | `6f93016d9413` | Code property graph overlays/layers, default analysis layer order, data-flow overlays, graph provenance lessons. |
| TypeScript | `research/data-flow/repos/TypeScript` | `f350b5233149` | Incremental compiler builder: file versions, public signatures, affected-file/project scheduling, `.tsbuildinfo`. |
| Pyre | `research/data-flow/repos/pyre-check` | `34af3721bc04` | Explicit dependency tracked memory, environment tables, update propagation, full-vs-incremental consistency discipline. |
| Go tools / gopls | `research/call-graphs/repos/golang-tools` | `a3954b5c7496` | Practical analyzer cache: recipe hashes, package/analyzer DAG, persistent facts, in-flight dedupe, parse LRU. |
| OpenRewrite | `research/agent-extension-surface/repos/rewrite` | `0f600f466394` | Extension scheduling, typed markers, data tables, multi-cycle recipe execution, evidence rows. |
| Kythe | `research/analysis-kernel/repos/kythe` | `954bc791a8f6` | Cross-language graph storage, VNames, anchors, facts/edges, build-configuration identity. |
| SCIP | `research/analysis-kernel/repos/scip` | `99236e35450c` | Cross-language index schema: documents, occurrences, symbols, relationships, tool metadata, streaming indexes. |
| Semgrep | `research/data-flow/repos/semgrep` | `db2be62416a2` | Taint path reporting and practical model/rule data-flow ergonomics. |

## Notable Source Files

### Salsa / rust-analyzer

- `research/data-flow/repos/salsa/book/src/reference/algorithm.md`
- `research/data-flow/repos/salsa/book/src/reference/durability.md`
- `research/data-flow/repos/salsa/book/src/plumbing/database_and_runtime.md`
- `research/data-flow/repos/salsa/book/src/tuning.md`
- `research/analysis-kernel/repos/rust-analyzer/crates/base-db/src/lib.rs`
- `research/analysis-kernel/repos/rust-analyzer/crates/base-db/src/change.rs`
- `research/analysis-kernel/repos/rust-analyzer/crates/ide-db/src/lib.rs`
- `research/analysis-kernel/repos/rust-analyzer/crates/hir/src/db.rs`

### Relations / Datalog / Fixpoints

- `research/data-flow/repos/souffle/src/ast/analysis/SCCGraph.cpp`
- `research/data-flow/repos/souffle/src/ast2ram/seminaive/UnitTranslator.cpp`
- `research/data-flow/repos/souffle/src/synthesiser/Relation.cpp`
- `research/data-flow/repos/souffle/src/ram/analysis/Index.cpp`
- `research/data-flow/repos/doop/docs/documentation.md`
- `research/data-flow/repos/doop/docs/doop-101.md`
- `research/data-flow/repos/codeql/shared/dataflow/codeql/dataflow/DataFlow.qll`
- `research/data-flow/repos/codeql/shared/dataflow/codeql/dataflow/internal/DataFlowImpl.qll`
- `research/data-flow/repos/codeql/shared/dataflow/codeql/dataflow/internal/DataFlowImplConsistency.qll`
- `research/data-flow/repos/FlowLog/crates/flowlog-build/src/stratifier/core.rs`
- `research/data-flow/repos/FlowLog/crates/flowlog-compiler/src/assembly/inc.rs`

### Analysis Products

- `research/data-flow/repos/WALA/core/src/main/java/com/ibm/wala/ipa/callgraph/CallGraphBuilder.java`
- `research/data-flow/repos/WALA/core/src/main/java/com/ibm/wala/ipa/callgraph/IAnalysisCacheView.java`
- `research/data-flow/repos/WALA/core/src/main/java/com/ibm/wala/dataflow/IFDS/TabulationProblem.java`
- `research/data-flow/repos/WALA/core/src/main/java/com/ibm/wala/ipa/slicer/Slicer.java`
- `research/data-flow/repos/joern/semanticcpg/src/main/scala/io/shiftleft/semanticcpg/layers/LayerCreator.scala`
- `research/data-flow/repos/joern/joern-cli/frontends/x2cpg/src/main/scala/io/joern/x2cpg/X2Cpg.scala`
- `research/data-flow/repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/layers/dataflows/OssDataFlow.scala`

### Incremental Build / IDE Engines

- `research/data-flow/repos/TypeScript/src/compiler/builderState.ts`
- `research/data-flow/repos/TypeScript/src/compiler/builder.ts`
- `research/data-flow/repos/TypeScript/src/compiler/watchPublic.ts`
- `research/data-flow/repos/pyre-check/source/analysis/environment.mli`
- `research/data-flow/repos/pyre-check/source/analysis/environment.ml`
- `research/data-flow/repos/pyre-check/source/service/dependencyTrackedMemory.ml`
- `research/data-flow/repos/pyre-check/source/service/scheduler.ml`
- `research/call-graphs/repos/golang-tools/gopls/internal/cache/analysis.go`
- `research/call-graphs/repos/golang-tools/gopls/internal/cache/parse_cache.go`

### Index Identity

- `research/analysis-kernel/repos/kythe/kythe/proto/storage.proto`
- `research/analysis-kernel/repos/kythe/kythe/typescript/SCHEMA.md`
- `research/analysis-kernel/repos/kythe/kythe/docs/rfc/2967.md`
- `research/analysis-kernel/repos/scip/scip.proto`

### Current polint Integration Points

- `crates/polint/src/runner/mod.rs`
- `crates/polint/src/core/mod.rs`
- `crates/polint/src/analysis_plan.rs`
- `crates/polint/src/cache/mod.rs`
- `crates/polint/src/cache/keys.rs`
- `crates/polint/src/go/adapter.rs`
- `crates/polint/src/ts/adapter.rs`
- `crates/polint/src/module_graph/mod.rs`
- `crates/polint/src/symbol_graph/mod.rs`
- `crates/polint/src/metrics.rs`
