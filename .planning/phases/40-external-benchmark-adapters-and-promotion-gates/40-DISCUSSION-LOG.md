# Phase 40: External Benchmark Adapters and Promotion Gates - Discussion Log

**Gathered:** 2026-05-26
**Mode:** `$gsd-discuss-phase 40 --auto`

## Auto-Selected Gray Areas

[--auto] Selected all gray areas:

- Benchmark result shape
- Benchmark families and first targets
- Agent adaptation protocol
- Adapter and report architecture
- Promotion gates
- Public boundary and safety

## Decisions Captured

### Benchmark result shape

[auto] Q: "How should benchmark tables frame polint's results?" → Selected: "Three-way comparison: other scanner/product results, polint baseline, and polint agent-adapted." (recommended default)

Rationale: This matches the product thesis discussed with the user. Polint must be measurable as a default scanner and as an agent-adapted scanner, while staying comparable to products such as Semgrep, CodeQL, gosec, and suite-native baselines.

### Benchmark families and first targets

[auto] Q: "Are benchmarks only vulnerability-finding benchmarks?" → Selected: "No: include scanner outcome, graph/fact/path, and engine/adaptation benchmarks." (recommended default)

Rationale: Vulnerability benchmarks prove user-facing findings; graph/fact/path benchmarks prove CFG/call/data-flow/evidence substrate correctness; adaptation benchmarks prove repo-local extension value.

[auto] Q: "Which targets should come first?" → Selected: "Native fixtures plus supported-language smoke suites for TS/JS and Go." (recommended default)

Rationale: Current polint supports Go and TS/JS, so current benchmark scorecards and adaptation runs should stay inside those languages.

### Agent adaptation protocol

[auto] Q: "How should adapted scanner results be produced?" → Selected: "A separate adaptation agent writes repo-local rules/models/provider extensions using a recorded prompt and bounded budget." (recommended default)

Rationale: Adapted mode must represent the intended product workflow: a code-aware agent inspects a target repo and improves polint's analysis semantics through validated repo-local code.

[auto] Q: "What prevents benchmark gaming?" → Selected: "Record the prompt, forbid expected labels before adaptation, forbid case-id/filename hardcoding, and report cost/quality deltas." (recommended default)

Rationale: The adapted score is only credible if the artifact shows what the agent saw, what it changed, and whether the improvement created false positives, invalid facts, or unacceptable overhead.

### Adapter and report architecture

[auto] Q: "Should Phase 40 build a new harness?" → Selected: "No: extend the current crate-private `eval` module, native fixture model, provider run reports, and deterministic report hashing." (recommended default)

Rationale: The codebase already has an internal evaluation substrate from prior phases. Phase 40 should add external adapters, comparison records, adaptation artifacts, tiers, gates, and reports on top of it.

### Promotion gates

[auto] Q: "What gates are required?" → Selected: "Native fixture gates first, then tiered external suite gates with deterministic baseline comparison." (recommended default)

Rationale: External scanner suites do not validate provenance, extension merge safety, cache invalidation, unknowns, or public no-leak behavior. Native fixtures remain the engine-quality gate.

### Public boundary and safety

[auto] Q: "Should Phase 40 stabilize public eval/query APIs?" → Selected: "No: keep eval hidden/internal unless explicitly gated; Phase 41 owns public SDK/query ergonomics." (recommended default)

Rationale: Phase 40 generates evidence for promotion. Phase 41 decides which validated surfaces are safe to expose.

## Canonical References Added

- `.planning/ROADMAP.md`
- `.planning/REQUIREMENTS.md`
- `.planning/PROJECT.md`
- `.planning/STATE.md`
- `research/evaluation-harness/FINAL-REPORT.md`
- `research/evaluation-harness/RECOMMENDED_IMPLEMENTATION.md`
- `research/evaluation-harness/STANDARD.md`
- `research/evaluation-harness/RESEARCH-ANALYSIS.md`
- `research/evaluation-harness/implementation/polint-eval-path.md`
- `research/evaluation-harness/REPO-INDEX.md`
- `research/evaluation-harness/VALIDATION.md`
- `research/evaluation-harness/oss/benchmark-comparison.md`
- `research/evaluation-harness/algorithms/scoring-and-matching.md`
- `research/evaluation-harness/algorithms/benchmark-scheduling.md`
- `research/data-flow/VALIDATION.md`
- `research/call-graphs/VALIDATION.md`
- `research/program-slicing-evidence/VALIDATION.md`
- `.planning/phases/39-slicing-paths-and-evidence-bundles/39-CONTEXT.md`
- `.planning/phases/38-local-plus-summary-projected-data-flow/38-CONTEXT.md`
- `.planning/phases/37-refined-call-graph-providers/37-CONTEXT.md`
- `.planning/phases/36-p0-type-value-place-alias-substrate/36-CONTEXT.md`
- `.planning/phases/35-framework-entrypoints-and-trust-boundaries/35-CONTEXT.md`
- `.planning/phases/34-rust-extension-provider-sink/34-CONTEXT.md`
- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md`
- `.claude/skills/polint/SKILL.md`
- `research/agent-extension-surface/FINAL-REPORT.md`
- `research/agent-extension-surface/RECOMMENDED_IMPLEMENTATION.md`
- `research/agent-extension-surface/VALIDATION.md`
- `research/agent-rule-authoring/FINAL-REPORT.md`
- `research/agent-rule-authoring/RECOMMENDED_IMPLEMENTATION.md`

## Deferred Ideas

- Stable public eval/query APIs and SDK views remain Phase 41 or later.
- Benchmarks for languages without a polint frontend remain future language-adapter work.
- Full competitor reproduction and broad release-suite runs remain release/research tier after the first supported-language path works.
