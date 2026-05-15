# Paper And Documentation Index

Downloaded papers live in `research/analysis-kernel/papers/`.

| File | Topic | Why it matters |
|---|---|---|
| `incidfa-oopsla-2025.pdf` | IncIDFA: incremental iterative data-flow analysis. | Latest relevant research on incrementally updating monotone IDFA without resetting SCC solutions to bottom. Useful later for CFG/data-flow invalidation. |
| `incremental-codeql-fse-2023.pdf` | Incrementalizing Production CodeQL Analyses. | Industrial evidence that sophisticated interprocedural/context-sensitive Datalog analyses can be incrementally reused, but with hard limitations. |
| `flowlog-vldb-2026.pdf` | FlowLog. | Modern typed Datalog-like language compiled to Differential Dataflow, relevant for future long-lived/watch-mode relation kernels. |

## Web References

- Salsa red-green algorithm and durability: <https://salsa-rs.github.io/salsa/reference/algorithm.html>
- rust-analyzer architecture: <https://rust-analyzer.github.io/book/contributing/architecture.html>
- rust-analyzer durable incrementality: <https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html>
- CodeQL recursion and least fixed points: <https://codeql.github.com/docs/ql-language-reference/recursion/>
- CodeQL evaluation model: <https://codeql.github.com/docs/ql-language-reference/evaluation-of-ql-programs/>
- CodeQL custom model provenance for Go: <https://codeql.github.com/docs/codeql-language-guides/customizing-library-models-for-go/>
- CodeQL incremental paper page: <https://2023.esec-fse.org/details/fse-2023-industry/1/Incrementalizing-Production-CodeQL-Analyses>
- Souffle provenance docs: <https://souffle-lang.github.io/provenance2>
- IncIDFA OOPSLA 2025 page: <https://2025.splashcon.org/details/OOPSLA/33/IncIDFA-An-Efficient-and-Generic-Algorithm-for-Incremental-Iterative-Dataflow-Analys>
- FlowLog paper PDF: <https://www.vldb.org/pvldb/vol19/p361-zhao.pdf>
- Incremental CodeQL paper PDF: <https://szabta89.github.io/publications/inc-codeql-fse2023.pdf>
- Kythe schema overview: <https://kythe.io/docs/schema-overview.html>
- Kythe storage model: <https://kythe.io/docs/kythe-storage.html>
- SCIP schema: <https://github.com/sourcegraph/scip/blob/main/scip.proto>
- TypeScript incremental option: <https://www.typescriptlang.org/tsconfig/incremental.html>
- SARIF 2.1.0: <https://docs.oasis-open.org/sarif/sarif/v2.1.0/os/sarif-v2.1.0-os.html>
- gopls cache package docs: <https://pkg.go.dev/golang.org/x/tools/gopls/internal/cache>

## Research Takeaways

The papers point in one direction:

1. Batch fixpoint evaluation is simple and robust, but expensive for large recursive analyses.
2. Fully incremental recursive analyses are possible, but the implementation burden is high and the performance win depends on relation shape, dependency precision, and edit locality.
3. A new product should start with explicit layer digests and provider dependency keys, not with a hard dependency on the most complex incremental engine.
4. Once fact families and relation shapes stabilize, polint can add finer incremental invalidation per family:
   - per-file for syntax;
   - per-package/module for symbol and import resolution;
   - SCC-based for call graphs and data-flow summaries;
   - differential/fixpoint maintenance for long-lived agent sessions.

