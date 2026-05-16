# Paper And Source Index

This index records the papers and official sources used for the incremental
query engine research. Local copies of PDFs are stored in `papers/` where
available.

## Research Papers

| Source | Local file | Main lesson for polint |
|---|---|---|
| Incrementalizing Production CodeQL Analyses, FSE 2023. <https://2023.esec-fse.org/details/fse-2023-industry/1/Incrementalizing-Production-CodeQL-Analyses> PDF: <https://szabta89.github.io/publications/inc-codeql-fse2023.pdf> | `papers/incremental-codeql-fse-2023.pdf` | Fine-grained incremental Datalog can make update time proportional to change size, but full incrementality can require very high memory. Hybrid caching is the practical production lesson. |
| IncIDFA: An Efficient and Generic Algorithm for Incremental Iterative Dataflow Analysis, OOPSLA 2025. <https://2025.splashcon.org/details/OOPSLA/33/IncIDFA-An-Efficient-and-Generic-Algorithm-for-Incremental-Iterative-Dataflow-Analys> | `papers/incidfa-oopsla-2025.pdf` | Recursive data-flow SCCs should not always reset to bottom/top on changes. A generic incremental IDFA algorithm can update monotone analyses more cheaply. |
| FlowLog: A Datalog Engine With Flexible State-Delta Management, PVLDB 2026. <https://www.vldb.org/pvldb/vol19/p361-zhao.pdf> | `papers/flowlog-vldb-2026.pdf` | Differential-style relation engines can support incremental recursive Datalog, but the relation backend should be a later specialized sub-engine, not the whole polint kernel. |
| Adapton: Composable, Demand-Driven Incremental Computation. <https://www.cs.tufts.edu/~jfoster/papers/cs-tr-5027.pdf> | `papers/adapton-demand-driven-incremental-computation.pdf` | Demand matters. polint should not eagerly compute every expensive global fact when rules only request a subset. |
| Demanded Abstract Interpretation, PLDI 2021. <https://arxiv.org/pdf/2104.01270> | `papers/demanded-abstract-interpretation-2021.pdf` | Abstract interpretation can be demand-driven and incremental while retaining soundness/termination, if dependencies are explicitly represented. |
| Using Standard Typing Algorithms Incrementally, 2018. <https://arxiv.org/pdf/1808.00225> | `papers/using-standard-typing-algorithms-incrementally-2018.pdf` | Existing language algorithms can be made incremental with grey-box caches when inputs, context, and result shape are explicit. Useful for official language-tool integration. |
| Differential Dataflow, 2013. <https://www.microsoft.com/en-us/research/wp-content/uploads/2013/01/differentialdataflow.pdf> | `papers/differential-dataflow-2013.pdf` | Partial-order timestamps and maintained traces are powerful for changing recursive graph relations, but memory and implementation cost are high. |
| Naiad: A Timely Dataflow System, 2013. <https://www.cs.princeton.edu/courses/archive/fall22/cos418/papers/naiad.pdf> | `papers/naiad-timely-dataflow.pdf` | Logical timestamps, cyclic dataflow, and notifications are useful mental models for a later daemon/relation backend. |

## Official Documentation And Source References

| Source | Why it matters |
|---|---|
| Salsa red-green algorithm and durability docs. <https://salsa-rs.github.io/salsa/reference/algorithm.html> | The clearest Rust-native design for demand queries, dependency recording, durability, red-green verification, and backdating. |
| rust-analyzer architecture. <https://rust-analyzer.github.io/book/contributing/architecture.html> | Production use of Salsa for a large interactive analyzer, including stable file IDs, item tree shape separation, and cancellation. |
| TypeScript `incremental` option. <https://www.typescriptlang.org/tsconfig/incremental.html> | Official compiler support for storing project graph and file metadata in `.tsbuildinfo`. |
| Bazel Skyframe source. <https://github.com/bazelbuild/bazel> | Mature incremental build graph with dirty checking, reverse dependencies, equality-based pruning, and versioned nodes. |
| Buck2 DICE source. <https://github.com/facebook/buck2> | Modern Rust dynamic incremental computation engine with key equality, injected inputs, projections, transactions, and invalidation tracking. |
| Go tools/gopls source. <https://github.com/golang/tools> | Snapshot-based incremental workspace model, parse cache, package metadata invalidation, and analysis cache keys. |
| TypeScript compiler source. <https://github.com/microsoft/TypeScript> | Shape-signature based invalidation and affected-file scheduling. |
| Pyright source. <https://github.com/microsoft/pyright> | Import graph dirtying, transitive dependent marking, resolver invalidation, and library-change batching. |
| Pyrefly source. <https://github.com/facebook/pyrefly> | Modern module-level incremental type-checker architecture in Rust, with dirty epochs and transaction consistency. |
| Pyre/Pysa source. <https://github.com/facebook/pyre-check> | Shared-memory saved-state and cache invalidation for type and interprocedural taint analysis. |
| Souffle source. <https://github.com/souffle-lang/souffle> | Semi-naive Datalog evaluation, delta relations, indexing, provenance, and relation representations. |
| Ruff/Ty source. <https://github.com/astral-sh/ruff> | Current Rust/Python analyzer work using Salsa, with concrete lessons about stable project handles and untracked state. |

## Claims To Treat Carefully

- Incrementality does not improve worst-case complexity. A public API edit,
  lockfile change, or extension change can legitimately invalidate a large
  reverse-dependency closure.
- Fine-grained relation incrementality can trade CPU for memory. The CodeQL
  research is the strongest warning here.
- Query engines require disciplined inputs. Untracked global state, filesystem
  reads, toolchain drift, environment variables, or undeclared extension reads
  can make cached facts wrong.
- Demand-driven analysis can improve latency, but it can also hide costs until
  rule execution. polint needs telemetry and budgets.
