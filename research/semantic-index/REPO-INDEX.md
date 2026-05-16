# Repository Index

Third-party repositories were cloned under `research/semantic-index/repos/`, which is gitignored. They are local research artifacts, not vendored dependencies.

## Cloned Repositories

| Repository | Commit | Why It Was Studied | Key Local Evidence |
|---|---:|---|---|
| <https://github.com/github/codeql> | `a84332ac150e` | Relational multi-language semantic facts and recursive name/import/value-flow libraries. | `go/ql/lib/semmle/go/Scopes.qll`, `python/ql/lib/semmle/python/Scope.qll`, `python/ql/lib/semmle/python/Variables.qll`, `javascript/ql/lib/semmle/javascript/Variables.qll`, `javascript/ql/lib/semmle/javascript/internal/NameResolution.qll` |
| <https://github.com/rust-lang/rust-analyzer> | `1a68212c5683` | Rust-native incremental semantic model, DefMap, scopes, source-to-HIR facade, symbol search. | `crates/hir-def/src/nameres.rs`, `crates/hir-def/src/nameres/collector.rs`, `crates/hir-def/src/item_scope.rs`, `crates/hir-def/src/expr_store/scope.rs`, `crates/hir/src/semantics.rs`, `crates/ide-db/src/symbol_index.rs` |
| <https://github.com/microsoft/TypeScript> | `f350b5233149` | Compiler-owned binder/checker symbol tables, declaration merging, language service references, incremental builder. | `src/compiler/binder.ts`, `src/compiler/types.ts`, `src/compiler/checker.ts`, `src/services/findAllReferences.ts`, `src/compiler/builder.ts`, `src/compiler/builderState.ts` |
| <https://github.com/golang/tools> | `a3954b5c7496` | gopls package metadata, go/types facts, serializable cross-reference index. | `gopls/internal/cache/package.go`, `gopls/internal/cache/metadata/metadata.go`, `gopls/internal/golang/definition.go`, `gopls/internal/golang/references.go`, `gopls/internal/cache/xrefs/xrefs.go` |
| <https://github.com/microsoft/pyright> | `b13157b0fac4` | Python binder, scopes, symbols, declarations, references provider, program/source-file lifecycle. | `packages/pyright-internal/src/analyzer/scope.ts`, `symbol.ts`, `binder.ts`, `sourceFile.ts`, `program.ts`, `service.ts`, `languageService/referencesProvider.ts` |
| <https://github.com/astral-sh/ty> | `a63e55929645` | Modern Rust-native Python semantic index built around Salsa-like tracked queries, place tables, use-def maps. | `ty/ruff/crates/ty_python_core/src/symbol.rs`, `scope.rs`, `place.rs`, `use_def.rs`, `reachability_constraints.rs`, `lib.rs`; `ty_python_semantic/src/semantic_model.rs` |
| <https://github.com/facebook/pyrefly> | `050d3015bd46` | Modern Rust-native Python module binding and graph calculation model. | `ARCHITECTURE.md`, `pyrefly/lib/binding/scope.rs`, `binding.rs`, `bindings.rs`, `table.rs`, `crates/pyrefly_graph/src/index.rs`, `calculation.rs` |
| <https://github.com/eclipse-jdt/eclipse.jdt.core> | `d34042546c11` | Java compiler semantic model: scopes, bindings, lookup environment, import/reference dependency recording. | `org.eclipse.jdt.core.compiler.batch/src/org/eclipse/jdt/internal/compiler/lookup/Binding.java`, `Scope.java`, `LookupEnvironment.java`, `CompilationUnitScope.java`, `dom/.../DefaultBindingResolver.java` |
| <https://github.com/soot-oss/soot> | `135d1ead8c89` | Classic JVM analysis framework: resolving levels, class hierarchy, method maps, call graph/points-to services. | `src/main/java/soot/Scene.java`, `SootResolver.java`, `SootClass.java`, `FastHierarchy.java`, `jimple/toolkits/callgraph/CallGraph.java`, `ReachableMethods.java`, `jimple/spark/SparkTransformer.java` |
| <https://github.com/soot-oss/SootUp> | `c148177e9a26` | Modernized Soot architecture with immutable views, type hierarchy, and call graph algorithms. | `sootup.core/.../views/View.java`, `sootup.java.core/.../JavaView.java`, `sootup.core/.../typehierarchy/TypeHierarchy.java`, `sootup.callgraph/.../AbstractCallGraphAlgorithm.java`, `ClassHierarchyAnalysisAlgorithm.java`, `RapidTypeAnalysisAlgorithm.java` |
| <https://github.com/wala/WALA> | `bd2e8d172542` | JVM/JS analysis framework: analysis scope, class hierarchy, canonical references, context-sensitive call graph nodes, pointer analysis. | `core/.../AnalysisScope.java`, `ipa/cha/ClassHierarchy.java`, `types/TypeReference.java`, `types/MethodReference.java`, `ipa/callgraph/CGNode.java`, `ipa/callgraph/propagation/PointerAnalysis.java`, `AnalysisCacheImpl.java` |
| <https://github.com/semgrep/semgrep> | `2940ecd09a1f` | Multi-language generic AST naming and rule-oriented symbol analysis. | `src/naming/Naming_AST.ml`, `src/naming/Naming_utils.ml`, `cli/src/semgrep/symbol_analysis.py`, `cli/src/semgrep/semgrep_interfaces/semgrep_output_v1_t.mli` |
| <https://github.com/sourcegraph/scip> | `99236e35450c` | Modern code intelligence export schema: documents, occurrences, symbols, relationships. | `scip.proto`, `docs/scip.md`, `docs/DESIGN.md` |
| <https://github.com/microsoft/language-server-protocol> | `3a48eb708502` | LSIF graph export format and LSP-result dump model. | `_specifications/lsif/0.6.0/specification.md`, `_specifications/lsif/0.5.0/specification.md` |
| <https://github.com/kythe/kythe> | `954bc791a8f6` | Durable cross-language semantic graph model: VNames, facts, edges, graph store. | `kythe/proto/storage.proto`, `kythe/proto/schema.proto`, `kythe/go/services/graphstore/graphstore.go`, `kythe/go/util/schema/schema.go` |

## Why These Are The Relevant Set

- **Compiler/language-server indexes:** rust-analyzer, TypeScript, gopls, Pyright, Ty, Pyrefly, JDT.
- **Program-analysis frameworks:** CodeQL, Soot, SootUp, WALA.
- **Rule/pattern systems:** Semgrep.
- **Exchange/storage formats:** SCIP, LSIF, Kythe.

This covers the practical state of the art across Rust, TS/JS, Go, Python, Java/JVM, multi-language code scanning, and code navigation export.

## Not Treated As Primary Sources

- Jedi was not used as a primary source because Pyright, Ty, and Pyrefly better represent current high-precision Python type/semantic indexing directions for large repos and type-aware tooling.
- Tree-sitter-only taggers were not treated as primary sources because the research question is semantic indexing, not syntactic outline extraction.
- Proprietary systems such as Pylance and Sourcegraph's production backend were used only through public schema/code references where available.
