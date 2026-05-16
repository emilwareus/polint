# Paper And Documentation Index

This topic is mostly systems research: the state of the art is a combination of academic program-analysis frameworks and production extension mechanisms.

## Program Analysis Research

| Source | Relevance To This Track |
|---|---|
| Reps, Horwitz, Sagiv, "Precise Interprocedural Dataflow Analysis via Graph Reachability" | The IFDS model motivates a fact/summaries architecture where extensions contribute sources, sinks, barriers, and summaries without mutating solver internals. |
| Yamaguchi et al., "Modeling and Discovering Vulnerabilities with Code Property Graphs" | Shows the power of a shared cross-language graph substrate for syntax, control flow, and data flow. Joern's later overlay model is especially relevant for extension layering. |
| Bravenboer and Smaragdakis, "Strictly Declarative Specification of Sophisticated Points-to Analyses" | Datalog-style analysis proves that high-level relation extension can be powerful, but polint should not make a Datalog DSL its first public surface. |
| Souffle Datalog documentation and related program-analysis literature | Useful for future query-planning and solver internals; less appropriate as the first user extension surface because the user explicitly wants Rust code. |

## Official Documentation And OSS Sources

| System | Source | Main Lesson |
|---|---|---|
| Dylint | <https://github.com/trailofbits/dylint> | Rust dynamic lint libraries can work, but require exact toolchain management and in-process loading. |
| Rustc driver | <https://rustc-dev-guide.rust-lang.org/rustc-driver/intro.html> | Compiler callbacks are powerful but unstable and too low-level for polint's public API. |
| Clippy lint development | <https://doc.rust-lang.org/stable/clippy/development/adding_lints.html> | Good lint metadata, pass phases, UI testing, and documentation discipline. |
| Cargo external tools | <https://doc.rust-lang.org/cargo/reference/external-tools.html> | Stable command discovery pattern for Rust executables. |
| Error Prone plugins | <https://errorprone.info/docs/plugins> | Custom checks are loaded by `ServiceLoader` and implemented like built-ins, with semantic compiler state. |
| OpenRewrite recipes | <https://docs.openrewrite.org/concepts-and-explanations/recipes> | Strong managed lifecycle, validation, metadata, and scanning recipes. |
| ESLint custom rules | <https://eslint.org/docs/latest/extend/custom-rules> | Excellent rule ergonomics and fixture testing, but dynamic/stringly for Rust. |
| CodeQL custom library models | <https://codeql.github.com/docs/codeql-language-guides/customizing-library-models-for-cpp/> | Strong model taxonomy: sources, sinks, summaries, barriers, provenance. |
| Pysa model generators | <https://pyre-check.org/docs/pysa-model-generators/> | Project-specific model generation before analysis is a direct match for agent-authored extensions. |
| Pysa model DSL | <https://pyre-check.org/docs/pysa-model-dsl/> | Good examples of query-generated models and expected/unexpected model validation. |
| TypeScript language service plugins | <https://www.typescriptlang.org/tsconfig/plugins.html> | Plugins can augment editor intelligence but should not be confused with CLI/CI analysis semantics. |
| Joern docs | <https://docs.joern.io/> | Cross-language code property graph, custom query model, and extensible passes/overlays. |
| CPG specification | <https://cpg.joern.io/> | Layered schema for AST, call graph, CFG, PDG, findings, binding, tags, and configuration. |

## Research Takeaway

The strongest production systems expose one of two surfaces:

1. **Rule surface:** ergonomic callbacks over existing facts, optimized for diagnostics.
2. **Model surface:** constrained facts that improve engine precision, optimized for sources/sinks/summaries/framework semantics.

polint needs both. The user-facing rule API should stay small. The agent-facing analysis extension API can be larger because it is written by a code-capable agent, but it must remain typed, versioned, cached, and validated.
