# OSS Implementation Reports

This document applies the standard structure from `STANDARD.md` to the main implementations inspected during this research pass.

## Ty / Ruff

**Language/domain:** Python type checking.

**Role:** Rust-native Python type system, semantic index, place/use-def, reachability, and narrowing reference.

**Key source paths:**

- `ruff/crates/ty_python_core/src/place.rs`
- `ruff/crates/ty_python_core/src/builder.rs`
- `ruff/crates/ty_python_core/src/predicate.rs`
- `ruff/crates/ty_python_core/src/reachability_constraints.rs`
- `ruff/crates/ty_python_semantic/src/reachability.rs`
- `ruff/crates/ty_python_semantic/src/types/infer.rs`
- `ruff/crates/ty_python_semantic/src/types/narrow.rs`
- `ruff/crates/ty_python_semantic/src/types/relation.rs`

**Algorithm shape:** semantic indexing produces places, predicates, reachability constraints, and narrowing-supporting facts. Type inference and narrowing consume those facts. Reachability constraints are normalized ternary formulas, which is a more precise representation than a flat boolean "reachable/unreachable" label.

**Fact model:** places, scoped place IDs, predicates, reachability constraints, narrowing aliases, use-def/binding facts, type relations.

**Precision:** strong for modeled Python typing semantics and local narrowing. Dynamic Python behavior still requires unknowns or modeling.

**Cost model:** designed as Rust-native incremental/module-aware analysis. The expensive areas are type relations, union/intersection normalization, narrowing projection, imports, and recursive definitions.

**Strengths:** best direct implementation reference for polint's Python path because it is Rust-native and modern.

**Weaknesses:** not a general heap points-to engine. It is focused on type checking.

**Lessons for polint:** implement Python places and reachability/narrowing facts before points-to. Reuse the idea of narrowing aliases and projected constraints.

**Native implementation implication:** build polint-owned `PlaceFact`, `PredicateFact`, `ReachabilityFact`, and `NarrowedTypeFact`; do not expose Ty internals.

## Pyrefly

**Language/domain:** Python type checking.

**Role:** Rust-native module-centric checker architecture.

**Key source paths:**

- `ARCHITECTURE.md`
- `crates/pyrefly_graph/src/calculation.rs`
- `crates/pyrefly_types/src/types.rs`
- `crates/pyrefly_types/src/heap.rs`
- `crates/pyrefly_types/src/type_alias.rs`
- `crates/pyrefly_build/src/source_db/*`

**Algorithm shape:** module exports are computed, modules are converted to bindings, and bindings are solved. Recursive cases use `Type::Var` placeholders.

**Fact model:** module handles, bindings, flow types, type heap, recursive variables, source DB.

**Precision:** strong type-checking precision for typed Python, with flow types; not a general points-to tool.

**Cost model:** module-level solving and parallelism, rather than fine-grained per-symbol query solving.

**Strengths:** validates that a Rust-native Python analysis can be practical without delegating to Python.

**Weaknesses:** module-level solving may be too coarse for polint's multi-language fact invalidation unless wrapped in careful cache keys.

**Lessons for polint:** summaries and module-level SCCs are acceptable if cached and validated; recursive variables are useful for cycles.

**Native implementation implication:** use module-level fixed points for Python type facts where simpler than fine-grained queries, but normalize outputs into polint facts.

## Pyright

**Language/domain:** Python type checking.

**Role:** mature flow/narrowing reference.

**Key source paths:**

- `packages/pyright-internal/src/analyzer/binder.ts`
- `packages/pyright-internal/src/analyzer/codeFlowTypes.ts`
- `packages/pyright-internal/src/analyzer/codeFlowEngine.ts`
- `packages/pyright-internal/src/analyzer/checker.ts`
- `packages/pyright-internal/src/analyzer/typeEvaluatorTypes.ts`

**Algorithm shape:** binder creates flow nodes and analyzer computes narrowed types through flow evaluation. The engine supports rich narrowing cases such as type guards, pattern matching, context-manager/finally edges, and reference-specific flow.

**Fact model:** flow nodes, symbols, type evaluator, narrowed type state.

**Precision:** very strong local narrowing and Python typing coverage.

**Cost model:** query/caching heavy; avoids recomputing flow types where possible.

**Strengths:** production maturity and broad Python typing support.

**Weaknesses:** TypeScript implementation not directly reusable in Rust; not a heap points-to engine.

**Lessons for polint:** narrowed type must be keyed by reference/location, not only symbol.

**Native implementation implication:** `NarrowedTypeFact(place, cfg_node)` should be first-class.

## Pyre / Pysa

**Language/domain:** Python type checking and taint/data-flow.

**Role:** interprocedural summary/model reference.

**Key source areas:** `source/analysis`, `source/interprocedural`, `source/taint`, Pysa official implementation docs.

**Algorithm shape:** Pyre type analysis feeds Pysa interprocedural taint analysis with models, summaries, and fixpoints.

**Fact model:** callables, models, taint trees, sources/sinks/sanitizers, summaries.

**Precision:** strong when models exist; limited by framework/source/sink modeling.

