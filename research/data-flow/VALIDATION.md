# Validation

Date: 2026-05-15

## What Was Validated

This research package was validated against:

- cloned OSS repositories under `research/data-flow/repos/`;
- exact clone commit hashes listed in `REPO-INDEX.md`;
- downloaded papers/docs under `research/data-flow/papers/`;
- local implementation paths referenced from the research notes;
- a fresh web check for late-2025 and 2026 data-flow work.

## Repository Clone Validation

The following repositories were cloned and inspected locally:

- CodeQL
- Semgrep
- OpenGrep
- Joern
- Pyre/Pysa
- FlowDroid
- Heros
- WALA
- Checker Framework
- Doop
- Souffle
- FlowLog
- TypeScript
- Go taint
- gosec
- NilAway
- YASA-UAST
- OpenTaint
- Cognium
- Salsa

The repo commits are recorded in `REPO-INDEX.md`. The cloned code is intentionally not committed because `research/data-flow/repos/` is gitignored.

Representative exact implementation paths referenced by the notes were also checked with `test -e`, including:

- `repos/codeql/shared/dataflow/codeql/dataflow/DataFlow.qll`
- `repos/codeql/shared/dataflow/codeql/dataflow/TaintTracking.qll`
- `repos/opengrep/src/analyzing/Dataflow_core.ml`
- `repos/opengrep/src/tainting/Dataflow_tainting.ml`
- `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/Engine.scala`
- `repos/pyre-check/source/interprocedural_analyses/taint/taintAnalysis.ml`
- `repos/heros/src/heros/IFDSTabulationProblem.java`
- `repos/FlowDroid/soot-infoflow/src/soot/jimple/infoflow/problems/InfoflowProblem.java`
- `repos/WALA/core/src/main/java/com/ibm/wala/dataflow/IFDS`
- `repos/checker-framework/dataflow/src/main/java/org/checkerframework/dataflow/analysis`
- `repos/doop/souffle-logic/main/main.dl`
- `repos/souffle/src/ast/analysis/SCCGraph.cpp`
- `repos/TypeScript/src/compiler/types.ts`
- `repos/go-taint/check.go`

## Paper and Document Validation

The downloaded PDF files were checked with `file`; they are recognized as PDFs:

- `adataint-llm-taint-2025.pdf`
- `code-property-graph-oakland-2014.pdf`
- `flowdroid-pldi-2014.pdf`
- `ifds-reps-horwitz-sagiv-1995.pdf`
- `ifds-taint-access-paths-2021.pdf`
- `incidfa-oopsla-2025.pdf`
- `mcp-biflow-2026.pdf`
- `poto-python-points-to-ecoop-2025.pdf`
- `scalable-compositional-taint-icse-2023.pdf`
- `semtaint-taint-spec-2026.pdf`
- `tainttyper-2025.pdf`
- `yasa-uast-taint-2026.pdf`

HTML docs were also downloaded for CodeQL, Semgrep, Pysa, Joern, and FlowLog. Source URLs are listed in `PAPER-INDEX.md`.

## Freshness Check

A fresh web search on 2026-05-15 found and incorporated:

- YASA, `arXiv:2601.17390`, multi-language taint over UAST;
- MCP-BiFlow, `arXiv:2605.07836`, bidirectional static data-flow for MCP ecosystems, submitted 2026-05-08;
- SemTaint, `arXiv:2601.10865`, LLM-assisted taint specification extraction;
- PoTo, ECOOP 2025, Python Andersen-style points-to analysis;
- IncIDFA, OOPSLA 2025, incremental iterative data-flow analysis;
- current CodeQL, Semgrep, and Pysa data-flow documentation.

This does not prove no newer paper exists anywhere, but it covers the relevant recent primary sources surfaced by web search for multi-language static data-flow and taint analysis.

## Bootstrap Integration Validation

Date: 2026-05-16

The data-flow implementation recommendation was revalidated against the current
polint codebase and the local cloned implementation sources before adding
`implementation/BOOTSTRAP-INTEGRATION.md`.

