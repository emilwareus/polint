# Semantic Index Research

Date: 2026-05-15

This folder researches how state-of-the-art tools build semantic indexes: symbols, scopes, declarations, references, imports, exports, aliases, generated symbols, type-aware resolution, cross-reference indexes, and export formats.

The important conclusion is:

```text
Do not build one generic semantic index.
Build layered, language-owned semantic facts behind one typed fact contract.
Use explicit resolution status, provenance, precision, and extension merges.
Export with SCIP/Kythe-like identity when needed.
```

## Deliverables

| File | Purpose |
|---|---|
| `FINAL-REPORT.md` | Main synthesis and executive recommendation. |
| `RECOMMENDED_IMPLEMENTATION.md` | Concrete native Rust path for polint. |
| `STANDARD.md` | Normalized vocabulary and fact model used across reports. |
| `REPO-INDEX.md` | OSS repositories cloned and implementation files inspected. |
| `PAPER-INDEX.md` | Research papers and official technical sources used. |
| `RESEARCH-ANALYSIS.md` | Cross-tool algorithm analysis, accuracy, complexity, and tradeoffs. |
| `VALIDATION.md` | Benchmark, fixture, precision, and cache validation plan. |
| `tools/*.md` | Detailed per-tool reports. |
| `algorithms/semantic-index-pipeline.md` | Language-neutral pseudo-code for semantic indexing. |
| `implementation/native-rust-path.md` | Proposed internal module structure and staged implementation. |
| `oss/implementation-comparison.md` | Compact comparative table. |
| `decisions/DECISIONS.md` | Decision log and rejected alternatives. |

Third-party source checkouts live in `research/semantic-index/repos/`, which is gitignored. Papers live in `research/semantic-index/papers/`.

## State Of The Art Today

The strongest tools do not converge on a single architecture. They converge on a few principles:

- **Compiler-owned semantics win for accuracy.** TypeScript, gopls, Pyright, Ty/Pyrefly, and JDT all build indexes from their own language semantic model rather than from a generic AST.
- **Incrementality is a semantic architecture decision.** rust-analyzer, Ty, TypeScript builder state, gopls caches, and Pyright `Program` state all separate stable inputs from derived facts.
- **Relational/fixpoint layers are best for derived cross-language facts.** CodeQL is the reference for recursive derived facts over extracted relations, but its database/query model is not the best low-latency internal representation for polint.
- **Export formats are not internal engines.** SCIP, LSIF, and Kythe are excellent references for symbols, occurrences, stable identities, and cross-reference storage. They should inform export and identity design, not become the in-memory rule SDK.
- **A universal generic scope resolver is a low ceiling.** Semgrep's generic naming layer is useful for rule ergonomics but is not enough for high-accuracy semantic indexes.
- **Agent-extensible analysis changes the target.** polint should expose explicit unknowns and precision labels so agents can add repo-local Rust providers for generated symbols, framework references, aliases, and resolution hints.

## Recommended Shape

```text
Source files
  -> syntax trees
  -> declarations and lexical scopes
  -> imports, exports, packages, modules
  -> symbols and stable symbol keys
  -> local references and unresolved occurrences
  -> alias/reexport/generated-symbol fixpoint
  -> type-assisted resolution where available
  -> extension fact merge and validation
  -> searchable cross-reference indexes
  -> typed SDK views and optional SCIP/Kythe export
```

The first public SDK additions should be conservative:

- `Scopes<'_>`
- `Imports<'_>`
- `Symbols<'_>` deepened with stable identity and declaration/definition roles
- `References<'_>` deepened with resolution status and confidence
- internal `ResolutionFacts` before any public exactness claim

## Fit With Existing Research

This track consumes:

- `research/analysis-kernel/`: provider DAG, fact layers, provenance, cache keys, extension merge gates.
- `research/evaluation-harness/`: default-vs-extension metrics and benchmark schema.
- `research/agent-extension-surface/`: Rust-code model providers and validation lifecycle.
- `research/framework-entrypoints/`: generated dispatch and framework references.
- `research/call-graphs/`: call-site and target resolution requirements.
- `research/data-flow/`: place/use-def facts and source/sink model requirements.

It should feed the next research track:

- `research/module-graph/`: imports, package roots, workspace topology, generated-code zones, and dependency direction.
