# Repository Index

Reference implementations were cloned under `research/abstract-interpretation/repos/`.
The folder is gitignored by `research/*/repos/`.

## Cloned Repositories

| Repo | Commit | URL | Why Inspected |
|---|---:|---|---|
| Infer | `01a41d7` | <https://github.com/facebook/infer.git> | Abstract domain framework, Pulse, summaries, biabduction/resource models. |
| LLVM / Clang | `f5f4934` | <https://github.com/llvm/llvm-project.git> | Clang Static Analyzer, ProgramState, ExplodedGraph, range constraints, resource checkers. |
| rustc | `3514361` | <https://github.com/rust-lang/rust.git> | MIR dataflow, initializedness, move paths, borrow checker, const propagation. |
| Checker Framework | `0be8d5a` | <https://github.com/typetools/checker-framework.git> | Java dataflow framework, nullness, resource/must-call, called-methods. |
| TypeScript | `f350b52` | <https://github.com/microsoft/TypeScript.git> | Flow nodes, narrowing, control-flow type facts. |
| Pyright | `b13157b` | <https://github.com/microsoft/pyright.git> | Python code-flow engine, type guards, TypedDict, narrowing. |
| Pyre | `34af372` | <https://github.com/facebook/pyre-check.git> | Python type checking, CFG, Pysa taint domains and summaries. |
| Ruff / Ty | `a7ab646` | <https://github.com/astral-sh/ruff.git> | Rust-native Python semantic/type model and narrowing direction. |
| mypy | `1bbd6da` | <https://github.com/python/mypy.git> | Python binder, narrowing, type meet/join, TypedDict. |
| Goblint | `1afc78c` | <https://github.com/goblint/analyzer.git> | Abstract interpretation framework, domains, solver, product analyses. |
| IKOS | `ac7f7c1` | <https://github.com/NASA-SW-VnV/ikos.git> | Abstract-domain traits, fixpoint iterator, numeric/nullity/lifetime domains. |
| Apron | `78f8369` | <https://github.com/antoinemine/apron.git> | Numerical domain common API, intervals, octagons, polyhedra. |
| ELINA | `f524156` | <https://github.com/eth-sri/ELINA.git> | Optimized numerical domains, octagon/polyhedra implementation. |
| Jelly | `b799ed4` | <https://github.com/cs-au-dk/jelly.git> | JS/TS approximate interpretation and call graph analysis. |
| TAJS | `3bdf55a` | <https://github.com/cs-au-dk/TAJS.git> | JavaScript abstract interpretation value/state lattice. |

## Key Inspected Source Paths

### Infer

- `infer/src/absint/AbstractDomain.mli`
- `infer/src/absint/AbstractInterpreter.ml`
- `infer/src/absint/TransferFunctions.mli`
- `infer/src/backend/Summary.ml`
- `infer/src/pulse/PulseAbductiveDomain.ml`
- `infer/src/bufferoverrun/bufferOverrunDomain.ml`

### Clang Static Analyzer

- `clang/include/clang/StaticAnalyzer/Core/PathSensitive/ProgramState.h`
- `clang/include/clang/StaticAnalyzer/Core/PathSensitive/ExplodedGraph.h`
- `clang/lib/StaticAnalyzer/Core/RangeConstraintManager.cpp`
- `clang/lib/StaticAnalyzer/Core/RegionStore.cpp`
- `clang/lib/StaticAnalyzer/Checkers/MallocChecker.cpp`
- `clang/lib/StaticAnalyzer/Checkers/StreamChecker.cpp`
- `clang/lib/StaticAnalyzer/Checkers/NullabilityChecker.cpp`

### rustc

- `compiler/rustc_mir_dataflow/src/framework/mod.rs`
- `compiler/rustc_mir_dataflow/src/framework/lattice.rs`
- `compiler/rustc_mir_dataflow/src/impls/initialized.rs`
- `compiler/rustc_borrowck/src/dataflow.rs`
- `compiler/rustc_borrowck/src/borrow_set.rs`
- `compiler/rustc_borrowck/src/places_conflict.rs`
- `compiler/rustc_const_eval/src/check_consts/qualifs.rs`
- `compiler/rustc_mir_transform/src/dataflow_const_prop.rs`

### Checker Framework

- `dataflow/src/main/java/org/checkerframework/dataflow/analysis/AbstractAnalysis.java`
- `dataflow/src/main/java/org/checkerframework/dataflow/analysis/Store.java`
- `dataflow/src/main/java/org/checkerframework/dataflow/analysis/TransferFunction.java`
- `checker/src/main/java/org/checkerframework/checker/nullness/NullnessTransfer.java`
- `checker/src/main/java/org/checkerframework/checker/calledmethods/CalledMethodsTransfer.java`
- `checker/src/main/java/org/checkerframework/checker/mustcall/MustCallTransfer.java`
- `checker/src/main/java/org/checkerframework/checker/resourceleak/ResourceLeakChecker.java`

### TypeScript / Pyright / Python

- `TypeScript/src/compiler/types.ts`
- `TypeScript/src/compiler/binder.ts`
- `TypeScript/src/compiler/checker.ts`
- `pyright/packages/pyright-internal/src/analyzer/codeFlowTypes.ts`
- `pyright/packages/pyright-internal/src/analyzer/codeFlowEngine.ts`
- `pyright/packages/pyright-internal/src/analyzer/typeGuards.ts`
- `pyre-check/source/analysis/typeCheck.ml`
- `pyre-check/source/analysis/cfg.ml`
- `pyre-check/source/interprocedural_analyses/taint/domains.ml`
- `ruff/crates/ty_python_semantic/src/types.rs`
- `ruff/crates/ty_python_semantic/src/types/narrow.rs`
- `ruff/crates/ty_python_semantic/src/reachability.rs`
- `mypy/mypy/binder.py`
- `mypy/mypy/types.py`
- `mypy/mypy/meet.py`

### Numeric / AI Frameworks

- `goblint/src/framework/analyses.ml`
- `goblint/src/cdomain/value/cdomains/int/intervalDomain.ml`
- `ikos/core/include/ikos/core/domain/abstract_domain.hpp`
- `ikos/core/include/ikos/core/fixpoint/fwd_fixpoint_iterator.hpp`
- `ikos/core/include/ikos/core/domain/numeric/interval.hpp`
- `ikos/core/include/ikos/core/domain/numeric/octagon.hpp`
- `apron/apron/ap_manager.h`
- `apron/octagons/oct_transfer.c`
- `apron/newpolka/pk_widening.c`
- `ELINA/elina_auxiliary/elina_manager.h`
- `ELINA/elina_oct/opt_oct_transfer.c`
- `ELINA/elina_oct/opt_oct_closure_dense.c`

## Clone Notes

Some large repositories needed sparse checkout and partial clone. Git object
metadata was sufficient to inspect the relevant files with `git show` even when
some checkout materialization failed during the first parallel clone attempt.
