# Agent Rule Authoring Research

This folder researches how polint should expose rule SDKs, query ergonomics,
models, provider extensions, tests, packaging, and AI-agent authoring workflows.

The practical conclusion is:

```text
typed Rust rules first
  + macro-derived capability manifests
  + narrow RuleCtx
  + typed fact views
  + domain-specific query builders
  + declarative model packs
  + process-isolated provider extensions
  + fixture-first tests
  + inspect/explain/diff tooling for agents
```

Do not build a CodeQL clone, Semgrep YAML clone, Joern raw graph shell, or
Datalog query language as the first authoring surface. Those systems contain
important lessons, but polint's differentiator is repo-local Rust code over
typed native facts, with agents able to write rules, models, summaries, and
provider extensions that the engine can validate and measure.

## Reports

- [FINAL-REPORT.md](FINAL-REPORT.md): executive research report and decision.
- [RECOMMENDED_IMPLEMENTATION.md](RECOMMENDED_IMPLEMENTATION.md): concrete
  implementation plan.
- [RESEARCH-ANALYSIS.md](RESEARCH-ANALYSIS.md): deeper system comparison and
  risk analysis.
- [REPO-INDEX.md](REPO-INDEX.md): OSS repositories cloned and inspected.
- [PAPER-INDEX.md](PAPER-INDEX.md): research papers and official docs.
- [STANDARD.md](STANDARD.md): vocabulary and review checklist.
- [SUBAGENT-FINDINGS.md](SUBAGENT-FINDINGS.md): parallel research synthesis.
- [VALIDATION.md](VALIDATION.md): validation notes and commands.

## Supporting Notes

- [algorithms/agent-authoring-loop.md](algorithms/agent-authoring-loop.md):
  pseudocode for the agent workflow, rule manifest generation, and test runner.
- [implementation/POLINT-RULE-SDK-AUTHORING.md](implementation/POLINT-RULE-SDK-AUTHORING.md):
  implementation-ready SDK and CLI plan.
- [decisions/001-typed-rust-rules-not-dsl-first.md](decisions/001-typed-rust-rules-not-dsl-first.md):
  architecture decision record.
- [benchmarks/rule-authoring-evaluation-plan.md](benchmarks/rule-authoring-evaluation-plan.md):
  evaluation plan for authoring speed, correctness, and model/provider deltas.

## Core Product Rule

Rules are not prompts. Models are not comments. Provider extensions are not
unchecked plugins.

Every artifact an agent writes should be executable, typed, testable,
provenance-labeled, cache-keyed, and explainable.
