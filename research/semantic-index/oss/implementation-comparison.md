# Implementation Comparison

| Tool | Best At | Internal Shape | Accuracy Model | Complexity Driver | Polint Should Copy | Polint Should Avoid |
|---|---|---|---|---|---|---|
| CodeQL | Multi-language derived relations | Extracted DB + QL predicates | Precise where extractor/library/query are precise | Relation joins, recursion, DB build | Derived fact layers, recursive relation thinking | Runtime dependency or all-facts-in-DB architecture |
| rust-analyzer | Rust-native incremental semantics | HIR, DefMap, item scopes, expr scopes, Salsa | Compiler-like Rust semantics | Macro/import fixed point, query invalidation | typed arenas, semantic facade, fixed-point bounds | Public Salsa/query API |
| TypeScript | TS/JS compiler semantics | AST + binder symbols + checker + builder state | Compiler-owned exactness where possible | Type checking, overloads, unions, project refs | binder/checker split, signature digests | Mutable AST-based public facts |
| gopls | Go package/type facts and xrefs | metadata + package + types.Info + serialized xrefs | Package/type-checker exactness under lifecycle inputs | package loading, build tags, variants | package metadata, object paths, xref indexes | hidden lifecycle failures |
| Pyright | Mature Python language service | scopes + symbols + declarations + program state | type-aware Python with dynamic unknowns | imports, type narrowing, dynamic libs | scope kinds, symbol flags, candidate search | exact claims for runtime attributes |
| Ty | Rust-native Python semantics | tracked semantic index + scopes + places + use-def | evolving but architecturally strong | use-def/reachability, incremental queries | place/use-def separation, tracked facts | blocking on full Python checker |
| Pyrefly | Fast Rust-native Python binding | module exports + bindings + graph calc | explicit static/flow lookup states | import SCCs, binding graph | module/SCC batching, typed binding tables | assuming acyclic import graphs |
| JDT | Java source/binary bindings | LookupEnvironment + scopes + bindings | compiler-grade Java under classpath/module inputs | generics, overloads, classpath | binding taxonomy, problem bindings | file-only Java analysis |
| Soot | JVM whole-program analysis | Scene + classes + hierarchy + call graph | closed-world-ish with phantom classes | class resolving, points-to | resolving levels, hierarchy indexes | global mutable singleton |
| SootUp | Modern JVM analysis view | View + JavaView + TypeHierarchy | algorithm-tier precision | hierarchy lookup, RTA fixed point | AnalysisView, pending unresolved calls | one exact JVM algorithm |
| WALA | Context-sensitive JVM analysis | scope + hierarchy + references + CGNode | method + context identity | pointer propagation, context explosion | context in future callable IDs | flattening classloader/context |
| Semgrep | Rule ergonomics | generic AST + naming + RPC symbol analysis | useful but intentionally incomplete | AST size, pattern matching | ergonomic pattern/rule UX | generic naming as core semantic index |
| SCIP | Code-intelligence export | documents + occurrences + symbols | emitter-dependent | occurrence count | symbol grammar, occurrence roles | internal storage format |
| LSIF | LSP result export | graph vertices/edges/result sets | emitter-dependent | graph size | moniker concept | graph-shaped internal SDK |
| Kythe | Durable semantic graph | VName + facts + edges + graphstore | indexer-dependent | entries and graph scans | VNames, anchors, generated nodes | rule-time graphstore |

## Ranking For Polint Design Influence

1. **rust-analyzer:** strongest native Rust architecture reference.
2. **Ty/Pyrefly:** strongest modern Rust-native dynamic-language references.
3. **TypeScript/gopls/Pyright/JDT:** strongest language-specific semantic truth references.
4. **CodeQL:** strongest derived relation/query model reference.
5. **SCIP/Kythe:** strongest export/stable identity references.
6. **Soot/SootUp/WALA:** strongest JVM whole-program identity references.
7. **Semgrep:** strongest rule ergonomics reference, not semantic exactness reference.
8. **LSIF:** historical export reference only.

## Overall Recommendation

Use compiler/LSP semantic design for base facts, CodeQL-like fixpoints for derived facts, and SCIP/Kythe-like identity for export.

Do not select one tool architecture wholesale.
