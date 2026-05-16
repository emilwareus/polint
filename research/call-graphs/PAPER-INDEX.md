# Paper And Documentation Index

Downloaded papers and docs live in `research/call-graphs/papers/`. PDF metadata was validated with `pdfinfo` on 2026-05-15.

| File | Validated Title | Pages | Why It Matters |
|---|---|---:|---|
| `pycg-icse-2021.pdf` | PyCG: Practical Call Graph Generation in Python | 12 | Whole-program Python AST/import/points-to baseline. |
| `jarvis-python-call-graph-2023.pdf` | JARVIS: Scalable and Precise Application-Centered Call Graph Construction for Python | 12 | More recent Python application-centered analysis. |
| `static-js-call-graphs-comparative-2024.pdf` | Static JavaScript Call Graphs: a Comparative Study | 10 | Modern evidence that JS call graphs remain approximate and tool-dependent. |
| `java-call-graph-unsoundness-2026.pdf` | Detecting Call Graph Unsoundness without Ground Truth | 21 | Latest warning that even Java call graphs need explicit assumptions. |
| `soot-framework.pdf` | The Soot framework for Java program analysis: a retrospective | 8 | Classic JVM static-analysis architecture. |
| `sootup-tacas-2024.pdf` | SootUp: A Redesign of the Soot Static Analysis Framework | 18 | Modernized Soot API and analysis lifecycle. |
| `opal-unimocg-issta-2024.pdf` | Unimocg: Modular Call-Graph Algorithms for Consistent Handling of Language Features | 12 | Modular Java call graph construction and algorithm families. |
| `opal-totalrecall-issta-2024.pdf` | Total Recall? How Good Are Static Call Graphs Really? | 12 | Call graph recall and benchmarking perspective. |
| `spark-points-to-2003.pdf` | Scaling Java Points-To Analysis using SPARK | 16 | Practical Andersen-style Java points-to behind Soot Spark. |
| `codeql-navigating-call-graph.html` | Navigating the call graph | HTML | Query-facing `Callable` / `Call` / callee API. |
| `go-callgraph-package.html` | Go x/tools `go/callgraph` package docs | HTML | Practical Go algorithms: static, CHA, RTA, VTA. |
| `jarvis-project-page.html` | JARVIS project page | HTML | Project-level explanation and artifacts. |

Additional primary source URLs used:

- Go tools: https://github.com/golang/tools
- Go callgraph docs: https://pkg.go.dev/golang.org/x/tools/go/callgraph
- CodeQL call graph docs: https://codeql.github.com/docs/codeql-language-guides/navigating-the-call-graph/
- SootUp call graph docs: https://soot-oss.github.io/SootUp/latest/callgraphs/
- WALA call graph docs: https://wala.github.io/javadoc/
- OPAL publications: https://www.opal-project.de/Publications.html
- Jelly: https://github.com/cs-au-dk/jelly
- PyCG paper: https://arxiv.org/abs/2103.00587
- JARVIS paper: https://arxiv.org/abs/2305.05949
