# Research

This folder contains static-analysis research for polint.

Start with [ROADMAP.md](ROADMAP.md).

Existing research:

- [abstract-interpretation/](abstract-interpretation/): reduced-product abstract-domain kernel, lattice/transfer interfaces, widening/narrowing, nilness/nullish, constants, strings, ranges, initializedness, shapes, typestate/resource, extension validation, and benchmark strategy.
- [cfg-control-flow/](cfg-control-flow/): native operation/basic-block CFG facts, exceptional and cleanup edges, dominance/postdominance, control dependence, path evidence, and extension overlays.
- [module-graph/](module-graph/): native workspace roots, package managers, lockfiles, dependency edges, import-to-package resolution, source sets, build targets, and repo topology.
- [type-alias-points-to/](type-alias-points-to/): native type, value, place, points-to, and alias fact layers; bounded Andersen-style solver; alias provider stack; and agent extension hooks.
- [effects-summaries/](effects-summaries/): function effects and summaries as the scaling boundary for call graphs, data flow, alias queries, framework models, and agent-authored analysis extensions.
- [semantic-index/](semantic-index/): native scopes, symbols, imports, references, aliases, resolution facts, xref indexes, and export identity.
- [call-graphs/](call-graphs/): native call-site/call-edge facts, algorithm tiers, unresolved facts, and agent-extensible repo-local call models.
- [data-flow/](data-flow/): native data-flow substrate, summaries, source/sink/sanitizer models, call-graph dependency, and agent-extensible repo-local data-flow models.
- [agent-extension-surface/](agent-extension-surface/): Rust-code extension lifecycle, validation, provenance, capability planning, and default-vs-extended deltas.
- [analysis-kernel/](analysis-kernel/): fact layers, scheduling, provenance, precision, validation, extension merges, cache keys, and invalidation.
- [evaluation-harness/](evaluation-harness/): external-benchmark-first evaluation, ground truth, fixture schema, metrics, and regression gates.
- [framework-entrypoints/](framework-entrypoints/): framework/protocol boundaries, routes, jobs, callbacks, generated dispatch, MCP, and repo-local providers.
- [implementation-bootstrap/](implementation-bootstrap/): implementation-ready Rust bootstrap design for semantic MIR, place identity, direct call facts, P0 domains, direct summaries, minimal cache/invalidation, and extension sinks.
- [program-slicing-evidence/](program-slicing-evidence/): native slicing and evidence layer for diagnostics: PDG/SDG lessons, thin slices, chops, path ranking, JSON/SARIF evidence, provenance, unknowns, and extension merges.
