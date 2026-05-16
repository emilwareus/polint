# Subagent Findings And Review Notes

No subagents were spawned during this continuation because the active user
request did not explicitly ask for parallel agents. This file records the
second-pass review notes for future agents.

## First-Pass Evidence Base

Implementation source was inspected from:

- WALA SDG/PDG slicer and IFDS tabulation solver;
- CodeQL data-flow path graph and JavaScript path summaries;
- Joern data-flow slicing, query engine, task fingerprints, and semantics;
- Semgrep taint trace analysis and renderers;
- Frama-C slicing and PDG mark propagation;
- JavaSlicer SDG/ICFG construction.

Research papers were downloaded for:

- Program Dependence Graphs;
- Interprocedural slicing with dependence graphs;
- Thin slicing;
- slicing taxonomy;
- recent neural and LLM-agent slicing.

## Second-Pass Critique

Potential weak claim: "state of the art" can imply one best system.

Resolution: The reports describe state of the art as a layered design rather
than one algorithm. Different tools are strongest in different dimensions:
WALA for SDG slicing, CodeQL for path rendering, Joern for CPG query paths and
semantics overlays, Semgrep for practical traces, Frama-C for criteria/marks,
and recent papers for ML/agent benchmark direction.

Potential weak claim: "thin slices should be default."

Resolution: Thin slicing is recommended as the default display mode, not the
only analysis. Full slices and expansion remain required. This follows the thin
slicing paper's human-inspection result and avoids misleading users by recording
omitted edge classes.

Potential weak claim: "LLM slicers are not trusted."

Resolution: The rejection is scoped. LLM/agent slicers can be benchmark
oracles, extension authors, candidate slice generators, and comparison systems.
They should not be the trusted native fact source for diagnostics without
validation.

Potential missing dependency: slicing needs existing graph facts.

Resolution: Implementation path now explicitly depends on semantic operation
ids, CFG/control dependence, def-use/data dependence, direct call facts,
summaries, and data-flow facts.

Potential output trap: SARIF may force evidence shape.

Resolution: Reports recommend richer internal evidence and lossy SARIF
rendering, preserving JSON/debug detail separately.

## Remaining Research Questions

- How should path predicates from future abstract interpretation domains be
  rendered?
- What exact JSON schema should `evidence_v2` use?
- How should evidence query caching interact with a future incremental query
  engine?
- Which slicing benchmark source can be legally vendored or adapter-driven for
  polint's harness?