Current polint hooks checked:

- `crates/polint/src/sdk/facts.rs:650-736`: `DataFlow<'_>` is a reserved
  placeholder fact view.
- `crates/polint/src/analysis_plan.rs:637-642`: `dataflow` is still an
  unsupported capability.
- `crates/polint/src/core/mod.rs:142-153`: `FunctionFact.calls` is a string
  list, not a semantic call graph input.
- `crates/polint/src/core/mod.rs:1109-1127`: `Capabilities::dataflow` is
  documented as future facts built on CFG, symbols, and call graph support.
- `crates/polint-macros/src/lib.rs:318-327`: the rule macro maps `DataFlow` to
  the `dataflow` capability.

Reference implementation paths rechecked:

- CodeQL data-flow config, barriers, additional steps, access-path limits, and
  path graph contracts:
  `repos/codeql/shared/dataflow/codeql/dataflow/DataFlow.qll:400-451`,
  `:681-723`.
- CodeQL taint-as-layer design:
  `repos/codeql/shared/dataflow/codeql/dataflow/TaintTracking.qll:56-150`.
- CodeQL summary implementation:
  `repos/codeql/shared/dataflow/codeql/dataflow/internal/FlowSummaryImpl.qll:1769-1786`,
  `:1910-1938`.
- Joern sink-to-source path search, call-stack handling, and method semantics:
  `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/Engine.scala:40-54`,
  `:192-318`,
  `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/TaskCreator.scala:32-86`,
  `repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/semanticsloader/Semantics.scala:8-90`,
  `:111-176`.
- Pysa global taint fixpoint, per-callable CFG/call-graph/model analysis, callee
  model lookup, actual-to-formal matching, sanitizer matching, and forward local
  fixpoint extraction:
  `repos/pyre-check/source/interprocedural_analyses/taint/taintFixpoint.ml:8-52`,
  `:112-207`,
  `repos/pyre-check/source/interprocedural_analyses/taint/callModel.ml:23-52`,
  `:209-260`,
  `repos/pyre-check/source/interprocedural_analyses/taint/forwardAnalysis.ml:3740-3792`.
- OpenGrep deterministic generic fixpoint, taint precision caveats, lvalue
  environment, signatures, call graph, CFG transfer, and fixpoint call:
  `repos/opengrep/src/analyzing/Dataflow_core.ml:138-180`,
  `repos/opengrep/src/tainting/Dataflow_tainting.ml:55-67`,
  `:110-125`,
  `:2528-2684`,
  `:2801-2862`.
- Heros IFDS problem contract:
  `repos/heros/src/heros/IFDSTabulationProblem.java:21-60`.
- WALA tabulation problem and solver storage for path, call-flow, summary, seed,
  and call-processing edges:
  `repos/WALA/core/src/main/java/com/ibm/wala/dataflow/IFDS/TabulationProblem.java:20-48`,
  `repos/WALA/core/src/main/java/com/ibm/wala/dataflow/IFDS/TabulationSolver.java:100-152`,
  `:201-236`,
  `:500-548`.
- FlowDroid IFDS-style normal/call/return/call-to-return taint functions and
  alias/field/kill behavior:
  `repos/FlowDroid/soot-infoflow/src/soot/jimple/infoflow/problems/InfoflowProblem.java:72-190`,
  `:431-563`,
  `:776-832`.

Design conclusions validated by this pass:

- `DataFlow<'_>` should remain unsupported until internal facts are stable.
- Data flow must consume MIR, `PlaceId`, CFG, call targets, summaries, abstract
  domains, and extension model facts instead of owning parallel versions of
  those concepts.
- Taint must be a query/domain layer over general data-flow facts.
- Unknown/havoc facts must be queryable and cacheable.
- Summary-projected edges are the first scalable global layer.
- IFDS/IDE is a later internal solver once the ICFG and finite fact domains are
  stable.
- Path evidence should be query-scoped and call-context-aware.

