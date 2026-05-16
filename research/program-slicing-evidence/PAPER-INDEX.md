# Paper And Documentation Index

Access date for web material: 2026-05-16.

## Downloaded Papers

### Ferrante, Ottenstein, Warren: The Program Dependence Graph And Its Use In Optimization

- File: `papers/ferrante-ottenstein-warren-pdg-1987.pdf`
- Source URL: <https://bears.ece.ucsb.edu/class/ece253/papers/ferrante87.pdf>
- Authors: Jeanne Ferrante, Karl J. Ottenstein, Joe D. Warren
- Publication date: 1987
- Source type: paper
- Status: downloaded, summarized
- Short note: Defines the Program Dependence Graph as explicit data and control
  dependence over operations. The paper is foundational for slicing, debugging,
  optimization, and incremental update reasoning. It also makes the practical
  point that exact control/data dependence is not generally decidable, so useful
  tools rely on conservative approximations.

### Horwitz, Reps, Binkley: Interprocedural Slicing Using Dependence Graphs

- File: `papers/horwitz-reps-binkley-interprocedural-slicing-1990.pdf`
- Source URL: <https://www.cs.purdue.edu/homes/xyzhang/fall07/Papers/p26-horwitz.pdf>
- Authors: Susan Horwitz, Thomas Reps, David Binkley
- Publication date: 1990
- Source type: paper
- Status: downloaded, summarized
- Short note: Defines System Dependence Graph slicing and the summary-edge
  machinery needed to make interprocedural slices respect calling context. This
  is the core warning against naive graph reachability across call/return edges.

### Sridharan, Fink, Bodik: Thin Slicing

- File: `papers/sridharan-fink-bodik-thin-slicing-2007.pdf`
- Source URL: <https://manu.sridharan.net/files/pldi07.pdf>
- Authors: Manu Sridharan, Stephen J. Fink, Rastislav Bodik
- Publication date: 2007
- Source type: paper
- Status: downloaded, summarized
- Short note: Shows that traditional slices can be too large for people. Thin
  slicing keeps producer/value statements first and lets users expand into full
  slices later. The evaluation reports that desired statements were found after
  inspecting 3.3x fewer statements for debugging and 9.4x fewer for program
  understanding.

### Tip-Style Survey: Program Slicing Techniques And Applications

- File: `papers/program-slicing-techniques-applications-2011.pdf`
- Source URL: <https://arxiv.org/pdf/1108.1352>
- arXiv URL: <https://arxiv.org/abs/1108.1352>
- Authors: N. Sasirekha, A. Edwin Robert, M. Hemalatha
- Publication date: 2011-07
- Source type: paper
- Status: downloaded, summarized
- Short note: Useful taxonomy of static, dynamic, conditioned, amorphous,
  forward, backward, and chop-style slicing.

### SliceFormer: Dataflow-Aware Pretraining For Static Slicing

- File: `papers/sliceformer-2026.pdf`
- Source URL: <https://arxiv.org/pdf/2604.26961>
- arXiv URL: <https://arxiv.org/abs/2604.26961>
- Authors: Pengfei He, Shaowei Wang, Tse-Hsun Chen, Muhammad Asaduzzaman
- Publication date: 2026-05-10 arXiv v2
- Source type: paper
- Status: downloaded, summarized
- Short note: Treats slicing as a sequence-to-sequence task with dataflow-aware
  pretraining and constrained decoding. The abstract reports up to 22% gain in
  ExactMatch. This is useful for benchmark awareness, but not a substitute for
  trusted native graph facts.

### SliceMate: Effective Static Program Slicing With LLM-Powered Agents

