# Paper And Source Index

Downloaded artifacts live in `research/effects-summaries/papers/`.

## Core Algorithms

| Source | Local artifact | Why it matters |
|---|---|---|
| Sharir and Pnueli, functional approach to interprocedural analysis | See notes in report; background source: <https://pages.cs.wisc.edu/~fischer/cs701.f14/INTERPROCEDURAL-ANALYSIS-AUX/PhiFns.html> | The oldest clear formulation of summary functions as reusable interprocedural transformers. |
| Reps, Horwitz, Sagiv, IFDS POPL 1995 | `papers/ifds-popl95.pdf`, <https://research.cs.wisc.edu/wpis/papers/popl95.pdf> | Explains distributive finite subset problems, valid paths, and summary-based tabulation. |
| Sagiv, Reps, Horwitz, IDE | <https://cris.tau.ac.il/en/publications/precise-interprocedural-dataflow-analysis-with-applications-to-co-2/> | Generalizes IFDS to environment-transformer edge functions. |
| Reps, Schwoon, Jha, Melski, WPDS SAS 2003 | `papers/wpds-sas03.pdf`, <https://research.cs.wisc.edu/wpis/papers/sas03.pdf> | Weighted pushdown systems for interprocedural dataflow with matched calls/returns. |
| Cousot and Cousot, modular abstract interpretation | `papers/modular-abstract-interpretation-cc02.pdf`, <https://pcousot.github.io/publications/CousotCousot-CC02.pdf> | Abstract-interpretation view of modular procedure summaries and fixpoint semantics. |
| SWIFT, demand-driven compositional taint analysis | `papers/swift-pldi14.pdf` | Shows demand/compositional tradeoffs for scalable taint-style analysis. |
| Boomerang, demand-driven flow-sensitive alias analysis | `papers/boomerang-ecoop16.pdf`, <https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ECOOP.2016.22> | Shows why aliases often need demand summaries instead of whole-program precision everywhere. |
| BigDataflow | `papers/bigdataflow-2024.pdf`, <https://arxiv.org/abs/2412.12579> | Recent scaling reference for data-flow workloads and summary pressure. |

## Production Systems And Official Docs

| Source | Local artifact | Why it matters |
|---|---|---|
| CodeQL JavaScript library models | `papers/codeql-js-library-models.html`, <https://codeql.github.com/docs/codeql-language-guides/customizing-library-models-for-javascript/> | Best model for typed, declarative library summaries with access paths. |
| CodeQL sanitizers and validators in models-as-data | `papers/codeql-sanitizers-models-as-data-2026.html`, <https://github.blog/changelog/2026-04-21-codeql-now-supports-sanitizers-and-validators-in-models-as-data/> | Shows model data is expanding beyond sources/sinks/summaries into barriers and guards. |
| Pysa implementation details | `papers/pysa-implementation-details.html`, <https://pyre-check.org/docs/pysa-implementation-details/> | Best summary-first production taint design for Python. |
| Semgrep taint mode | `papers/semgrep-taint-overview.html`, <https://semgrep.dev/docs/writing-rules/data-flow/taint-mode/overview> | Best usability reference for source/sink/sanitizer/propagator ergonomics. |
| Infer Pulse docs | `papers/infer-pulse.html`, <https://fbinfer.com/docs/checker-pulse/> | Production reference for compositional heap/error summaries. |
| Infer separation logic and bi-abduction docs | `papers/infer-biabduction.html`, <https://fbinfer.com/docs/separation-logic-and-bi-abduction/> | Conceptual basis for pre/post heap summaries. |
| RacerD OOPSLA preprint | `papers/racerd-oopsla18-preprint.pdf`, <https://ilyasergey.net/papers/racerd-oopsla18-preprint.pdf> | High-signal race-detection summaries with known incompleteness. |
| Go `analysis` package | <https://pkg.go.dev/golang.org/x/tools/go/analysis> | Official Go modular facts and analyzer dependencies. |
| Go `buildssa` analyzer | <https://pkg.go.dev/golang.org/x/tools/go/analysis/passes/buildssa> | Official SSA-backed analysis pass. |
| TypeScript narrowing handbook | <https://www.typescriptlang.org/docs/handbook/2/narrowing.html> | Official control-flow and type-narrowing behavior. |
| TypeScript 3.7 release notes | <https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-7.html> | Assertion functions and `never` control-flow effects. |
| TypeScript 5.5 release notes | <https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-5.html> | Inferred type predicate summaries. |
| Python typing narrowing spec | <https://typing.python.org/en/latest/spec/narrowing.html> | Official `TypeGuard` and `TypeIs` semantics. |
| LLVM LangRef | `papers/llvm-langref.html`, <https://llvm.org/docs/LangRef.html> | Function attributes and memory effects. |
| LLVM ModRef source | `papers/llvm-modref-source.html`, <https://llvm.org/doxygen/ModRef_8h_source.html> | Compact memory-effect lattice source. |
| MLIR side effects | `papers/mlir-sideeffects.html`, <https://mlir.llvm.org/docs/Rationale/SideEffectsAndSpeculation/> | Resource-scoped effects and speculation. |
| OPAL PropertyStore API | `papers/opal-property-store.html`, <https://www.opal-project.de/library/api/SNAPSHOT/org/opalj/fpcf/PropertyStore.html> | Fixed-point property store model. |
| Soot SideEffectAnalysis docs | `papers/soot-sideeffect.html`, <https://www.sable.mcgill.ca/soot/doc/soot/jimple/toolkits/pointer/SideEffectAnalysis.html> | Read/write set effect summaries over points-to/call graph. |

## Important Accuracy Notes

- IFDS/IDE complexity claims only apply to domains that fit their distributive/finite assumptions. Many effect domains do not.
- Source/sink/sanitizer model packs are only as sound as call resolution, access-path identity, and framework lifecycle modeling.
- "Pure", "side-effect-free", and "deterministic" annotations are claims, not proof, unless validated by a trusted analyzer.
- Official language tools are acceptable provider inputs when they are the compatibility authority. Random OSS libraries should remain references/oracles, not core runtime dependencies.