## Accuracy Notes

- CodeQL and Semgrep product documentation is used for public API and design tradeoff claims.
- CodeQL, Semgrep/OpenGrep, Pysa, Joern, Heros, FlowDroid, WALA, Checker Framework, Doop, Souffle, and TypeScript claims are grounded in inspected local source paths listed in `REPO-INDEX.md` and `oss/implementation-comparison.md`.
- YASA, MCP-BiFlow, SemTaint, AdaTaint, PoTo, CFTaint, FlowDroid, IFDS, and CPG claims are grounded in downloaded papers/docs listed in `PAPER-INDEX.md`.
- OpenTaint and Cognium were treated as product signals only. Their public claims were not independently benchmarked.
- The `ifds-taint-access-paths-2021.pdf` local file is a short arXiv-rendered PDF. Re-download from the arXiv source before citing exact page numbers in formal documentation.
- The recommended architecture is an engineering synthesis, not a direct claim that any one source prescribes polint's design.
- Later product-path update: added agent-extensible data-flow modeling, repo-model provenance, validation status, call-graph model dependency, and default-vs-extended evaluation requirements. This is an architectural synthesis from the validated research plus polint's product direction, not a new paper claim.

## Research Metrics Extracted

The improved reports use the following metrics extracted with `pdftotext` from the downloaded papers:

- IFDS: general tabulation bound `O(E * D^3)` and locally separable bound `O(E * D)` from `ifds-reps-horwitz-sagiv-1995.pdf`.
- FlowDroid: 93% recall and 86% precision on DroidBench, plus reported InsecureBank and malware-sample timings from `flowdroid-pldi-2014.pdf`.
- IncIDFA: up to 11x update-time speedup, 2.6x geomean, up to 46% total compilation-time improvement, 15.1% geomean from `incidfa-oopsla-2025.pdf`.
- YASA: over 100M LOC, 7.3K applications, 314 paths, 92 confirmed 0-days, average 31.8 KLOC/min, with CodeQL and Joern comparison throughputs from `yasa-uast-taint-2026.pdf`.
- CFTaint: 96.09% recall, 93.51% sensitive-data precision, 127.31 second average time, and 3.86% of ANTaint's time from `scalable-compositional-taint-icse-2023.pdf`.
- MCP-BiFlow: 93.8% recall on 32 confirmed cases, 549 candidate clusters, 118 confirmed paths across 87 servers from `mcp-biflow-2026.pdf`.
- SemTaint: 106 of 162 CodeQL-missed vulnerabilities detected, 65.43% recall in that selected set, and unresolved-call candidate reduction from 94,909 to 10,184 from `semtaint-taint-spec-2026.pdf`.

## Validation Commands

Commands used during validation:

```sh
find research/data-flow -maxdepth 2 -type f | sort
file research/data-flow/papers/*.pdf | sort
for p in <representative source paths>; do test -e "$p"; done
git status --short
git diff --check
git check-ignore -v research/data-flow/repos/codeql research/data-flow/repos/joern research/data-flow/repos/pyre-check research/data-flow/repos/opengrep research/data-flow/repos/WALA research/data-flow/repos/heros research/data-flow/repos/FlowDroid
LC_ALL=C rg -n "[^\x00-\x7F]" research/data-flow/implementation/BOOTSTRAP-INTEGRATION.md research/data-flow/README.md research/data-flow/FINAL-REPORT.md research/data-flow/RECOMMENDED_IMPLEMENTATION.md research/data-flow/implementation/polint-data-flow-path.md research/data-flow/VALIDATION.md
```

## Residual Risks

- Some cloned repositories move quickly. Re-run clone/update validation before implementation begins.
- Java and Python language support require semantic lifecycle decisions that polint has not implemented yet.
- High-quality TS/JS data flow needs module resolution and dynamic feature handling beyond syntax facts.
- Interprocedural precision will be bounded by the call graph implementation.
- A native IFDS/IDE engine should wait until CFG and ICFG inputs are stable.
