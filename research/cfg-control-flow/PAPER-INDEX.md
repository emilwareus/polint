# Paper And Source Index

This index lists papers, official docs, and downloaded artifacts used for the CFG/control-dependence research.

## Downloaded Local Artifacts

| Artifact | Local file | Role |
|---|---|---|
| Ferrante, Ottenstein, Warren, “The Program Dependence Graph and Its Use in Optimization,” TOPLAS 1987 | `papers/ferrante-pdg-control-dependence-1987.pdf` | Classic control-dependence and PDG formulation. |
| Bilardi/Pingali APT / control-dependence work | `papers/bilardi-pingali-apt-control-dependence-1997.pdf` | Output-sensitive control-dependence query/data-structure reference. |
| Cytron et al., “Efficiently Computing Static Single Assignment Form and the Control Dependence Graph,” TOPLAS 1991 | `papers/cytron-ssa-control-dependence-1991.pdf` | Dominance frontiers, SSA construction, and CDG relationship. |
| Checker Framework Dataflow Manual | `papers/checker-framework-dataflow-manual.pdf` | Source-level Java CFG/dataflow reference. |
| TAJS paper | `papers/tajs-type-analysis-for-javascript.pdf` | JavaScript flow graph and abstract interpretation reference. |
| “QL for Source Code Analysis” | `papers/ql-for-source-code-analysis.pdf` | CodeQL/QL query architecture reference. |
| IBM research page on Java exception CFG modeling | `papers/fcfg-java-exceptions-ibm.html` | Java exceptional CFG precision reference. |
| Python `dis` docs snapshot | `papers/python-dis-bytecode.html` | CPython bytecode stability and semantics reference. |

## Foundational Papers

| Source | Why it matters |
|---|---|
| Ferrante, Ottenstein, Warren, “The Program Dependence Graph and Its Use in Optimization,” 1987. DOI: <https://doi.org/10.1145/24039.24041> | Defines the classic PDG and control dependence via postdominance. |
| Cytron, Ferrante, Rosen, Wegman, Zadeck, “Efficiently Computing Static Single Assignment Form and the Control Dependence Graph,” 1991. DOI: <https://doi.org/10.1145/115372.115320> | Establishes dominance-frontier-based SSA construction and CDG computation. |
| Bilardi and Pingali control-dependence/APT work. Cornell handle: <https://hdl.handle.net/1813/7252> | Shows that materializing all control-dependence edges can be avoided with output-sensitive queries. |
| Reps, Horwitz, Sagiv, “Precise Interprocedural Dataflow Analysis via Graph Reachability,” 1995 | Future IFDS/IDE solver depends on stable CFG/call graph substrate. |
| Bourdoncle, “Efficient chaotic iteration strategies with widenings,” 1993 | Pyre references weak topological ordering for fixpoint iteration. Relevant after CFG for data-flow and abstract interpretation. |

## Official Documentation And Primary Sources

### Go

- `golang.org/x/tools/go/cfg`: <https://pkg.go.dev/golang.org/x/tools/go/cfg>
- `golang.org/x/tools/go/ssa`: <https://pkg.go.dev/golang.org/x/tools/go/ssa>
- `go/analysis/passes/ctrlflow`: <https://pkg.go.dev/golang.org/x/tools/go/analysis/passes/ctrlflow>
- `go/analysis/passes/buildssa`: <https://pkg.go.dev/golang.org/x/tools/go/analysis/passes/buildssa>
- Go compiler SSA source: <https://go.dev/src/cmd/compile/internal/ssa/>

### TypeScript / JavaScript

- TypeScript narrowing handbook: <https://www.typescriptlang.org/docs/handbook/2/narrowing.html>
- TypeScript 1.8 control-flow analysis release note: <https://www.typescriptlang.org/docs/handbook/release-notes/typescript-1-8.html>
- Oxc repository: <https://github.com/oxc-project/oxc>
- Oxc CFG crate docs: <https://docs.rs/oxc/latest/oxc/cfg/index.html>
- ESLint code path analysis docs: <https://eslint.org/docs/latest/extend/code-path-analysis>
- CodeQL JS library guide: <https://codeql.github.com/docs/codeql-language-guides/codeql-library-for-javascript/>
- CodeQL JS CFG library: <https://codeql.github.com/codeql-standard-libraries/javascript/semmle/javascript/CFG.qll/module.CFG.html>

### Python

