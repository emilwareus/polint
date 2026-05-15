# TypeScript / JavaScript CFG Research

## Recommendation

Use Oxc as the Rust-native CFG substrate/reference, CodeQL JS as the semantic coverage reference, TypeScript/Pyright-style flow nodes as the type-narrowing reference, and ESLint code paths as rule-author ergonomics reference.

Do not expose any of these internals directly. Emit polint-owned CFG facts.

## Inspected Implementations

| System | Source paths | Takeaway |
|---|---|---|
| Oxc | `repos/oxc/crates/oxc_cfg`, `repos/oxc/crates/oxc_semantic/src/builder.rs` | Best Rust-native CFG implementation reference. |
| TypeScript compiler | `repos/typescript/src/compiler/binder.ts`, `checker.ts`, `types.ts` | Flow-node graph for narrowing/reachability, not a general CFG API. |
| ESLint | `repos/eslint/lib/linter/code-path-analysis` | Rule-facing code path segment model. |
| CodeQL JS | `repos/codeql/javascript/ql/lib/semmle/javascript/CFG.qll` | Best query-facing JS/TS CFG semantics reference. |
| TAJS | `repos/tajs/src/dk/brics/tajs/flowgraph`, `js2flowgraph` | Abstract interpretation and flow graph reference. |
| Jelly | `repos/jelly/src` | Modern JS/TS pragmatic call graph/points-to constraints; useful for future layers. |

Source validation found Oxc `EdgeType`, `ControlFlowGraph`, `BasicBlock`, error harnesses, finalizers, and append methods for throw/break/continue. It found TypeScript `FlowFlags`, `FlowNode`, `FlowSwitchClause`, `FlowReduceLabel`, binder creation helpers, and checker flow-node evaluation.

## Oxc Findings

Oxc’s CFG crate already has the right broad shape:

- `ControlFlowGraph`
- `BasicBlock`
- typed edge kinds;
- reachability helpers;
- builder methods for error harnesses and finalizers;
- explicit append methods for throw, break, continue, and unreachable flow;
- semantic-builder integration.

This maps well to polint’s current stack. The risk is freezing Oxc internals as public polint facts. Avoid that by translating Oxc concepts into stable `CfgNodeFact`, `BasicBlockFact`, and `CfgEdgeFact`.

## TypeScript Flow Nodes

TypeScript’s compiler builds an internal graph of `FlowNode`s:

- `FlowFlags`
- branch and loop labels;
- assignment, call, condition, switch, reduce labels;
- unreachable flow;
- lazy checker evaluation through `getTypeAtFlowNode`.

This is excellent for future type narrowing because it is reference-specific and demand-driven. It is poor as a general CFG substrate because it is not designed to enumerate all execution paths as blocks/edges.

Polint should copy the architectural split:

```text
CFG facts
  -> type/narrowing facts
```

Do not merge narrowing into CFG.

## ESLint Code Paths

ESLint exposes rule-facing lifecycle events over code path segments. The implementation handles:

- forks and joins;
- current segments;
- final returned/thrown segments;
- loops;
- break/continue/return/throw;
- try/catch/finally;
- logical expressions and optional chains.

This is valuable for SDK ergonomics. Rule authors want simple questions like “is this segment reachable?” or “what path leaves this function?” But polint’s internal CFG needs more structure for dominance, control dependence, path evidence, and data flow.

## CodeQL JS Findings

CodeQL JS exposes `ControlFlowNode`, synthetic entry/exit/guard nodes, successors/predecessors, concrete nodes, and guard/control-flow predicates.

Important semantic lesson: CodeQL explicitly documents that `finally` modeling can admit infeasible paths. It also models implicit exception edges from calls, `new`, property access, and `await` only when an enclosing `try/catch/finally` makes those edges relevant.

Polint should copy that honesty:

- never claim path-sensitive exactness for a merged `finally` graph;
- label implicit exception edges conservative;
- keep path feasibility status separate from reachability.

## Required JS/TS Edge Kinds

- `Normal`
- `True`
- `False`
- `ShortCircuit`
- `Nullish`
- `OptionalChain`
- `LoopBack`
- `LoopExit`
- `SwitchCase`
- `DefaultCase`
- `Break`
- `Continue`
- `Return`
- `Throw`
- `ImplicitThrow`
- `Finally`
- `AwaitSuspend`
- `AwaitResume`
- `AsyncReject`
- `YieldSuspend`
- `YieldResume`
- `FunctionBoundary`
- `Unknown`

## Hard Constructs

| Construct | Modeling decision |
|---|---|
| `&&`, `||`, `??` | Expression-level branches. |
| `&&=`, `||=`, `??=` | Assignment plus short-circuit control. |
| Optional chaining | Split at nullable check; preserve condition edge. |
| `try/finally` | Model cleanup edges and label infeasible merged paths if any. |
| `await` | Evaluate operand, normal continuation, possible rejection/throw in handler-sensitive view; no scheduler model first. |
| Generators | `yield` and `yield*` are suspend/resume nodes; `yield*` can call/throw through iterator protocol. |
| Destructuring/default initializers | May evaluate expressions and throw; include operation nodes. |
| Dynamic import | Call-like/promise-like node; module resolution belongs to module graph. |
| `eval` | Unsupported/dynamic fact; do not claim exact CFG. |
| CommonJS/ESM | Module graph/import facts, not CFG edges. |

## Complexity

JS/TS CFG construction is `O(N + E)` for AST/lowered operation count, but exception and finalizer modeling can add synthetic edges. Avoid duplicating large finalizer bodies by default; use gate/finalizer nodes and label precision. Add body duplication only when needed for exact path evidence and validated by benchmarks.

## Validation Fixtures

Required fixtures:

- `if_else_join`
- `logical_and_or_nullish`
- `optional_chain`
- `logical_assignment`
- `ternary`
- `for_while_do`
- `for_in_for_of`
- `switch_fallthrough`
- `labeled_break_continue`
- `try_catch_finally_return_throw_break`
- `async_await_try`
- `generator_yield_star`
- `destructuring_defaults_throw`
- `class_static_block`
- `top_level_await`
- `type_guard_discriminated_union`

## Final TS/JS Decision

Start with Oxc-backed native facts and CodeQL/ESLint/TypeScript differential validation. Keep TypeScript/Pyright-style narrowing as a separate future analysis layer.
