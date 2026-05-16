# Subagent Findings

Eight successful subagents contributed to this research pass.

## CodeQL

Findings:

- CodeQL query files are usually thin wrappers over reusable libraries.
- Query metadata matters; `@kind path-problem` changes result interpretation.
- Path queries select alert, source, sink, and message, not just a flat
  diagnostic.
- Model packs are relevant for sources, sinks, summaries, barriers, and guards.
- Test packs and exact expected output are central to custom query workflows.

Polint lesson:

- copy metadata, path diagnostics, model packs, and exact tests;
- do not invent QL as the first public API.

## Semgrep

Findings:

- Code-shaped patterns, metavariables, and `focus-metavariable` are major
  authoring wins.
- Taint mode has a clear source/sink/sanitizer/propagator vocabulary.
- Inline `ruleid`/`ok` markers and `.fixed` tests provide fast feedback.
- Hidden taint defaults and capability boundaries can surprise users.

Polint lesson:

- provide scaffolding, focused diagnostic spans, taint vocabulary, inline
  markers, and fixed-output tests;
- keep the primary power surface as Rust typed facts.

## Graph Query Systems

Findings:

- Joern's fluent traversal and `reachableByFlows` are powerful.
- Kythe/SCIP/LSIF are better lessons for stable IDs, anchors, deterministic
  indexes, and precomputed code intelligence than for rule authoring.
- Raw graph traversal can expose too much unstable schema.

Polint lesson:

- build graph-shaped internals and typed relationship views;
- expose structured evidence paths;
- avoid raw graph DSL first.

## Pysa, Infer/Pulse, Model Systems

Findings:

- Rules should stay policy-level.
- Models describe API behavior.
- Extensions should be reserved for new semantics or fact recovery.
- Pysa-style model exploration and expected/unexpected model tests are useful.

Polint lesson:

- separate rules, models, summaries, and provider extensions;
- validate models strictly and expose provenance/confidence.

## Mainstream Lint SDKs

Findings:

- ESLint uses metadata plus `create(context)` and `context.report`.
- typescript-eslint adds typed rule creators and typed options/messages.
- Go `analysis` has explicit `Requires`, `ResultType`, and `FactTypes`.
- Ruff and Clippy show preview gates, diagnostics, fix applicability, and
  snapshots.
- OpenRewrite is the best source for before/after rewrite tests.

Polint lesson:

- keep `RuleCtx` narrow but useful;
- generate an inspectable manifest;
- reserve structured fixes and applicability.

## AI-Agent Authoring

Findings:

- Recent LLM/static-analysis papers favor deterministic analyzers with
  execution feedback, retrieval, and validation loops.
- Agents need inspectable facts, unknowns, capability plans, and deltas.

Polint lesson:

- add inspect/explain/diff/eval commands before public advanced providers.

## Adversarial Review

Findings:

- Typed Rust is coherent, but friction is the main risk.
- Macro-derived capabilities need an inspectable manifest.
- `RuleCtx` must not be too anemic: diagnostics, options, paths, setup status,
  and future fixes belong there.
- Capability names need precision metadata.
- Model/provider boundaries must be sharper.

Polint lesson:

- rule manifest, fact precision labels, and fixture tooling are mandatory.

## Testing/Packaging

Findings:

- `polint test` should be a fixture runner, not just `cargo test`.
- Combine inline markers, JSON snapshots, before/after fix tests, model deltas,
  and provider fact snapshots.
- Keep one rule-pack crate per repo by default.

Polint lesson:

- new rules should generate tests by default;
- model/provider packs need their own test shapes.