- CodeQL Python control-flow guide: <https://codeql.github.com/docs/codeql-language-guides/analyzing-control-flow-in-python/>
- CodeQL Python library guide: <https://codeql.github.com/docs/codeql-language-guides/codeql-library-for-python/>
- Pyright repository: <https://github.com/microsoft/pyright>
- Python type narrowing guide: <https://typing.python.org/en/latest/guides/type_narrowing.html>
- Pyre/Pysa docs: <https://pyre-check.org/docs/pysa-basics/> and <https://pyre-check.org/docs/pysa-implementation-details/>
- Python `dis` docs: <https://docs.python.org/3/library/dis.html>
- PEP 339: <https://peps.python.org/pep-0339/>

### Java / JVM

- JLS 14 statements and abrupt completion: <https://docs.oracle.com/javase/specs/jls/se21/html/jls-14.html>
- JLS 14.20.2 `finally`: <https://docs.oracle.com/javase/specs/jls/se21/html/jls-14.html#jls-14.20.2>
- JLS 14.20.3 try-with-resources: <https://docs.oracle.com/javase/specs/jls/se21/html/jls-14.html#jls-14.20.3>
- JLS 14.19 synchronized: <https://docs.oracle.com/javase/specs/jls/se21/html/jls-14.html#jls-14.19>
- JVMS `monitorenter`: <https://docs.oracle.com/javase/specs/jvms/se21/html/jvms-6.html#jvms-6.5.monitorenter>
- JVMS `invokedynamic`: <https://docs.oracle.com/javase/specs/jvms/se21/html/jvms-6.html#jvms-6.5.invokedynamic>
- Soot exceptional graph docs: <https://www.sable.mcgill.ca/soot/doc/soot/toolkits/graph/ExceptionalUnitGraph.html>
- SootUp docs/repository: <https://github.com/soot-oss/SootUp>
- WALA documentation: <https://github-wiki-see.page/m/wala/WALA/wiki/Intermediate-Representation-%28IR%29>
- Checker Framework dataflow API/docs: <https://checkerframework.org/releases/3.49.4/api/org/checkerframework/dataflow/cfg/package-summary.html>

### Language-Neutral IR And Query Systems

- LLVM LangRef basic blocks: <https://llvm.org/docs/LangRef.html>
- LLVM exception handling: <https://llvm.org/docs/ExceptionHandling.html>
- LLVM testing guide: <https://llvm.org/docs/TestingGuide.html>
- LLVM FileCheck: <https://llvm.org/docs/CommandGuide/FileCheck.html>
- MLIR LangRef: <https://mlir.llvm.org/docs/LangRef/>
- MLIR Control Flow dialect: <https://mlir.llvm.org/docs/Dialects/ControlFlowDialect/>
- MLIR SCF dialect: <https://mlir.llvm.org/docs/Dialects/SCFDialect/>
- CodeQL path query docs: <https://codeql.github.com/docs/writing-codeql-queries/creating-path-queries/>
- CodeQL query testing docs: <https://codeql.github.com/docs/writing-codeql-queries/testing-custom-queries/>
- Code Property Graph spec: <https://cpg.joern.io/>
- Joern docs: <https://docs.joern.io/code-property-graph/>
- Semgrep data-flow overview: <https://semgrep.dev/docs/writing-rules/data-flow/data-flow-overview>

## Benchmark And Validation Sources

- Test262: <https://github.com/tc39/test262>
- ESLint code-path tests: <https://github.com/eslint/eslint/tree/main/tests/lib/linter/code-path-analysis>
- TypeScript compiler tests: <https://github.com/microsoft/TypeScript/tree/main/tests/cases/compiler>
- CodeQL JS tests: <https://github.com/github/codeql/tree/main/javascript/ql/test/library-tests>
- CodeQL Go control-flow tests: <https://github.com/github/codeql/tree/main/go/ql/test/library-tests/semmle/go/controlflow/ControlFlowGraph>
- CodeQL Java control-flow tests: <https://github.com/github/codeql/tree/main/java/ql/test/library-tests/controlflow>
- CodeQL Python control-flow tests: <https://github.com/github/codeql/tree/main/python/ql/test/library-tests/ControlFlow>
- SecBench.js: <https://conf.researchr.org/details/icse-2023/icse-2023-artifact-evaluation/51/SecBench-js-An-Executable-Security-Benchmark-Suite-for-Server-Side-JavaScript>
- OWASP Benchmark Java: <https://owasp.org/www-project-benchmark/>
- Juliet Java test suite: <https://www.nist.gov/publications/juliet-11-cc-and-java-test-suite>
- DroidBench: <https://github.com/secure-software-engineering/DroidBench>

## Citation Caveats

- Some downloaded PDFs are public mirrors. Prefer DOI/official pages in prose where available.
- CodeQL, Oxc, Pyright, TypeScript, Go, Soot, WALA, Checker Framework, CPython, LLVM, and MLIR findings were validated against cloned source snapshots as well as public docs.
- Java bytecode/source CFG findings intentionally distinguish source-level and bytecode-level semantics.
