# Research

This folder contains static-analysis research for polint.

Start with [ROADMAP.md](ROADMAP.md).

Existing research:

- [cfg-control-flow/](cfg-control-flow/): native operation/basic-block CFG facts, exceptional and cleanup edges, dominance/postdominance, control dependence, path evidence, and extension overlays.
- [module-graph/](module-graph/): native workspace roots, package managers, lockfiles, dependency edges, import-to-package resolution, source sets, build targets, and repo topology.
- [semantic-index/](semantic-index/): native scopes, symbols, imports, references, aliases, resolution facts, xref indexes, and export identity.
- [call-graphs/](call-graphs/): native call-site/call-edge facts, algorithm tiers, unresolved facts, and agent-extensible repo-local call models.
- [data-flow/](data-flow/): native data-flow substrate, summaries, source/sink/sanitizer models, call-graph dependency, and agent-extensible repo-local data-flow models.
- [agent-extension-surface/](agent-extension-surface/): Rust-code extension lifecycle, validation, provenance, capability planning, and default-vs-extended deltas.
- [analysis-kernel/](analysis-kernel/): fact layers, scheduling, provenance, precision, validation, extension merges, cache keys, and invalidation.
- [evaluation-harness/](evaluation-harness/): external-benchmark-first evaluation, ground truth, fixture schema, metrics, and regression gates.
- [framework-entrypoints/](framework-entrypoints/): framework/protocol boundaries, routes, jobs, callbacks, generated dispatch, MCP, and repo-local providers.
