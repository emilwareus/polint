# Paper and Document Index

Downloaded papers and docs live under `research/data-flow/papers/`.

| Local file | Source | Why it matters |
|---|---|---|
| `papers/yasa-uast-taint-2026.pdf` | https://arxiv.org/abs/2601.17390 | Recent multi-language industrial taint paper using unified AST, points-to, taint propagation, and language-specific semantics across Java, JavaScript, Python, and Go. |
| `papers/mcp-biflow-2026.pdf` | https://arxiv.org/abs/2605.07836 | Very recent bidirectional static data-flow paper for MCP servers. Important because it shows that agent-era rule engines need domain entrypoint recovery, protocol-specific models, and request/return-side flows. |
| `papers/semtaint-taint-spec-2026.pdf` | https://arxiv.org/abs/2601.10865 | LLM-assisted extraction of sources, sinks, call edges, and library summaries for JavaScript packages. Relevant to AI-agent-authored rule workflows. |
| `papers/incidfa-oopsla-2025.pdf` | https://www.cse.iitm.ac.in/~krishna/preprints/oopsla25/oopsla25.pdf | Recent incremental iterative data-flow analysis algorithm. Important for future cache/invalidation design. |
| `papers/poto-python-points-to-ecoop-2025.pdf` | https://arxiv.org/abs/2409.03918 | Andersen-style points-to analysis for Python, including hybrid concrete evaluation for external library calls and call graph/type inference clients. |
| `papers/ifds-taint-access-paths-2021.pdf` | https://arxiv.org/abs/2103.16240 | Demand-driven IFDS taint with access paths for large Java web applications. |
| `papers/tainttyper-2025.pdf` | https://arxiv.org/abs/2504.18529 | Type-based taint checking and inference; useful for modular/incremental alternatives to whole-program taint. |
| `papers/adataint-llm-taint-2025.pdf` | https://arxiv.org/abs/2511.04023 | LLM-assisted source/sink identification and false-positive mitigation grounded in static facts. Relevant to AI-agent-authored rules. |
| `papers/scalable-compositional-taint-icse-2023.pdf` | https://yuleisui.github.io/publications/icse23.pdf | Summary/compositional taint design for industrial microservices. |
| `papers/flowdroid-pldi-2014.pdf` | https://orbilu.uni.lu/handle/10993/20223 | Classic high-precision Android taint analysis: context-, flow-, field-, object-sensitive with lifecycle modeling. |
| `papers/ifds-reps-horwitz-sagiv-1995.pdf` | https://web.stanford.edu/class/archive/cs/cs295/cs295.1086/papers/p49-reps.pdf | Foundational IFDS graph-reachability paper. |
| `papers/code-property-graph-oakland-2014.pdf` | https://www.ieee-security.org/TC/SP2014/papers/ModelingandDiscoveringVulnerabilitieswithCodePropertyGraphs.pdf | Original code property graph vulnerability-mining paper. |
| `papers/codeql-dataflow-cpp.html` | https://codeql.github.com/docs/codeql-language-guides/analyzing-data-flow-in-cpp/ | CodeQL local/global data-flow and taint query model. |
| `papers/codeql-dataflow-go.html` | https://codeql.github.com/docs/codeql-language-guides/analyzing-data-flow-in-go/ | CodeQL Go local/global data-flow and taint API. Useful for matching Go fact ergonomics. |
| `papers/codeql-dataflow-typescript-javascript.html` | https://codeql.github.com/docs/codeql-language-guides/analyzing-data-flow-in-javascript-and-typescript/ | CodeQL JS/TS local/global data-flow API and source-node model. Useful for TS/JS SDK design. |
| `papers/semgrep-dataflow-overview.html` | https://semgrep.dev/docs/writing-rules/data-flow/data-flow-overview | Semgrep documented design tradeoffs: intraprocedural, lightweight, no path sensitivity, no soundness guarantee. |
| `papers/semgrep-taint-mode-overview.html` | https://semgrep.dev/docs/writing-rules/data-flow/taint-mode/overview | Rule-author-facing source/sink/sanitizer/propagator model and taint traces. |
| `papers/pysa-implementation-details.html` | https://pyre-check.org/docs/pysa-implementation-details/ | Pysa summaries, TITO, source/sink summaries, and global fixed point. |
| `papers/joern-code-property-graph.html` | https://docs.joern.io/code-property-graph/ | CPG representation combining syntax, control flow, and data dependencies. |
| `papers/flowlog-2025.html` | https://www.flowlog-rs.com/ | Rust incremental Datalog-style system; relevant for native relational/fixed-point design. |

## Other Research Used

The following were used via web search or subagent synthesis and should be revisited before implementation:

- Sagiv/Reps/Horwitz, "Precise Interprocedural Dataflow Analysis with Applications to Constant Propagation" (IDE).
- "Optimal and Perfectly Parallel Algorithms for On-Demand Data-Flow Analysis".
- "Parameterized Algorithms for Scalable Interprocedural Data-flow Analysis" (`arXiv:2309.11298`).
- "Demanded Abstract Interpretation".
- "Context Sensitivity without Contexts".
- "Better Not Together: Staged Solving for Context-Free Language Reachability".
- "Program Analysis via Multiple Context Free Language Reachability".
- "Scaling Inter-procedural Dataflow Analysis on the Cloud".
- "Artemis: Toward Accurate Detection of Server-Side Request Forgeries through LLM-Assisted Inter-Procedural Path-Sensitive Taint Analysis".