- File: `papers/slicemate-2025.pdf`
- Source URL: <https://arxiv.org/pdf/2507.18957>
- arXiv URL: <https://arxiv.org/abs/2507.18957>
- Authors: Jianming Chang, Jieke Shi, Yunbo Lyu, Xin Zhou, Lulu Wang, Zhou Yang, Bixin Li, David Lo
- Publication date: 2025-07-25 arXiv v1
- Source type: paper
- Status: downloaded, summarized
- Short note: Agentic slicing that synthesizes, verifies, and refines slices
  without explicit dependency graph construction. Introduces SliceBench with
  2,200 manually annotated Java/Python programs. Useful as evaluation
  inspiration and as a reminder that agents can repair or extend analysis, but
  polint should not treat LLM-generated slices as trusted native facts.

### SliceT5: Sequence-To-Sequence Static Program Slicing

- File: `papers/slicet5-2025.pdf`
- Source URL: <https://arxiv.org/pdf/2509.17338>
- arXiv URL: <https://arxiv.org/abs/2509.17338>
- Authors: Pengfei He, Shaowei Wang, Tse-Hsun Chen
- Publication date: 2025-09-22 arXiv v1
- Source type: paper
- Status: downloaded, summarized
- Short note: Java-focused sequence-to-sequence slicing with copy mechanism and
  constrained decoding, targeting incomplete or unparsable snippets. Lesson for
  polint: constrained extraction from existing spans is safer than generated
  free text.

## Official Documentation And Specs

### CodeQL: Creating Path Queries

- Source URL: <https://codeql.github.com/docs/writing-codeql-queries/creating-path-queries/>
- Publisher / project: GitHub CodeQL
- Source type: official docs
- Status: summarized
- Short note: Explains the path-query convention around path problem metadata,
  `PathNode`, `edges`, and selecting source/sink/path nodes. This aligns with
  the CodeQL source inspection of `PathGraphSig`, `PathNode`, `flowPath`, and
  `subpaths`.

### CodeQL: About Data Flow Analysis

- Source URL: <https://codeql.github.com/docs/writing-codeql-queries/about-data-flow-analysis/>
- Publisher / project: GitHub CodeQL
- Source type: official docs
- Status: summarized
- Short note: Official orientation for local/global data flow, taint tracking,
  sources, sinks, and path graph output.

### OASIS SARIF v2.1.0

- Source URL: <https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html>
- Publisher / project: OASIS
- Publication date: 2020
- Source type: spec
- Status: summarized
- Short note: `codeFlows` and `threadFlows` are the interoperable output target
  for path explanations. polint should keep a richer internal evidence model and
  render a subset into SARIF.

### Joern Data-Flow Steps

- Source URL: <https://docs.joern.io/cpgql/data-flow-steps/>
- Publisher / project: Joern
- Source type: official docs
- Status: summarized
- Short note: Describes `reachableBy` and `reachableByFlows`, matching the local
  source-code inspection of Joern's visible data-flow paths.

### Joern CPG Slicing

- Source URL: <https://docs.joern.io/cpg-slicing/>
- Publisher / project: Joern
- Source type: official docs
- Status: summarized
- Short note: Useful public-facing reference for CPG slice output and data-flow
  slice behavior.

### Semgrep Taint Mode Documentation

- Source URL: <https://semgrep.dev/docs/writing-rules/data-flow/taint-mode/overview>
- Publisher / project: Semgrep
- Source type: official docs
- Status: summarized
- Short note: Documents user-facing source/sink/propagator/sanitizer concepts.
  Source inspection shows how traces become JSON/text/SARIF-oriented output.

### Frama-C Slicing Plugin Documentation

- Source URL: <https://www.frama-c.com/fc-plugins/slicing.html>
- Publisher / project: Frama-C
- Source type: official docs
- Status: summarized
- Short note: Documents slicing as a plugin over PDG-style dependencies. Local
  source inspection shows selection marks and propagation through call structure.

## Source Status Checks

Downloaded PDFs were verified locally with `file` on 2026-05-16. Cloned
implementation snapshots are indexed in `REPO-INDEX.md`; the clone directory is
ignored by git and is not part of the committed source tree.
