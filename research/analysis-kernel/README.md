# Analysis Kernel Research

Date: 2026-05-15

This folder researches the internal analysis kernel polint should build before adding serious entrypoint, CFG, call graph, data-flow, effects, slicing, and agent-extension features.

The kernel is not a public rule API. It is the internal substrate that decides:

- which fact families exist;
- which providers can produce them;
- which layers are native, derived, extension-provided, synthetic, or heuristic;
- which facts are exact, conservative, heuristic, lossy, or unknown;
- how facts carry provenance and validation;
- how providers are scheduled;
- how extension facts merge into native facts;
- how cache keys and invalidation work.

## Conclusion

Build a **hybrid analysis kernel**:

```text
deterministic provider DAG
  + typed fact-family registry
  + layer manifests
  + sidecar provenance and validation
  + relation/fixpoint sub-engine for recursive analyses
  + content-addressed layer cache
  + extension merge gates
```

Do not start by adopting Salsa, Souffle, CodeQL, a graph database, or a public query language wholesale.

Instead, copy the parts that matter:

- from Salsa and rust-analyzer: revisions, dependency edges, durability, backdating, snapshots, LRU pressure, invalidation barriers;
- from Souffle, Doop, CodeQL, and FlowLog: typed relations, SCC/fixpoint scheduling, semi-naive deltas, indexes by access pattern, extraction database discipline;
- from gopls and TypeScript: persistent recipe keys, affected-file/package scheduling, file shape digests, stable build info;
- from Pyre: explicit value and presence dependencies;
- from WALA: typed analysis products rather than one public global graph;
- from Joern: named overlays/layers, but with stricter planning and validation;
- from CodeQL and SARIF: path/evidence export and model provenance;
- from Kythe and SCIP: stable cross-language entity names, source anchors, and separate occurrence/symbol/relationship layers.

## Why This Matters

polint is not trying to be a sealed universal static analyzer. The target user includes AI coding agents that can inspect a repo and write repo-local Rust extensions. That changes the kernel design.

Classic tools try to hide missing framework knowledge behind heuristics. polint should expose uncertainty as typed facts, then let agents write validated extensions that add codebase-specific knowledge. That only works if every fact has identity, provenance, precision, validation, dependencies, and stable cache behavior.

Without this kernel, call graphs, data flow, entrypoints, and effects will each invent their own lifecycle, their own "unknown" model, their own cache keys, and their own extension merge behavior. That would build the project into a corner.

## Files

- `FINAL-REPORT.md`: executive synthesis and recommended direction.
- `RECOMMENDED_IMPLEMENTATION.md`: concrete implementation path for polint.
- `RESEARCH-ANALYSIS.md`: deeper technical analysis and tradeoffs.
- `STANDARD.md`: shared vocabulary and schema for facts, layers, provenance, validation, scheduling, and cache keys.
- `REPO-INDEX.md`: OSS repositories inspected and local clone locations.
- `PAPER-INDEX.md`: papers and documentation downloaded or referenced.
- `VALIDATION.md`: checks the kernel should enforce.
- `SUBAGENT-FINDINGS.md`: how the parallel research agents contributed to the synthesis.
- `oss/implementation-comparison.md`: comparison of Salsa, rust-analyzer, Souffle, Doop, CodeQL, WALA, Joern, gopls, TypeScript, Pyre, FlowLog, Kythe, SCIP, OpenRewrite.
- `algorithms/provider-scheduling.md`: provider DAG and fixpoint scheduling pseudocode.
- `algorithms/cache-invalidation.md`: cache key and invalidation pseudocode.
- `algorithms/extension-merge-validation.md`: validation and merge pseudocode.
- `decisions/decision-log.md`: decision paths, rejected alternatives, and rationale.

## Immediate Product Recommendation

The first implementation slice should be:

1. Add an internal `analysis_kernel` module.
2. Move the current runner sequence into a provider DAG without changing behavior.
3. Add provider manifests for Go syntax, TS syntax, module graph, symbol graph, and metrics.
4. Add layer cache keys separated from rule execution keys.
5. Add sidecar provenance for facts, starting with default native provenance.
6. Add validation and deterministic merge gates.
7. Plug the first extension-provided fact family into the kernel, preferably `Entrypoints<'_>`.

This gives polint a stable spine before adding call graph or data-flow facts.
