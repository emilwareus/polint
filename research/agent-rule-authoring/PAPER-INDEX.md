# Paper And Source Index

This index records the papers and official sources used for the agent rule
authoring research. Local copies of PDFs are stored in `papers/` where
available.

## LLM And Agentic Static-Analysis Papers

| Source | Local file | Main lesson for polint |
|---|---|---|
| QLCoder, 2025. <https://arxiv.org/abs/2511.08462> | `papers/qlcoder-2025.pdf` | LLMs do better when query/rule synthesis is constrained by execution feedback, retrieval, and tool protocols. This supports inspect/test/diff loops, not prompt-only authoring. |
| KNighter, 2025. <https://arxiv.org/abs/2503.09002> | `papers/knighter-2025.pdf` | Specialized checkers can be synthesized from patch evidence and refined with false-positive feedback. Polint should support fixture-driven rule generation and repair. |
| IRIS, 2024. <https://arxiv.org/abs/2405.17238> | `papers/iris-llm-inferred-taint-specs-2024.pdf` | LLM-inferred taint specs can improve static analysis when the deterministic analyzer consumes and checks them. Polint models should be generated artifacts, not trusted facts by default. |
| SemTaint, 2026. <https://arxiv.org/abs/2601.10865> | `papers/semtaint-2026.pdf` | Combining static call/data facts with LLM-generated source/sink/unresolved-edge specs is stronger than either alone. Polint should expose unknowns as model/provider tasks. |
| RuleLLM, 2025. <https://arxiv.org/abs/2504.17198> | `papers/rulellm-2025.pdf` | LLM-generated Semgrep/YARA rules can be useful when measured for precision/recall. Polint should require generated rules to ship with positive/negative fixtures and evaluation deltas. |

## Official Documentation And Source References

| Source | Why it matters |
|---|---|
| CodeQL query docs. <https://codeql.github.com/docs/writing-codeql-queries/about-codeql-queries/> | Metadata, query kind, thin query over reusable libraries, and path-problem result model. |
| CodeQL path queries. <https://codeql.github.com/docs/writing-codeql-queries/creating-path-queries/> | First-class source-to-sink path diagnostics. |
| CodeQL JS/TS data flow. <https://codeql.github.com/docs/codeql-language-guides/analyzing-data-flow-in-javascript-and-typescript/> | Configuration-oriented source/sink/barrier/extra-step APIs and value-flow versus taint-flow distinction. |
| CodeQL model packs. <https://docs.github.com/en/code-security/tutorials/customize-code-scanning/creating-and-working-with-codeql-packs> | Pack distribution and data extensions for sources, sinks, summaries, and barriers. |
| Semgrep rule syntax. <https://semgrep.dev/docs/writing-rules/rule-syntax> | Code-shaped matching, metavariables, focus metavariables, and match/formula ergonomics. |
| Semgrep taint mode. <https://semgrep.dev/docs/writing-rules/data-flow/taint-mode/overview> | Sources, sinks, sanitizers, propagators, exactness, and capability limits. |
| Semgrep rule testing. <https://semgrep.dev/docs/writing-rules/testing-rules> | Inline positive/negative markers and `.fixed` expected-output tests. |
| ESLint custom rules. <https://eslint.org/docs/latest/extend/custom-rules> | Rule metadata, `create(context)`, diagnostics, fixes, suggestions, and `RuleTester`. |
| typescript-eslint custom rules. <https://typescript-eslint.io/developers/custom-rules/> | Type-safe rule creators, typed options/messages, and typed parser services. |
| Go `analysis`. <https://pkg.go.dev/golang.org/x/tools/go/analysis> | Explicit analyzer metadata, `Requires`, `ResultType`, `FactTypes`, diagnostics, and suggested fixes. |
| Go `analysistest`. <https://pkg.go.dev/golang.org/x/tools/go/analysis/analysistest> | Source-comment expectations and suggested-fix golden tests. |
| Joern traversal/data-flow docs. <https://docs.joern.io/traversal-basics/> and <https://docs.joern.io/cpgql/data-flow-steps/> | Fluent graph traversal and path evidence. Useful internally, too raw for first public SDK. |
| Joern data-flow semantics. <https://docs.joern.io/dataflow-semantics/> | Explicit argument-to-argument and argument-to-return library semantics. |
| Pysa basics, model DSL, and explorer. <https://pyre-check.org/docs/pysa-basics/> <https://pyre-check.org/docs/pysa-model-dsl/> <https://pyre-check.org/docs/pysa-explore/> | Model files, model generators, expected/unexpected models, and model debugging. |
| Ruff contributing, linter, preview, versioning. <https://docs.astral.sh/ruff/contributing/> <https://docs.astral.sh/ruff/linter/> <https://docs.astral.sh/ruff/preview/> <https://docs.astral.sh/ruff/versioning/> | Rust implementation discipline, snapshots, preview gates, and safe/unsafe fix applicability. |
| Clippy lint development. <https://doc.rust-lang.org/clippy/development/adding_lints.html> <https://doc.rust-lang.org/clippy/development/writing_tests.html> | Lint passes, diagnostics, applicability, `.stderr`, and `.fixed` UI tests. |
| OpenRewrite recipes and tests. <https://docs.openrewrite.org/concepts-and-explanations/recipes> <https://docs.openrewrite.org/authoring-recipes/recipe-testing> | Recipe metadata, visitor model, before/after transformation tests, and dry-run mindset. |

## Claims To Treat Carefully

- LLM-generated rules are not trusted because an LLM generated them. They are
  trusted only after compilation, fixture tests, validation, and benchmark
  deltas.
- Declarative models can change analysis behavior as much as code. They need
  provenance, applicability, validation, cache digests, and stale-model checks.
- Typed Rust rules have more friction than YAML/DSL rules. The product must
  compensate with scaffolding, manifests, examples, fast compilation, and
  excellent test failures.
