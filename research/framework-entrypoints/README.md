# Framework, Lifecycle, And Entrypoint Research

Date: 2026-05-15

This folder researches how polint should recover framework and protocol boundaries: HTTP routes, middleware, lifecycle callbacks, jobs, queues, CLI commands, tests, MCP tools/resources/prompts, serverless handlers, decorators, annotations, generated dispatch, and project-specific framework wrappers.

The important conclusion is narrow:

```text
Recover framework/protocol boundary facts with provenance.
Do not claim exact runtime behavior.
Feed validated boundary facts into call graph and data flow as optional overlays.
Let repo-local Rust providers add project-specific knowledge.
```

## Deliverables

| File | Purpose |
|---|---|
| `FINAL-REPORT.md` | Main research synthesis and executive decision. |
| `RECOMMENDED_IMPLEMENTATION.md` | Concrete native Rust implementation path for polint. |
| `STANDARD.md` | Standard vocabulary and fact schema for comparing implementations. |
| `REPO-INDEX.md` | Index of OSS repositories cloned and studied. |
| `PAPER-INDEX.md` | Index of papers downloaded and how each supports the design. |
| `VALIDATION.md` | Accuracy, complexity, benchmark, and fixture strategy. |
| `SUBAGENT-REVIEWS.md` | Synthesis of the parallel research and secondary review waves. |
| `algorithms/framework-entrypoint-recovery.md` | Language-neutral pseudo-code for entrypoint/lifecycle recovery. |
| `implementation/native-rust-path.md` | Provider DAG, extension, merge, cache, and SDK path. |
| `oss/implementation-comparison.md` | Comparison of CodeQL, Pysa, FlowDroid, Semgrep, F4F, AutoWeb, CGMiner, MCP-BiFlow. |
| `benchmarks/evaluation-plan.md` | External benchmark and native fixture plan for this research track. |
| `languages/*.md` | Go, TS/JS, Java/JVM, Python framework notes. |
| `decisions/DECISIONS.md` | Decision log and rejected alternatives. |

Third-party source checkouts live in `research/framework-entrypoints/repos/`, which is gitignored. Papers live in `research/framework-entrypoints/papers/`.

## How This Fits The Existing Research

This track depends on and refines:

- `research/analysis-kernel/`: fact layers, provider DAG, provenance, validation, cache keys.
- `research/evaluation-harness/`: external-benchmark-first validation and default-vs-extension metrics.
- `research/call-graphs/`: call-site/call-edge facts, unresolved calls, synthetic framework edges.
- `research/data-flow/`: source/sink/sanitizer/summary model layer and path evidence.
- `research/agent-extension-surface/`: repo-local Rust analysis providers and validated extension facts.

The first implementation should not jump straight to global call graph or global data flow. It should prove the shared kernel with a small but valuable fact family:

```text
Entrypoints<'_>
  + TrustBoundaries<'_>
  + FrameworkDispatch overlays
  + validation fixtures
  + agent-authored Rust provider support
```
