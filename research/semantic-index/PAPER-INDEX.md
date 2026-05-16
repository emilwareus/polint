# Paper And Technical Source Index

Papers were downloaded under `research/semantic-index/papers/` when available. Official documentation pages and source repositories are linked directly.

## Downloaded Research Papers

| Local File | Source | Relevance |
|---|---|---|
| `papers/ql-for-source-code-analysis.pdf` | <https://codeql.github.com/publications/ql-for-source-code-analysis.pdf> | Foundational CodeQL/QL paper: relational database of source facts, object-oriented Datalog-like query language, least-fixpoint recursion. |
| `papers/incremental-codeql-fse-2023.pdf` | <https://szabta89.github.io/publications/inc-codeql-fse2023.pdf> | Production CodeQL incrementalization: how precise relational analyses interact badly with small source changes and how extraction/query layers can be incrementized. |
| `papers/incremental-typing-2018.pdf` | <https://arxiv.org/abs/1808.00225> | Grey-box incremental type checking: reuse ordinary type checkers while tracking dependencies and edits. Relevant to polint's desire to avoid freezing the first query engine too early. |
| `papers/java-call-graph-unsoundness-2026.pdf` | <https://arxiv.org/abs/2604.00885> | Recent study of Soot, SootUp, WALA, and Doop semantic inconsistencies. Important warning: even mature frameworks disagree on "ground truth." |
| `papers/program-derived-semantics-graph-2020.pdf` | <https://arxiv.org/abs/2009.12537> | Program-derived semantics graph for code understanding. Relevant to AI-oriented graph construction, but less concrete than compiler indexes. |
| `papers/scg-cli-2023.pdf` | Project/publication sources for Semantic Code Graph CLI | Portable protobuf code semantics graph for Java/Scala comprehension. Useful as a graph export/transport reference. |
| `papers/semantic-code-graph-2023.pdf` | Semantic Code Graph paper/project material | Information model for code dependencies and comprehension. Useful for export/schema comparison, not the internal engine. |
| `papers/aoci-ai-oriented-code-indexing-2026.pdf` | arXiv preprint, "AOCI: Symbolic-Semantic Indexing for Practical Repository-Scale Code Understanding with LLMs" | Very recent AI-oriented code indexing work. Relevant to polint's agent-facing use case; should be treated as emerging, not as mature engineering baseline. |

## Official Technical Sources

| Source | Relevance |
|---|---|
| CodeQL documentation: <https://codeql.github.com/docs/> | Database/query/extractor model and QL library behavior. |
| GitHub CodeQL extractor docs: <https://docs.github.com/en/enterprise-cloud@latest/code-security/reference/code-scanning/codeql/codeql-cli/extractor-options> | Extractor options and language-specific database creation inputs. |
| rust-analyzer architecture: <https://rust-analyzer.github.io/book/contributing/architecture.html> | HIR, salsa, incremental semantic model, and code organization. |
| rust-analyzer durable incrementality: <https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html> | Salsa query dependency tracking and durability idea. |
| TypeScript incremental documentation: <https://www.typescriptlang.org/tsconfig/incremental.html> | `.tsbuildinfo` and project graph reuse. |
| TypeScript compiler internals overview: <https://basarat.gitbook.io/typescript/overview> | Binder/checker/symbol table orientation. Used as secondary documentation; source code was primary. |
| gopls package docs: <https://pkg.go.dev/golang.org/x/tools/gopls/internal/cache> and <https://pkg.go.dev/golang.org/x/tools/gopls/internal/cache/xrefs> | Package metadata/cache and serializable cross-reference index. |
| Pyright repository/docs: <https://github.com/microsoft/pyright> | Python binder, scopes, symbols, type evaluator, and language service. Source code was primary. |
| Ty repository/docs: <https://github.com/astral-sh/ty> and <https://docs.astral.sh/ty/> | Modern Rust-native Python type checker and semantic-index architecture. |
| JDT lookup docs: <https://wiki.eclipse.org/JDT_Core_Programmer_Guide/ECJ/Lookups> | LookupEnvironment, package/type lookup, and module perspective. |
| JDT binding docs: <https://wiki.eclipse.org/JDT_Core_Programmer_Guide/ECJ/Bindings> | Binding hierarchy and identity notes. |
| WALA repository/wiki: <https://github.com/wala/WALA> and <https://github-wiki-see.page/m/wala/WALA/wiki/Call-Graph> | Class hierarchy, pointer analysis, context-sensitive call graph, analysis scope. |
| Semgrep repository/docs: <https://github.com/semgrep/semgrep> | Generic AST/naming and rule-oriented analysis surface. |
| SCIP schema/design: <https://github.com/sourcegraph/scip/blob/main/scip.proto> and <https://github.com/sourcegraph/scip/blob/main/docs/DESIGN.md> | Modern occurrence/symbol exchange format. |
| LSIF spec: <https://microsoft.github.io/language-server-protocol/overviews/lsif/overview/> | Graph dump format for LSP-derived code intelligence. |
| Kythe schema/storage docs: <https://kythe.io/docs/schema-overview.html>, <https://kythe.io/docs/schema/>, <https://kythe.io/docs/kythe-storage.html> | Durable semantic graph, VName identity, facts/edges/storage model. |

## Research Takeaways

1. The strongest evidence supports **layered language-specific semantic facts**, not a universal AST-based resolver.
2. Relational and graph models are powerful for derived relations and export, but every mature implementation still depends on language-specific extraction and identity.
3. Incrementality is not just caching: it requires stable semantic units, dependency tracking, and precise invalidation boundaries.
4. The latest AI-oriented indexing papers reinforce polint's product thesis, but the implementation should still copy proven compiler/LSP designs before speculative LLM-first index structures.
