# Validation Notes

## Source Validation Performed

### Repository snapshots

Each cloned repository was checked for a commit hash and stored in `REPO-INDEX.md`. Repositories live under the ignored `repos/` directory.

### Ty / Ruff

`rg` validation confirmed:

- `ty_python_core/src/reachability_constraints.rs` contains core reachability constraint data structures and normalization.
- `ty_python_semantic/src/reachability.rs` documents and implements reachability constraints, binding reachability, narrowing projection, and `narrow_by_constraint`.
- `ty_python_core/src/builder.rs` contains `NarrowingAlias`, `narrowing_aliases`, `alias_predicates`, alias invalidation, place definitions, and use/definition recording.
- `ty_python_core/src/predicate.rs` connects predicates to reachability and type narrowing.

### Pyrefly

`git show HEAD:ARCHITECTURE.md` confirmed:

- Pyrefly computes exports, converts modules to bindings, then solves bindings.
- It uses flow types.
- It targets module-level incrementality/parallelism.
- Recursive cases use `Type::Var` placeholders.

Sparse checkout did not leave `ARCHITECTURE.md` in the worktree, so the file was read through `git show`.

### TypeScript

`rg` validation confirmed:

- `binder.ts` defines `createFlowNode`, `createFlowCondition`, `createFlowMutation`, `createFlowCall`, branch/loop/reduce labels, and assignment/call flow nodes.
- `types.ts` defines `FlowFlags` and `FlowNode`.
- `checker.ts` defines `flowTypeCache`, `getTypeAtFlowNode`, `getTypeAtFlowAssignment`, `getTypeAtFlowCall`, `getTypeAtFlowCondition`, `narrowTypeByEquality`, `narrowTypeByInstanceof`, `narrowTypeByInKeyword`, and discriminant narrowing helpers.

### Go

`rg` validation confirmed:

- `go/callgraph/static`, `cha`, `rta`, and `vta` exist in the current `golang-tools` snapshot.
- VTA comments describe "Practical Virtual Type Analysis", type/function-literal propagation, and call resolution.
- `go/callgraph/callgraph_test.go` includes benchmark comments and explicitly says the algorithms are unsound with respect to reflection.
- No current `go/pointer` directory exists under `golang-tools/go`.

This corrects the common but stale claim that current `golang.org/x/tools/go/pointer` is part of the active x/tools tree.

### Java/JVM

Source inspection confirmed:

- SootUp/Spark exposes field-sensitivity and on-the-fly call graph options.
- Soot/Spark has points-to analysis graph structures.
- Doop has Souffle/Datalog rules for points-to, call graph, field flow, and reflection modeling.
- WALA contains pointer/call graph infrastructure.
- Checker Framework contains source CFG/data-flow infrastructure.

### LLVM/SVF/Rust

Source inspection confirmed:

- LLVM AliasAnalysis is a provider-style query interface with conservative defaults.
- LLVM MemorySSA documentation describes MemoryDef/MemoryUse/MemoryPhi and intraprocedural sparse memory representation.
- SVF builds memory SSA and sparse value-flow graph infrastructure.
- Rust borrowck/Polonius are ownership/loan analyses, not general cross-language alias analysis.

## Paper/Doc Validation

Downloaded papers and official docs are listed in `PAPER-INDEX.md`.

Important validation decisions:

- Official documentation and source snapshots are treated as stronger evidence than secondary summaries.
- Recent arXiv preprints are used for trend awareness, not as primary implementation authority.
- The Smaragdakis/Balatsouras pointer-analysis survey was identified as relevant but not downloaded because the PDF fetch returned 403. Do not claim it was locally mirrored.

## Accuracy Caveats

- "State of the art" differs by language. A Python type checker, a Java pointer-analysis framework, and LLVM AliasAnalysis solve different problems.
- Many tools optimize for type checking, not alias analysis.
- Many academic pointer-analysis tools assume closed-world programs and lower-level IRs. polint targets repo-local policy checks in real multi-language repositories, often with partial code and framework conventions.
- Dynamic language facts must be precision-labeled. Avoid exact claims for monkeypatching, reflection, dynamic property keys, `eval`, import hooks, proxies, and generated APIs unless modeled.
- `MustAlias` should be rare. `MayAlias` and `Unknown` are honest default answers when facts are incomplete.

## Validation Commands Worth Re-Running

```bash
git -C research/type-alias-points-to/repos/ruff rev-parse --short=12 HEAD
rg -n "NarrowingAlias|reachability|narrow_by_constraint|alias_predicates" research/type-alias-points-to/repos/ruff/crates/ty_python_core/src research/type-alias-points-to/repos/ruff/crates/ty_python_semantic/src

git -C research/type-alias-points-to/repos/typescript rev-parse --short=12 HEAD
rg -n "FlowFlags|createFlowNode|getTypeAtFlowNode|narrowTypeBy" research/type-alias-points-to/repos/typescript/src/compiler/{binder.ts,checker.ts,types.ts}

git -C research/type-alias-points-to/repos/golang-tools rev-parse --short=12 HEAD
find research/type-alias-points-to/repos/golang-tools/go/callgraph -maxdepth 2 -type f
test ! -d research/type-alias-points-to/repos/golang-tools/go/pointer
```

## Final Confidence

| Area | Confidence | Reason |
|---|---|---|
| Type/value before points-to recommendation | High | Consistent across Ty, Pyright, TypeScript, CodeQL, Go, and Java tools. |
| Andersen as bounded baseline | High | Classic and still useful, with known engineering requirements. |
| Alias as provider-stack query | High | LLVM and practical solver architecture support this. |
| Sparse flow-sensitive later | High | LLVM MemorySSA and SVF validate this direction. |
| Go current `go/pointer` absence | High | Direct local source validation. |
| Exact algorithmic complexity details for every tool | Medium | Production tools have many optimizations and undocumented heuristics. |
| Recent preprint impact | Medium/Low | Scanned for direction, not mature product guidance. |
