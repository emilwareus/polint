# Paper And Documentation Index

Downloaded artifacts live under `research/abstract-interpretation/papers/`.

## Downloaded Artifacts

| File | Source | Use |
|---|---|---|
| `cousot-cousot-popl77.html` | <https://cs.nyu.edu/~pcousot/COUSOTpapers/POPL77.shtml> | Foundational lattice model of abstract interpretation. |
| `cousot-halbwachs-popl78.html` | <https://www.di.ens.fr/~cousot/COUSOTpapers/POPL78.shtml> | Linear constraints and numeric domains. |
| `trace-partitioning-esop05.pdf` | <https://www.di.ens.fr/~rival/papers/esop05-partitioning.pdf> | Trace partitioning and path precision. |
| `apron-cav09.pdf` | <https://antoinemine.github.io/Apron/doc/papers/article-mine-jeannet-cav09.pdf> | Numerical abstract domain library API. |
| `apron-doc.html` | <https://antoinemine.github.io/Apron/doc/> | Apron domain list and common interface docs. |
| `octagon-domain-arxiv.pdf` | <https://arxiv.org/pdf/cs/0703084> | Octagon domain complexity and algorithms. |
| `elina-home.html` | <https://elina.ethz.ch/> | Optimized numeric domain implementation notes. |
| `frama-c-eva-manual.pdf` | <https://frama-c.com/download/frama-c-eva-manual.pdf> | Eva abstract interpretation user/manual reference. |
| `checker-framework-dataflow-manual.pdf` | <https://checkerframework.org/manual/checker-framework-dataflow-manual.pdf> | Java dataflow framework and stores. |
| `rustc-mir-dataflow.html` | <https://rustc-dev-guide.rust-lang.org/mir/dataflow.html> | rustc dataflow framework concepts. |
| `typescript-narrowing.html` | <https://www.typescriptlang.org/docs/handbook/2/narrowing.html> | TypeScript official narrowing semantics. |
| `pyright-type-concepts.html` | <https://microsoft.github.io/pyright/#/type-concepts-advanced> | Pyright advanced type/narrowing docs entry. |
| `clang-static-analyzer.html` | <https://clang.llvm.org/docs/ClangStaticAnalyzer.html> | Clang Static Analyzer overview. |
| `clang-analyzer-debug-checks.html` | <https://clang.llvm.org/docs/analyzer/developer-docs/DebugChecks.html> | Analyzer state/eval debug checks. |
| `abstract-interpretation-lecture-notes.pdf` | <https://cs.au.dk/~amoeller/spa/spa.pdf> | Static program analysis reference notes. |
| `tarski-fixpoint-1955.pdf` | <https://msp.org/pjm/1955/5-2/pjm-v5-n2-p11-p.pdf> | Fixed point theorem reference. |
| `reps-horwitz-sagiv-popl95-ifds.pdf` | <https://research.cs.wisc.edu/wpis/papers/popl95.pdf> | IFDS interprocedural dataflow via graph reachability. |
| `might-shivers-gcfa-2006.pdf` | <https://matt.might.net/papers/might2006gcfa.pdf> | Abstract garbage collection for flow analysis. |
| `vanhorn-might-aam-2010.pdf` | <https://matt.might.net/papers/vanhorn2010abstract.pdf> | Abstracting abstract machines. |
| `sharir-pnueli-cern-record.html` | <https://cds.cern.ch/record/120118> | Bibliographic record for classic interprocedural dataflow chapter. |

## Primary Research References

| Reference | Topic | Design Impact |
|---|---|---|
| Tarski, "A Lattice-Theoretical Fixpoint Theorem" | Complete lattices and fixpoints | Theoretical basis for monotone analysis. Downloaded. |
| Cousot & Cousot, POPL 1977 | Abstract interpretation | Sound approximation and abstract fixpoints. Downloaded. |
| Cousot & Halbwachs, POPL 1978 | Linear restraints | Numerical domain lineage. Downloaded. |
| Miné, Octagon Abstract Domain | Octagon constraints | Relational numeric domain candidate with known cost/precision tradeoffs. Downloaded. |
| Jeannet & Miné, Apron | Numerical domain API | Common manager interface, domain swapping. Downloaded. |
| Mauborgne & Rival, Trace Partitioning | Path sensitivity | Budgeted trace partitions for selected guards. Downloaded. |
| Sharir & Pnueli | Interprocedural dataflow | Summary/context sensitivity foundations. Bibliographic record downloaded. |
| Reps/Horwitz/Sagiv IFDS | Graph-reachability dataflow | Useful for taint/dataflow, not all domains. Downloaded. |
| Might & Shivers abstract GC | Heap precision | Scoped forgetting/abstract GC before joins. Downloaded. |
| Van Horn & Might AAM | Abstract machines | Useful theory for future evaluator/domain unification. Downloaded. |

## Tool Documentation References

| Tool | Docs |
|---|---|
| Infer / Pulse | <https://fbinfer.com/docs/checker-pulse/>, <https://fbinfer.com/docs/absint-framework/> |
| Clang Static Analyzer | <https://clang.llvm.org/docs/ClangStaticAnalyzer.html> |
| rustc MIR dataflow | <https://rustc-dev-guide.rust-lang.org/mir/dataflow.html> |
| Checker Framework | <https://checkerframework.org/manual/> |
| TypeScript narrowing | <https://www.typescriptlang.org/docs/handbook/2/narrowing.html> |
| Pyright advanced concepts | <https://microsoft.github.io/pyright/#/type-concepts-advanced> |
| Python typing narrowing spec | <https://typing.python.org/en/latest/spec/narrowing.html> |
| CodeQL dataflow/model packs | <https://codeql.github.com/docs/codeql-language-guides/analyzing-data-flow-in-javascript-and-typescript/> |
| Semgrep taint | <https://semgrep.dev/docs/writing-rules/data-flow/taint-mode/overview> |
| Goblint docs | <https://goblint.readthedocs.io/> |
| Frama-C Eva | <https://www.frama-c.com/fc-plugins/eva.html> |
| Apron | <https://antoinemine.github.io/Apron/doc/> |
| ELINA | <https://elina.ethz.ch/> |

## Validation Notes

- The original attempted Clang developer manual URL returned 404 and was
  removed. It was replaced with the official Static Analyzer overview and
  DebugChecks docs.
- Pyright's web app entry was downloaded as a short HTML shell. Claims about
  Pyright implementation details should rely on the cloned source tree and tests,
  not that shell alone.
- Flow, NullAway, Error Prone, WALA, Soot, OPAL, SpotBugs, pytype, and Astrée
  are treated as secondary/contextual references unless a specific local clone
  or downloaded primary document is listed above.
