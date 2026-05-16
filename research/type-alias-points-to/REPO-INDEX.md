# Repository Index

Third-party repositories were cloned under `research/type-alias-points-to/repos/`. That directory is gitignored by the repository-level `research/*/repos/` rule.

## Cloned Repository Snapshots

| Repository | Local path | Snapshot | Why inspected |
|---|---:|---:|---|
| `astral-sh/ty` | `repos/ty` | `a63e55929645` | Ty wrapper/docs/release context; source lives in Ruff submodule. |
| `astral-sh/ruff` | `repos/ruff` | `dd3fc71130b` | Ty source crates: Rust-native Python type/place/narrowing engine. |
| `facebook/pyrefly` | `repos/pyrefly` | `050d3015bd46` | Rust-native Python type checker architecture, bindings, flow types, module solving. |
| `microsoft/pyright` | `repos/pyright` | `b13157b0fac4` | Mature Python flow-node/narrowing/type checker. |
| `facebook/pyre-check` | `repos/pyre-check` | `34af3721bc04` | Pyre/Pysa interprocedural and taint model reference. |
| `python/mypy` | `repos/mypy` | `e27179372e28` | Python binder and type narrowing reference. |
| `google/pytype` | `repos/pytype` | `f411a8b445c1` | Typegraph/VM-style Python analysis reference. |
| `microsoft/TypeScript` | `repos/typescript` | `f350b5233149` | TypeScript flow nodes and structural type checker. |
| `oxc-project/oxc` | `repos/oxc` | `795cebf06da5` | Rust-native JS/TS parser, semantic, scope, and CFG substrate. |
| `facebook/flow` | `repos/flow` | `eb6ae1d5d67d` | JS type checker refinement/incrementality reference. |
| `cs-au-dk/TAJS` | `repos/tajs` | `3bdf55a411d6` | JavaScript abstract interpretation and heap/value domains. |
| `cs-au-dk/jelly` | `repos/jelly` | `b799ed4f0d68` | Modern JS/TS call graph and points-to style analysis. |
| `github/codeql` | `repos/codeql` | `a84332ac150e` | Query-facing type tracking, data flow, API modeling, call graph libraries. |
| `golang/tools` | `repos/golang-tools` | `a3954b5c7496` | Go SSA, callgraph static/CHA/RTA/VTA, analysis passes. |
| `golang/go` | `repos/golang-go` | `c6eaf037885e` | Go compiler and `go/types` semantic reference. |
| `dominikh/go-tools` | `repos/staticcheck` | `27f54249714d` | Production Go static analyzers over Go types/SSA. |
| `plast-lab/doop` | `repos/doop` | `3cb3ae54e7d9` | Datalog Java points-to/call graph framework. |
| `wala/WALA` | `repos/wala` | `bd2e8d172542` | Java/JVM SSA, pointer analysis, call graph builders. |
| `soot-oss/soot` | `repos/soot` | `135d1ead8c89` | Classic Soot/Jimple/Spark points-to framework. |
| `soot-oss/SootUp` | `repos/sootup` | `7caccae57e9c` | Modern Soot, call graphs, Spark/Qilin options. |
| `opalj/opal` | `repos/opal` | `bfef3aae2630` | JVM bytecode/TAC/fixpoint analysis framework. |
| `typetools/checker-framework` | `repos/checker-framework` | `cc0c76cc2957` | Java source-level dataflow/type qualifier framework. |
| `SVF-tools/SVF` | `repos/svf` | `72ff689903bf` | Pointer analysis, memory SSA, sparse value-flow graph. |
| `llvm/llvm-project` | `repos/llvm-project` | `4f9a7d09f476` | LLVM AliasAnalysis and MemorySSA. |
| `rust-lang/rust` | `repos/rust` | `88ba7fbe0a6e` | Rust borrow checker/MIR dataflow reference. |
| `rust-lang/polonius` | `repos/polonius` | `2ea65ee209e3` | Relational borrow checking fact engine. |
| `rust-lang/rust-analyzer` | `repos/rust-analyzer` | `1a68212c5683` | Incremental semantic database/reference architecture. |
| `souffle-lang/souffle` | `repos/souffle` | `c3861e0d3b82` | Datalog engine reference. |
| `joernio/joern` | `repos/joern` | `da77724000f5` | Code property graph and data-flow query architecture. |

## Key Source Paths

### Ty / Ruff

- `crates/ty_python_core/src/place.rs`
- `crates/ty_python_core/src/builder.rs`
- `crates/ty_python_core/src/predicate.rs`
- `crates/ty_python_core/src/reachability_constraints.rs`
- `crates/ty_python_semantic/src/reachability.rs`
- `crates/ty_python_semantic/src/types/infer.rs`
- `crates/ty_python_semantic/src/types/narrow.rs`
- `crates/ty_python_semantic/src/types/relation.rs`
- `crates/ty_module_resolver/src/resolve.rs`

### Pyrefly

- `ARCHITECTURE.md`
- `crates/pyrefly_graph/src/calculation.rs`
- `crates/pyrefly_types/src/types.rs`
- `crates/pyrefly_types/src/heap.rs`
- `crates/pyrefly_types/src/type_alias.rs`
- `crates/pyrefly_build/src/source_db/*`

### Pyright

- `packages/pyright-internal/src/analyzer/binder.ts`
- `packages/pyright-internal/src/analyzer/codeFlowTypes.ts`
- `packages/pyright-internal/src/analyzer/codeFlowEngine.ts`
- `packages/pyright-internal/src/analyzer/checker.ts`
- `packages/pyright-internal/src/analyzer/typeEvaluatorTypes.ts`

### TypeScript

- `src/compiler/binder.ts`
- `src/compiler/checker.ts`
- `src/compiler/types.ts`

### Go

- `go/callgraph/static/static.go`
- `go/callgraph/cha/cha.go`
- `go/callgraph/rta/rta.go`
- `go/callgraph/vta/vta.go`
- `go/callgraph/vta/graph.go`
- `go/ssa/*`
- `src/go/types/*` in `golang-go`

### Java/JVM

- Doop: `souffle-logic/main/*`, especially reflection and points-to relations.
- WALA: `core/src/main/java/com/ibm/wala/ipa/callgraph`, `core/src/main/java/com/ibm/wala/analysis/pointers`.
- Soot: `src/main/java/soot/jimple/spark`, `src/main/java/soot/toolkits`.
- SootUp: `sootup.callgraph`, `sootup.spark`, `qilin`.
- OPAL: `OPAL/*/src/main/scala`.
- Checker Framework: `dataflow/src/main/java/org/checkerframework/dataflow`.

### Neutral

- LLVM: `llvm/lib/Analysis/AliasAnalysis.cpp`, `llvm/docs/AliasAnalysis.rst`, `llvm/docs/MemorySSA.rst`.
- SVF: `svf/lib/MSSA`, `svf/lib/Graphs`, `svf/lib/CFL`.
- Rust: `compiler/rustc_borrowck`, `compiler/rustc_mir_dataflow`.
- Polonius: `polonius-engine/src`, `inputs`.
- rust-analyzer: crates for semantic database and HIR.

## Repository Caveats

- Cloned repositories are research inputs and should not become runtime dependencies.
- Some clones are sparse or partial to reduce disk usage; source paths were inspected through checkout or `git show` where needed.
- The `ty` repository is a wrapper around a Ruff submodule; the relevant Ty implementation was inspected in the Ruff clone.
- Current `golang-tools` did not include `go/pointer`; this corrected an earlier assumption that current x/tools still ships that package.