**Cost model:** interprocedural fixed point over call graph/summaries.

**Strengths:** model-based security analysis at scale.

**Weaknesses:** requires high-quality models; Python dynamic dispatch remains hard.

**Lessons for polint:** make summaries and model facts part of the engine, not external config bolted on later.

**Native implementation implication:** Rust extensions should emit summary facts with validation and provenance.

## mypy

**Language/domain:** Python type checking.

**Role:** practical binder/narrowing and type semantics reference.

**Key source paths:** `mypy/binder.py`, `mypy/checker.py`, `mypy/reachability.py`, `mypy/subtypes.py`.

**Algorithm shape:** semantic analysis plus type checking with binder-tracked narrowing and reachability.

**Fact model:** symbols, types, binder frames, subtype checks.

**Precision:** strong for many Python typing workflows; intentionally gradual.

**Cost model:** batch/module type checking with caches.

**Lessons for polint:** binding/narrowing is a distinct layer and should not be hidden inside rule execution.

## pytype

**Language/domain:** Python type inference/checking.

**Role:** typegraph/VM-style analysis reference.

**Key source/docs:** `docs/typegraph.md`, `docs/main_loop.md`, `docs/abstract_values.md`, `pytype/typegraph`.

**Algorithm shape:** Python code is interpreted into a typegraph of variables, bindings, origins, and CFG-visible choices.

**Fact model:** CFG nodes, variables, bindings, origins, abstract values.

**Precision:** good inference-oriented model, with explicit binding provenance.

**Cost model:** graph/solver complexity and VM interpretation cost.

**Lessons for polint:** binding origins are critical for diagnostic evidence and provenance.

## TypeScript Compiler

**Language/domain:** TypeScript/JavaScript type checking.

**Role:** structural type system and control-flow narrowing reference.

**Key source paths:** `src/compiler/binder.ts`, `src/compiler/checker.ts`, `src/compiler/types.ts`.

**Algorithm shape:** binder creates flow nodes; checker evaluates flow nodes lazily for a reference to compute narrowed types. Type relations, unions, intersections, conditional types, overloads, and structural checking are handled in the checker.

**Fact model:** symbols, types, flow nodes, type facts, caches.

**Precision:** excellent for TypeScript types and narrowing; not heap points-to.

**Cost model:** can be high for complex type-level programs; uses caches and limits.

**Lessons for polint:** implement TypeScript-style narrowing facts without treating the TS compiler as an alias oracle.

## Oxc

**Language/domain:** JS/TS parser and semantic infrastructure.

**Role:** Rust-native frontend substrate.

**Key source paths:** `crates/oxc_semantic`, `crates/oxc_cfg`, `crates/oxc_resolver`, `crates/oxc_linter`.

**Algorithm shape:** AST parsing, scope/symbol/reference construction, resolver, and CFG infrastructure.

**Fact model:** semantic scopes, symbols, references, AST nodes, CFG blocks/edges.

**Precision:** strong frontend facts; type/points-to coverage is not the goal.

**Cost model:** designed for speed.

**Lessons for polint:** use Oxc to feed polint-owned facts; do not leak Oxc internals through SDK.

## Flow

**Language/domain:** JavaScript type checking.

**Role:** refinement and incremental type-checking reference.

**Algorithm shape:** type inference/checking with refinements and dependency-aware rechecking.

**Lessons for polint:** local refinement and invalidation are architectural concerns, not just type-system details.

## TAJS

**Language/domain:** JavaScript abstract interpretation.

**Role:** deep JS heap/value abstract interpretation reference.

**Key source paths:** `src/dk/brics/tajs/analysis`, `flowgraph`, `lattice`, `js2flowgraph`.

**Algorithm shape:** flow graph plus abstract interpretation over JS value and heap domains.

**Fact model:** abstract values, object labels, heap/state, flow graph.

**Precision:** stronger runtime JS modeling than type checkers, with abstract interpretation tradeoffs.

**Cost model:** higher than syntax/type-only approaches; precision requires domains and widenings.

**Lessons for polint:** abstract domains should be opt-in providers after the first type/value layers.

## Jelly

**Language/domain:** JavaScript/TypeScript call graph and points-to.

**Role:** modern JS/TS function-object/property flow reference.

**Algorithm shape:** pragmatic flow-insensitive/control-flow-informed constraints for JS/TS modules, property accesses, functions, callbacks, and async/module idioms.

**Lessons for polint:** function-object propagation and property-sensitive access paths are the key middle tier for JS call graphs.

## CodeQL

**Language/domain:** multi-language query analysis.

**Role:** typed query APIs, type tracking, API/data-flow modeling.

**Key source areas:** `javascript/ql/lib`, `python/ql/lib`, `java/ql/lib`, data-flow/type-tracking libraries.

**Algorithm shape:** relational facts and query libraries over extracted code databases.

**Fact model:** AST/CFG/call/data-flow/type tracking relations with extensible predicates.

**Precision:** strong when libraries/models exist; query authors can add modeling.

**Cost model:** database extraction plus query evaluation.

**Lessons for polint:** user-facing APIs should be high-level typed views with extension points.

