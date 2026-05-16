# Standard Model: CFG And Control Dependence

This document defines the normalized vocabulary for CFG research and future implementation.

## Terminology

| Term | Meaning |
|---|---|
| Operation node | A fine-grained execution step, often expression-level or statement-level. One AST node may produce zero, one, or many operation nodes. |
| Basic block | A maximal sequence of operation nodes with single-entry sequential execution and no internal branch boundary. |
| CFG edge | Directed transfer of control from one operation/block to another. |
| Normal edge | Ordinary fallthrough or structured successor. |
| Abrupt edge | `return`, `break`, `continue`, `throw`, `panic`, `goto`, `yield`, `await`, cleanup, or language-specific non-fallthrough transfer. |
| Exceptional edge | Transfer caused by a thrown exception, panic, VM exception, rejected await, or implicit runtime exception. |
| Cleanup edge | Transfer through `finally`, `defer`, `with/__exit__`, try-with-resources close, or monitor exit. |
| Virtual entry | Synthetic node/block that starts a body graph. |
| Virtual exit | Synthetic node/block that unifies normal and selected abnormal exits for postdominance. |
| Dominator | `A` dominates `B` if all paths from entry to `B` pass through `A`. |
| Postdominator | `A` postdominates `B` if all selected paths from `B` to the unified exit pass through `A`. |
| Control dependence | A node is control-dependent on a branch/edge if whether it executes depends on that branch outcome. |
| Path evidence | Ordered nodes/edges plus source spans and precision notes used to explain a diagnostic. |

## Fact Families

### `CfgFunctionFact`

Represents an analyzed body:

```rust
struct CfgFunctionFact {
    function_id: FunctionId,
    stable_key: StableFunctionKey,
    language: LanguageId,
    source_set: SourceSetId,
    file_id: FileId,
    span: Span,
    entry_node: CfgNodeId,
    normal_exit_node: CfgNodeId,
    exceptional_exit_node: Option<CfgNodeId>,
    precision: Precision,
    provenance: Provenance,
}
```

Bodies include functions, methods, constructors, module/toplevel bodies, class static blocks, lambda bodies, comprehensions when represented separately, and generated/synthetic bodies when validated.

### `CfgNodeFact`

Represents an operation-level node:

```rust
struct CfgNodeFact {
    id: CfgNodeId,
    function_id: FunctionId,
    block_id: BasicBlockId,
    kind: CfgNodeKind,
    ast_anchor: Option<AstAnchor>,
    source_span: Option<Span>,
    generated: bool,
    operation_index: u32,
    precision: Precision,
    provenance: Provenance,
}
```

Recommended `CfgNodeKind` values:

```text
Entry
ExitNormal
ExitExceptional
Expression
Statement
Condition
CallSite
Return
Throw
Panic
Break
Continue
Goto
Yield
Await
Defer
RunDefers
FinallyEnter
FinallyExit
WithEnter
WithExit
ResourceClose
MonitorEnter
MonitorExit
Synthetic
Unsupported
```

### `BasicBlockFact`

Represents scalable block-level structure:

```rust
struct BasicBlockFact {
    id: BasicBlockId,
    function_id: FunctionId,
    kind: BasicBlockKind,
    first_node: CfgNodeId,
    last_node: CfgNodeId,
    node_range: Range<u32>,
    reachable: Reachability,
    reverse_postorder: u32,
    precision: Precision,
}
```

### `CfgEdgeFact`

Represents typed control transfer:

```rust
struct CfgEdgeFact {
    id: CfgEdgeId,
    function_id: FunctionId,
    from: CfgNodeId,
    to: CfgNodeId,
    from_block: BasicBlockId,
    to_block: BasicBlockId,
    kind: CfgEdgeKind,
    label: Option<EdgeLabel>,
    condition: Option<ExprId>,
    precision: Precision,
    provenance: Provenance,
}
```

Required edge kinds:

```text
Normal
True
False
SwitchCase
DefaultCase
LoopEnter
LoopBack
LoopExit
Break
Continue
Goto
Return
Throw
ImplicitThrow
Panic
Recover
Finally
Cleanup
Defer
ResourceClose
MonitorExit
ShortCircuit
OptionalChain
Nullish
YieldSuspend
YieldResume
AwaitSuspend
AwaitResume
AsyncReject
Spawn
Unreachable
Unknown
Synthetic
Extension
```

Important distinction: `Spawn` is not an intraprocedural successor into the spawned function. It is an event/control-boundary fact. Call graph and lifecycle overlays consume it.

## Precision Labels

| Precision | Meaning |
|---|---|
| `ExactSyntax` | Directly follows syntax and source evaluation order. |
| `ExactLowered` | Follows a well-defined language lowering with stable semantics. |
| `Semantic` | Uses type/lifecycle/package facts to improve edges. |
| `Conservative` | Over-approximates possible control flow. |
| `Heuristic` | Useful but not guaranteed by semantics. |
| `RuntimeDerived` | Learned from traces/generated models. |
| `AgentAsserted` | Added by repo-local Rust provider. |
| `Unsupported` | Recognized but not modeled. |
| `Unknown` | Analyzer cannot classify the edge precisely. |

Control-dependence and postdominance facts must carry the precision of the CFG view they were computed over.

## CFG Views

Do not pretend one graph answers every question. Support named views:

| View | Purpose |
|---|---|
| `normal_only` | Ignore exceptional/implicit throw edges. Useful for simple reachability and style rules. |
| `abrupt_aware` | Include return/break/continue/goto/panic/throw/defer/finally edges. Default for serious rules. |
| `exception_conservative` | Include potential implicit exception edges where a handler/finally can observe them. |
| `async_surface` | Include await/yield suspend/resume markers without modeling scheduler interleavings. |
| `extension_overlay` | Include validated extension-emitted synthetic edges/facts. |

SDK methods should make the selected view explicit or default to a documented conservative view.

## Invariants

Every CFG provider must pass these invariants before facts are accepted:

- one virtual entry node per function/body;
- at least one exit node or an explicit infinite/unsupported marker;
- every edge endpoint exists and belongs to the same function/body unless it is an allowed boundary edge;
- block node ranges are non-empty except synthetic entry/exit blocks;
- nodes have deterministic order;
- edge IDs are deterministic;
- every reachable node is reachable from entry in the selected view;
- no duplicate identical edges after normalization;
- exception/cleanup/finally edges carry a non-normal edge kind;
- extension facts cannot erase native facts without explicit replacement provenance;
- unsupported constructs emit capability/precision facts instead of placeholder exact edges.

## Public SDK Shape

Preferred future shape:

```rust
#[polint::rule]
fn require_check_before_use(
    ctx: &mut RuleCtx<'_>,
    cfg: Cfg<'_>,
    refs: References<'_>,
) -> RuleResult {
    for function in cfg.functions() {
        let graph = cfg.graph(function, CfgView::AbruptAware)?;
        for use_site in refs.uses_in(function) {
            if !graph.is_guarded_by(use_site.node(), "is_authorized") {
                ctx.diagnostic(...);
            }
        }
    }
    Ok(())
}
```

Do not expose mutable graphs, raw parser nodes, or language-tool internals through the public SDK.
