# CFG And Control Dependence Research

Date: 2026-05-15

This folder researches how polint should build native control-flow graph facts, dominance/postdominance facts, control-dependence facts, and path evidence across Go, TypeScript/JavaScript, Python, Java/JVM, and future languages.

The core conclusion is:

```text
Do not build one universal AST-walk CFG.
Build a native layered CFG substrate:
  operation nodes
  basic blocks
  typed edges
  exceptional/abrupt-flow edges
  virtual entry/exit nodes
  provenance and precision labels
  derived dominance/postdominance/control-dependence
  extension overlays that can add facts but must validate
```

CFG is the missing structural layer between the semantic index/module graph and serious data-flow/call-graph precision. It should be a first-class fact family, not an implementation detail hidden inside each rule.

## Deliverables

| File | Purpose |
|---|---|
| `FINAL-REPORT.md` | Main synthesis, product recommendation, and state-of-the-art analysis. |
| `RECOMMENDED_IMPLEMENTATION.md` | Concrete native Rust implementation path for polint. |
| `STANDARD.md` | Normalized vocabulary, fact model, edge kinds, precision labels, and invariants. |
| `REPO-INDEX.md` | OSS repositories cloned and implementation files inspected. |
| `PAPER-INDEX.md` | Papers, official docs, and downloaded/local artifacts. |
| `RESEARCH-ANALYSIS.md` | Accuracy, complexity, algorithm, and product-fit analysis. |
| `VALIDATION.md` | Validation pass over sources, claims, and recommended test strategy. |
| `SUBAGENT-FINDINGS.md` | Consolidated findings from parallel research agents. |
| `languages/*.md` | Go, TS/JS, Python, and Java/JVM reports. |
| `tools/language-neutral-ir.md` | LLVM, MLIR, CodeQL, Joern, Semgrep, and CPG lessons. |
| `algorithms/*.md` | Python-ish pseudo-code for CFG construction, control dependence, and exceptional/async flow. |
| `implementation/native-rust-path.md` | Internal module layout, staged implementation, and SDK path. |
| `oss/implementation-comparison.md` | Comparative table across inspected systems. |
| `benchmarks/evaluation-plan.md` | Fixture, benchmark, and differential validation strategy. |
| `decisions/DECISIONS.md` | Decision log and rejected alternatives. |

Third-party repositories are cloned in `research/cfg-control-flow/repos/`, which is gitignored. Research papers and official docs snapshots are downloaded in `research/cfg-control-flow/papers/`.

## State Of The Art Today

The mature systems do not converge on a single representation. They converge on a principle: control flow must be explicit enough for the next analysis layer, and every loss of language semantics must be declared.

- **Go:** `golang.org/x/tools/go/ssa` is the best public Go CFG substrate. `go/cfg` is useful as a syntactic reference, but it omits important expression and abnormal flow details.
- **TS/JS:** Oxc has the best Rust-native CFG shape. TypeScript and Pyright-style flow nodes are excellent models for type narrowing but not general CFG APIs. ESLint code paths are a useful rule-author ergonomics reference. CodeQL JS is the best semantic coverage reference.
- **Python:** CodeQL-style source CFG plus Pyright/Pyre-style flow/narrowing layers is the best target. CPython bytecode is a semantic reference, not a source-level rule API.
- **Java/JVM:** Java needs source-level and bytecode-level precision tiers. Checker Framework is the best source CFG reference; Soot, SootUp, WALA, and OPAL are the best bytecode/JVM CFG references.
- **Language-neutral IR:** LLVM/MLIR validate the block/terminator/dominator shape. CodeQL validates typed query views. Joern/CPG validates multi-graph layering, but polint should not expose a public graph database.

## Recommended Shape

```text
selected source set and lifecycle inputs
  -> parse and semantic-index facts
  -> function/body discovery
  -> language-owned operation CFG
  -> block CFG
  -> normal + abrupt + exceptional edge facts
  -> reachability and graph invariants
  -> dominators and postdominators
  -> control-dependence facts
  -> path evidence and diagnostics
  -> extension overlay validation and merge
  -> typed SDK views
```

The first public SDK view should be conservative:

- `Cfg<'_>` for functions/bodies, blocks, nodes, edges, entry/exit, reachability, and precision filters.
- `ControlDependence<'_>` after postdominance and validation are stable.
- No public raw parser ASTs, no public `petgraph`, no public Oxc/SSA/Soot object leakage.

## Fit With Existing Research

This track consumes:

- `research/semantic-index/`: stable function/body IDs, source spans, scopes, imports, references, and unresolved semantic facts.
- `research/module-graph/`: source-set/build-target/lifecycle selection and package/module identity.
- `research/analysis-kernel/`: provider DAG, fact layers, provenance, validation, extension merges, cache keys.
- `research/evaluation-harness/`: structural snapshots, differential checks, default-vs-extension metrics.
- `research/framework-entrypoints/`: synthetic dispatch and lifecycle overlays must not be mixed into local CFG without labels.
- `research/call-graphs/`: callsites depend on CFG placement; call edges are not CFG edges.
- `research/data-flow/`: local flow and taint paths require stable CFG, exceptional edges, and evidence paths.

It feeds:

- Type/value/points-to/alias analysis.
- Function effects and summaries.
- Sparse value-flow and IFDS/IDE solvers.
- Program slicing and path explanation.
- Rule SDK ergonomics for reachability, dominance, and guarded code.