## Go Tools

**Language/domain:** Go.

**Role:** official semantic/SSA/call graph oracle.

**Key source paths:** `go/types`, `go/ssa`, `go/callgraph/static`, `cha`, `rta`, `vta`.

**Algorithm shape:** type checking, SSA IR, static/CHA/RTA/VTA call graph algorithms.

**Fact model:** packages, types, SSA values/instructions, call graph nodes/edges, VTA type propagation graph.

**Precision:** strong for official Go semantics; reflection/unsafe remain difficult.

**Cost model:** package loading and SSA construction cost; VTA benchmark comments show higher time/memory than CHA/RTA.

**Lessons for polint:** Go tools are official language authority, not random OSS analyzers. Use them as validation oracles and, where it makes semantic sense, provider inputs. Always normalize their output into polint-owned facts and keep the precision ladder under polint's fact/provenance model.

## Staticcheck

**Language/domain:** Go static analysis.

**Role:** production rule-engine reference over Go types/SSA.

**Algorithm shape:** analyzers consume Go compiler facts and SSA-like facts for diagnostics.

**Lessons for polint:** many useful Go rules do not need global alias analysis; type/SSA facts and summaries cover a lot.

## Doop

**Language/domain:** Java/JVM points-to/call graph.

**Role:** declarative state-of-the-art Java pointer analysis.

**Key source areas:** `souffle-logic/main`.

**Algorithm shape:** Datalog/Souffle relations compute points-to, call graph, field flow, reflection models, and context-sensitive variants.

**Fact model:** relations such as variable points-to, heap allocation, field points-to, method invocation, call graph edge.

**Precision:** very strong for Java when configuration/models/classpath are good.

**Cost model:** relation/fixpoint evaluation; context sensitivity increases relation size.

**Lessons for polint:** relational sub-engines and model facts are valuable, but public APIs should stay typed.

## WALA

**Language/domain:** Java/JVM and other languages.

**Role:** SSA IR, call graph, pointer analysis framework.

**Algorithm shape:** call graph builders and pointer analysis over pointer keys, instance keys, contexts, and heap abstractions.

**Lessons for polint:** context selectors and instance-key policies should be pluggable precision policies.

## Soot / Spark / SootUp / Qilin

**Language/domain:** Java/JVM.

**Role:** Jimple IR and points-to/call graph framework.

**Algorithm shape:** points-to analysis graph, on-the-fly call graph, field sensitivity options, context sensitivity in Qilin/SootUp.

**Lessons for polint:** field sensitivity and on-the-fly call graph construction should be provider policies with explicit cache keys.

## OPAL

**Language/domain:** JVM bytecode.

**Role:** bytecode/TAC/fixpoint reference.

**Algorithm shape:** bytecode analysis, TAC conversion, property computation/fixpoint scheduling.

**Lessons for polint:** source and bytecode facts should be distinguished; property/fact computation architecture matters.

## Checker Framework

**Language/domain:** Java source-level type qualifier/data-flow.

**Role:** local data-flow and source CFG reference.

**Algorithm shape:** source CFG plus transfer functions for type qualifiers.

**Lessons for polint:** local data-flow and type facts are valuable even without whole-program heap analysis.

## LLVM AliasAnalysis / MemorySSA

**Language/domain:** compiler IR.

**Role:** alias provider stack and sparse memory SSA reference.

**Algorithm shape:** alias queries go through provider stack; MemorySSA tracks memory defs/uses/phis for sparse memory queries.

**Lessons for polint:** alias should be a provider-stack query service; sparse flow-sensitive memory should come later.

## SVF

**Language/domain:** LLVM IR pointer/value-flow.

**Role:** state-of-the-art sparse value-flow graph reference.

**Algorithm shape:** pointer analysis plus memory SSA plus sparse value-flow graph.

**Lessons for polint:** future high precision should be sparse, not dense program-point points-to.

## Rust Borrow Checker / Polonius

**Language/domain:** Rust ownership/borrowing.

**Role:** ownership/loan and relational dataflow reference.

**Algorithm shape:** MIR borrow facts, region/loan liveness, Polonius relational computation.

**Lessons for polint:** ownership facts can prove strong no-alias in languages that support them, but this is not a general alias engine.

## rust-analyzer

**Language/domain:** Rust IDE semantic analysis.

**Role:** incremental semantic database reference.

**Algorithm shape:** query/incremental architecture over parsed/lowered Rust facts.

**Lessons for polint:** stable IDs and invalidation are a first-order design issue.

## Souffle

**Language/domain:** Datalog engine.

**Role:** relational/fixpoint engine reference.

**Algorithm shape:** relations, rules, semi-naive evaluation, compiled queries.

**Lessons for polint:** a relation engine is useful internally for recursive analyses and validation, but should not replace typed SDK views.

## Joern

**Language/domain:** code property graph/static analysis.

**Role:** unified graph query architecture reference.

**Algorithm shape:** AST/CFG/PDG/call/data-flow layers in a queryable graph.

**Lessons for polint:** unified graph identity is powerful, but public raw-graph APIs can become unstable and too broad.
