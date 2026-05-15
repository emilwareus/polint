# Python CFG Research

## Recommendation

When Python support is added, build a CodeQL-style source CFG with operation nodes, basic blocks, explicit exceptional edges, and separate type/narrowing facts.

Do not use CPython bytecode as the public rule-facing CFG. Use CPython as a semantic reference.

## Inspected Implementations

| System | Source paths | Takeaway |
|---|---|---|
| CodeQL Python | `repos/codeql/python/ql/lib/semmle/python/Flow.qll` | Best query-facing source CFG API. |
| Pyright | `repos/pyright/packages/pyright-internal/src/analyzer/codeFlowTypes.ts`, `binder.ts`, `codeFlowEngine.ts` | Best practical flow-sensitive type narrowing reference. |
| Pyre/Pysa | `repos/pyre-check/source/analysis/cfg.ml`, `fixpoint.ml` | Explicit Python CFG and fixpoint architecture. |
| mypy | `repos/mypy/mypy/binder.py`, `checker.py`, `reachability.py` | Mature binder/narrowing/reachability model, not a CFG API. |
| CPython | `repos/cpython/Python/flowgraph.c`, `codegen.c`, `bytecodes.c` | Runtime/bytecode semantic reference. |

Source validation found CodeQL `ControlFlowNode`, `BasicBlock`, exceptional successors, and dominance predicates. It found Pyright `FlowFlags`, `PreFinallyGate`, `PostFinally`, `PostContextManager`, and `NarrowForPattern`. It found Pyre `Node.kind` variants including `Dispatch`, `Try`, `With`, and `Final`, and `fixpoint.ml` weak-topological/fixpoint machinery.

## CodeQL Python Findings

CodeQL’s Python CFG is the best public design reference:

- AST nodes can map to zero, one, or many flow nodes.
- `ControlFlowNode` exposes normal, true, false, and exceptional successors.
- `BasicBlock` groups flow nodes for scalable dominance/reachability.
- Exception edges are first-class.
- Dominance is query-facing.

This many-to-one and one-to-many relationship is essential. Python constructs like `try/finally`, `with`, boolean expressions, comprehensions, and pattern matching cannot be modeled accurately with one CFG node per AST node.

## Pyright Findings

Pyright builds a flow graph optimized for type narrowing and reachability:

- `FlowFlags`
- branch/loop labels;
- assignment/call/condition nodes;
- `PreFinallyGate` and `PostFinally`;
- `PostContextManager`;
- `NarrowForPattern`;
- `ExhaustedMatch`;
- lazy backward walking with caches.

This should inspire a future `TypeNarrowing<'_>` layer. It should not replace CFG because it is reference-query driven and not a complete block graph.

## Pyre Findings

Pyre has an explicit CFG:

- integer-ID nodes;
- node kinds such as `Entry`, `Final`, `Try`, `With`, `Dispatch`, `If`, `For`, `While`, `Join`;
- synthetic conditions for match/pattern handling;
- forward/backward fixpoint analysis using weak topological ordering and widening.

This is a good reference for polint’s future data-flow engine after CFG stabilizes.

## CPython Findings

CPython compiles AST to bytecode using a CFG in `flowgraph.c` and lowering in `codegen.c`.

Relevant semantic constructs:

- `basicblock`;
- `basicblock_add_jump`;
- `normalize_jumps`;
- `mark_except_handlers`;
- `label_exception_targets`;
- `calculate_stackdepth`;
- `remove_unreachable`;
- `optimize_cfg`;
- `codegen_try_finally`;
- `codegen_with`;
- `codegen_match`;
- bytecodes such as `GET_AWAITABLE`, `YIELD_VALUE`, `WITH_EXCEPT_START`.

The Python `dis` documentation warns bytecode is CPython-specific and can change across releases. That makes bytecode a semantic oracle, not a stable public analysis representation.

## Required Python Edge Kinds

- `Normal`
- `True`
- `False`
- `LoopBack`
- `LoopExit`
- `Break`
- `Continue`
- `Return`
- `Raise`
- `ImplicitRaise`
- `Finally`
- `WithEnter`
- `WithExit`
- `ExceptionSuppressed`
- `YieldSuspend`
- `YieldResume`
- `AwaitSuspend`
- `AwaitResume`
- `AsyncReject`
- `ComprehensionScope`
- `MatchCase`
- `Unknown`

## Hard Constructs

| Construct | Modeling decision |
|---|---|
| `try/except/else/finally` | Explicit exceptional and cleanup edges. |
| `except*` | Separate exception-group handling; unsupported if not modeled. |
| `with` | Model `__enter__`, body, `__exit__`, and possible exception suppression. |
| `async with` | Same plus await/suspend markers. |
| `yield` / `yield from` | Suspend/resume boundary and value-flow point. |
| `await` | Suspend boundary; external mutation invalidation belongs to narrowing/effects. |
| comprehensions | Nested scope/subgraph or expression-local CFG with scope metadata. |
| `match` | Control flow plus pattern narrowing; protocol details need precision labels. |
| dynamic import/eval/exec | Unknown/dynamic facts. |
| `NoReturn` | Type/effect summary input to reachability. |

## Complexity

Source CFG construction is `O(N + E)`. Python can inflate E through exception/finally/with modeling and comprehension subgraphs. Avoid full bytecode desugaring in the source CFG; represent source-level operations plus precision labels.

## Validation Fixtures

Required fixtures:

- `if_else_join`
- `bool_short_circuit`
- `while_for_break_continue`
- `try_except_else_finally`
- `return_through_finally`
- `with_suppression`
- `async_with`
- `yield_yield_from`
- `async_generator`
- `comprehension_scope`
- `match_guard_capture`
- `raise_from`
- `except_star`
- `no_return`
- `eval_exec_unknown`

## Final Python Decision

When Python is added, implement a source CFG like CodeQL, keep type narrowing as a separate layer like Pyright/mypy, and use CPython only as a semantic reference for tricky runtime lowering.
