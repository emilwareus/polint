# Paper And Source Index

This index lists papers, official docs, and downloaded artifacts used for this research track.

## Downloaded Local Artifacts

| Artifact | Local file | Role |
|---|---|---|
| Steensgaard, "Points-to Analysis in Almost Linear Time", POPL 1996 | `papers/steensgaard-popl96.pdf` | Near-linear unification-based pointer analysis baseline. |
| Shapiro and Horwitz, "Fast and Accurate Flow-Insensitive Points-To Analysis", POPL 1997 | `papers/shapiro-horwitz-popl97.pdf` | Tunable points-to precision/cost reference. |
| Sui and Xue, SVF paper, CC 2016 | `papers/svf-cc16.pdf` | Sparse value-flow graph and memory SSA reference. |
| Bravenboer and Smaragdakis, Doop, OOPSLA 2009 | `papers/doop-oopsla09.pdf` | Declarative Java points-to/call graph analysis. |
| Flow, "Fast and Precise Type Checking for JavaScript", OOPSLA 2017 | `papers/flow-oopsla17.pdf` | JS type checking/refinement reference. |
| TAJS paper, ECOOP 2009 | `papers/tajs-ecoop09.pdf` | JavaScript abstract interpretation reference. |
| PoTo Python points-to paper | `papers/poto-python-points-to-ecoop25.pdf` | Recent Python points-to research reference. |
| Andersen fine-grained complexity paper | `papers/andersen-fine-grained-complexity-2020.pdf` | Complexity boundaries for Andersen-style pointer analysis. |
| Phoenix pointer analysis 2026 preprint | `papers/phoenix-pointer-analysis-2026.pdf` | Recent pointer-analysis research scanned for trends. |
| PIP incomplete C pointer analysis 2026 preprint | `papers/pip-incomplete-c-2026.pdf` | Incomplete-program pointer-analysis trend reference. |
| CG-FSPTA 2025 preprint | `papers/cg-fspta-2025.pdf` | Call-graph-guided flow-sensitive pointer-analysis trend reference. |
| IFDS paper | `papers/ifds-popl95.pdf` | Finite distributive data-flow framework reference. |
| Cousot abstract interpretation page | `papers/cousot-abstract-interpretation-1977.html` | Foundational abstract interpretation reference. |
| Ty type-system docs | `papers/ty-type-system.html` | Official Ty type-system documentation snapshot. |
| Pyrefly v1 blog | `papers/pyrefly-v1.html` | Official Pyrefly release/design context. |
| Pysa implementation details | `papers/pysa-implementation-details.html` | Official Pysa interprocedural/modeling docs. |
| CodeQL Python dataflow docs | `papers/codeql-python-dataflow.html` | Official Python data-flow model docs. |
| CodeQL JS type tracking docs | `papers/codeql-js-type-tracking.html` | Official JS API/type-tracking docs. |
| LLVM AliasAnalysis docs | `papers/llvm-alias-analysis.html` | Official LLVM alias provider interface docs. |
| LLVM MemorySSA docs | `papers/llvm-memoryssa.html` | Official LLVM sparse memory SSA docs. |
| Oxc semantic docs | `papers/oxc-semantic-docs.html` | Official Oxc semantic API docs. |
| Oxc scoping docs | `papers/oxc-scoping-docs.html` | Official Oxc scoping API docs. |
| WALA pointer-analysis wiki mirror | `papers/wala-pointer-analysis.html` | WALA pointer-analysis overview. |

## Canonical Links

### Python

- Ty type system docs: <https://docs.astral.sh/ty/features/type-system/>
- Ty repository: <https://github.com/astral-sh/ty>
- Ruff repository with Ty source: <https://github.com/astral-sh/ruff>
- Pyrefly blog v1: <https://pyrefly.org/blog/v1.0/>
- Pyrefly repository: <https://github.com/facebook/pyrefly>
- Pyright repository: <https://github.com/microsoft/pyright>
- Pyre/Pysa implementation details: <https://pyre-check.org/docs/pysa-implementation-details/>
- mypy docs: <https://mypy.readthedocs.io/>
- pytype repository/docs: <https://github.com/google/pytype>
- Python typing narrowing guide: <https://typing.python.org/en/latest/guides/type_narrowing.html>
- PoTo Python points-to paper: <https://arxiv.org/abs/2409.03918>

### TypeScript / JavaScript

- TypeScript compiler repository: <https://github.com/microsoft/TypeScript>
- TypeScript narrowing handbook: <https://www.typescriptlang.org/docs/handbook/2/narrowing.html>
- Oxc repository: <https://github.com/oxc-project/oxc>
- Oxc semantic docs: <https://docs.rs/oxc/latest/oxc/semantic/struct.Semantic.html>
- Oxc scoping docs: <https://docs.rs/oxc_semantic/latest/oxc_semantic/struct.Scoping.html>
- Flow paper: <https://arxiv.org/abs/1708.08021>
- TAJS paper page: <https://cs.au.dk/~amoeller/papers/tajs/>
- Jelly repository: <https://github.com/cs-au-dk/jelly>
- CodeQL JavaScript type tracking: <https://codeql.github.com/docs/codeql-language-guides/using-type-tracking-for-api-modeling/>

### Go

- Go `go/types` package docs: <https://pkg.go.dev/go/types>
- Go `x/tools/go/ssa`: <https://pkg.go.dev/golang.org/x/tools/go/ssa>
- Go call graph packages: <https://pkg.go.dev/golang.org/x/tools/go/callgraph>
- Go VTA package: <https://pkg.go.dev/golang.org/x/tools/go/callgraph/vta>
- Go RTA package: <https://pkg.go.dev/golang.org/x/tools/go/callgraph/rta>

### Java / JVM

- Doop paper: <https://people.cs.umass.edu/~yannis/doop-oopsla09prelim.pdf>
- Doop repository: <https://github.com/plast-lab/doop>
- WALA repository: <https://github.com/wala/WALA>
- WALA pointer-analysis wiki mirror: <https://github-wiki-see.page/m/wala/WALA/wiki/Pointer-Analysis>
- Soot repository: <https://github.com/soot-oss/soot>
- SootUp repository: <https://github.com/soot-oss/SootUp>
- OPAL repository: <https://github.com/opalj/opal>
- Checker Framework dataflow docs: <https://checkerframework.org/releases/latest/api/org/checkerframework/dataflow/cfg/package-summary.html>

### Neutral Algorithms

- Steensgaard POPL 1996 DOI: <https://doi.org/10.1145/237721.237727>
- Shapiro/Horwitz POPL 1997 DOI: <https://doi.org/10.1145/263699.263703>
- LLVM AliasAnalysis docs: <https://llvm.org/docs/AliasAnalysis.html>
- LLVM MemorySSA docs: <https://www.llvm.org/docs/MemorySSA.html>
- SVF CC 2016 paper: <https://yuleisui.github.io/publications/cc16.pdf>
- IFDS paper: <https://research.cs.wisc.edu/wpis/papers/popl95.pdf>
- Cousot abstract interpretation paper page: <https://www.di.ens.fr/~cousot/COUSOTpapers/POPL77.shtml>
- Andersen fine-grained complexity: <https://arxiv.org/abs/2006.01491>

## Citation Caveats

- The nowpublishers survey by Smaragdakis and Balatsouras was identified as relevant but was not mirrored locally because the PDF request returned 403 during this run. Use its DOI/publisher page in future citation work.
- Recent 2025/2026 arXiv preprints were scanned for trends, not treated as mature implementation guidance.
- Official implementation docs and local source snapshots are weighted more heavily than secondary blog posts.
