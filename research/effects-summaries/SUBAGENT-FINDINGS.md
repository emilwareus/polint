# Subagent Findings

Six parallel research agents investigated the problem from different angles.

## Academic Algorithms

Main points:

- Summaries replace repeated callee traversal with reusable transformers.
- Sharir-Pnueli, IFDS, IDE, WPDS, modular abstract interpretation, and
  demand-driven analyses are complementary, not one linear progression.
- IFDS is appropriate only for finite distributive subset domains.
- Summary cache keys must include function identity, body hash, language/setup
  config, domain version, context abstraction, dependency summary digests, and
  extension/model digests.
- Recursion requires SCC fixed points and widening.
- Unknown must be represented explicitly.

## CodeQL, Semgrep, Joern, Pysa

Main points:

- CodeQL models-as-data is the richest summary/model reference, especially
  `summaryModel`, source/sink/barrier models, access paths, provenance, and
  exactness.
- Semgrep is the best usability reference for taint sources/sinks/sanitizers,
  propagators, `exact`, `by-side-effect`, labels, and testing.
- Joern's CPG semantics show compact method flow pairs, useful as shorthand but
  too positional for polint's internal representation.
- Pysa is the closest summary-first comparator: sources, sinks, TITO,
  sanitizers, model queries, generated models, features, and emitted debug
  traces.

## Pysa, Infer, Pulse, RacerD

Main points:

- Pysa computes a global fixed point over callable models and uses access-path
  trees with broadening.
- Infer/Pulse stores pre/post disjunctive execution states and non-disjunctive
  summary data.
- Pulse address attributes give polint a strong vocabulary for invalidation,
  allocation, resource obligations, awaited/unawaited async objects, taint, and
  initialization.
- RacerD is a high-signal example of intentionally incomplete summaries: it
  gives up alias completeness to keep output actionable.

## Go, TS/JS, Python

Main points:

- Go should use `go/packages`, `go/analysis`, and `buildssa` as official provider
  inputs where semantic precision matters.
- TS/JS should use Oxc for fast native syntax/scope/CFG and the TypeScript
  compiler as an optional official checker provider for type predicates,
  assertions, `never`, and declaration summaries.
- Python should use CPython-compatible AST/typing semantics, with `TypeGuard`,
  `TypeIs`, `NoReturn`/`Never`, decorators, callable signatures, and async/generator
  behavior modeled explicitly.
- Framework overlays should be first-class summaries, not hidden heuristics.

## JVM, LLVM, MLIR, OPAL

Main points:

- WALA synthetic summaries and bypass methods are the best JVM reference for
  modeled library bodies.
- Soot read/write sets show how memory effects depend on call graph and
  points-to.
- Doop shows model packs for reflection, native methods, open programs, and
  framework behavior.
- OPAL shows effect properties should be separate fixed-point facts.
- LLVM/MLIR provide the best compact effect-lattice design: access kind crossed
  with resource/location kind.

## Critical Review

Main points:

- A generic summary bag is a dead end.
- A giant effect enum is also a dead end.
- Every summary must carry status, precision, provenance, trust, validation, and
  cache-key inputs.
- AI-authored summaries should be treated as untrusted candidates until
  validated.
- The public SDK should expose typed views, not raw summary internals.
- Benchmarks must cover recursion, dynamic dispatch, async fanout, large repos,
  warm/cold cache, and extension invalidation.
