# OSS Implementation Comparison

## Summary Table

| System | Kernel Shape | Scheduling | Provenance | Cache/Invalidation | What polint should copy | What polint should avoid |
|---|---|---|---|---|---|---|
| Salsa | Incremental query DB | Demand-driven red-green query validation | Dependency graph, not source-level evidence | Revisions, durability, backdating, LRU | Revisions, dependency edges, durability, backdating | Hard dependency before fact families stabilize |
| rust-analyzer | Layered Salsa DBs | Demand-driven queries plus IDE warmup | Mostly internal query dependencies | Source roots, file changes, durability, query caches | Invalidation barriers and semantic layers | Rust-specific DB shape as product architecture |
| Souffle | Typed relations | SCC topological order plus semi-naive fixpoint | Optional provenance/info mode | Batch, not persistent incremental | Relation storage, indexes, SCC/fixpoint | External codegen engine and public Datalog first |
| Doop | Extracted facts plus Datalog analyses | Souffle schedule | Fact/rule trace through Datalog outputs | Batch caches/facts dirs | Extraction discipline and analysis options | Java-specific complexity and compile-heavy lifecycle |
| CodeQL | Relational DB plus QL object views | Bottom-up least fixed points | Path graphs, model provenance | Snapshot DB; incremental research prototypes | Product shape, path evidence, models-as-data | Full QL language/evaluator as first public API |
| FlowLog | Typed Datalog to Differential Dataflow | Strata and recursive fixpoints | Internal relation provenance | Transactional epochs/differential updates | Future watch-mode inspiration | Differential dependency before stable schema |
| WALA | Typed analysis products | Builder-driven analysis products | IR/call graph/data-flow product evidence | Analysis cache views | Separate products for IR/CFG/call graph/dataflow | Broad public option object |
| Joern | Code property graph overlays | Fixed overlay order and dependency checks | Graph overlays and semantics files | Persistent graph overlays | Named layer/overlay manifests | Mutable public global graph and skip-on-missing deps |
| gopls | Analyzer/package DAG | Postorder package/analyzer DAG | Analyzer facts/diagnostics | Recipe hashes, persistent cache, in-flight dedupe | Provider recipe keys and package DAG | Go-only package assumptions |
| TypeScript | Incremental compiler builder | Affected files/projects | Diagnostics/build info | File versions, signature shape, `.tsbuildinfo` | Shape digests and affected work | TS-specific mutable builder state |
| Pyre | Environment tables | Dependency-key propagation | Explicit dependencies | Shared memory updates, Get/Mem deps | Presence dependencies and full-vs-incremental tests | Manual dependency system without strong tests |
| OpenRewrite | Recipes, markers, data tables | Scan/generate/edit cycles | Markers and data tables | Run cycles, recipe outputs | Evidence tables and controlled multipass | Mutation-oriented recipe API as analysis kernel |
| Kythe | Cross-language graph entries | Indexer/post-processing pipeline | Facts/edges and anchors | Stable VNames, graph store | Stable entity identity, anchors, build-config identity | Treating index graph as full analysis engine |
| SCIP | Cross-language index schema | Indexer emits documents/symbols/occurrences | Tool metadata and occurrence ranges | Streaming index files | Symbol/occurrence/relationship schema discipline | Limiting polint to code intelligence indexing |

## Detailed Notes

### Salsa

Salsa's red-green algorithm tracks query dependencies and revisions. If inputs did not change, cached outputs are reused. If a dependency may have changed, Salsa can re-execute and backdate if the output is equal. Durability reduces validation work for stable inputs.

polint lesson: use revisions, durability, and output equality even in a batch layer cache. Parser outputs and module graph outputs can be reused if their normalized outputs are unchanged.

### rust-analyzer

rust-analyzer layers syntax, source roots, crate graph, macro expansion, name resolution, HIR, and type information. It uses invalidation barriers so small edits do not invalidate the world.

polint lesson: create shape digests for fact layers. Avoid using raw file content as the only invalidation unit for every downstream analysis.

### Souffle and Doop

Souffle builds a precedence graph, finds SCCs, and evaluates recursive strata with semi-naive deltas. Doop uses facts and Datalog rules to express complex Java analyses.

polint lesson: recursive analysis families should be relation-based internally. Call graph/data flow/effects should not be ad hoc DFS passes once they become interdependent.

### CodeQL

CodeQL extracts code into a relational database, then QL libraries expose object-style views. Recursion uses least fixed point semantics. Data-flow path graphs provide explainable edges and subpaths. Models-as-data carries provenance.

polint lesson: rule authors can get ergonomic typed views over relational facts. The kernel can stay relational while the SDK stays Rust-native.

### FlowLog

FlowLog shows a modern path for typed Datalog-like programs compiled to incremental dataflow. Its transactional/epoch model is attractive for long-lived agent sessions.

polint lesson: do not start here, but keep the kernel's relation API compatible with future differential maintenance.

### WALA

WALA keeps analysis products explicit. IFDS consumes supergraphs, domains, flow functions, and seeds. Call graph construction exposes pointer analysis.

polint lesson: internal products should be typed and explicit. Public rules should not rummage around a global graph.

### Joern

Joern overlays are the closest direct analogy to fact layers. A layer declares a name and dependencies; default overlays are applied in order.

polint lesson: named layers are good. But polint should plan prerequisites or emit capability diagnostics, not silently skip important layers.

### TypeScript and gopls

These systems are practical cache references. TypeScript uses public signature changes to limit downstream work. gopls computes recipe hashes over analyzer inputs and dependencies.

polint lesson: use provider recipe keys and shape digests.

### Pyre

Pyre's dependency tracking includes presence/membership, not only value reads.

polint lesson: dependency tracking for absence matters. "No matching source model" and "symbol unresolved" are dependencies too.

### Kythe and SCIP

These systems show cross-language indexing identity.

polint lesson: use stable keys and anchors. Build/lifecycle configuration may be part of identity for facts tied to source anchors.

