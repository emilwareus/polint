# Go CFG Research

## Recommendation

Design Go CFG facts against `golang.org/x/tools/go/ssa`, not `go/cfg`.

`go/cfg` is useful as a syntactic baseline and lifecycle reference through `go/analysis/passes/ctrlflow`, but `go/ssa` is the analysis-grade reference for short-circuiting, select, defer, panic, recover, and block-level control.

## Inspected Implementations

| System | Source paths | Takeaway |
|---|---|---|
| `go/cfg` | `repos/golang-tools/go/cfg/cfg.go`, `builder.go` | Single-function AST CFG. Fast, simple, but omits key semantic/abnormal flow details. |
| `go/analysis/passes/ctrlflow` | `repos/golang-tools/go/analysis/passes/ctrlflow/ctrlflow.go` | Package-level wrapper around `go/cfg`, including no-return facts. |
| `go/analysis/passes/buildssa` | `repos/golang-tools/go/analysis/passes/buildssa/buildssa.go` | Analysis-framework bridge to `go/ssa`. |
| `go/ssa` | `repos/golang-tools/go/ssa/ssa.go`, `builder.go`, `dom.go`, `block.go` | Best public Go CFG/SSA substrate. |
| Go compiler SSA | `repos/golang-go/src/cmd/compile/internal/ssa` | Strong internal reference for blocks, edges, dominators, layout, and optimization. Not a dependency. |

Source validation found `Function`, `BasicBlock`, `Panic`, `RunDefers`, `Defer`, `builder.cond`, and `builder.selectStmt` in the Go SSA package.

## Precision Notes

### `go/cfg`

Strengths:

- simple CFG over AST statements;
- exposes formatting/dot output;
- integrates with analysis pass facts;
- good for basic reachability examples.

Limits:

- not enough for expression-level short-circuit precision;
- not enough for panic/recover/defer correctness;
- does not model goroutine/control lifecycle;
- should not be polint’s high-capability target.

### `go/ssa`

Strengths:

- explicit `BasicBlock` predecessor/successor graph;
- dominator APIs;
- lowers logical expressions, switches, selects, and loops;
- has instructions for `Panic`, `Defer`, `RunDefers`, `Go`, `Select`, `Recover`;
- better source-position mapping than raw compiler SSA for analysis use.

Limits:

- requires package loading/type information;
- lifecycle inputs such as module roots, build tags, and tests must be handled;
- raw SSA objects are not stable public polint facts;
- panic/recover interprocedural semantics still require precision modes.

## Recommended Native Fact Mapping

```text
ssa.Function       -> CfgFunctionFact
ssa.BasicBlock     -> BasicBlockFact
ssa.Instruction    -> CfgNodeFact
block.Succs/Preds  -> CfgEdgeFact
ssa.Panic          -> Panic edge / exceptional exit
ssa.Defer          -> Defer node
ssa.RunDefers      -> Defer cleanup edge
ssa.Go             -> Spawn fact, not local successor into callee
ssa.Select         -> SelectCase edges
```

## Go Edge Kinds

Required edge kinds for Go:

- `Normal`
- `True`
- `False`
- `LoopBack`
- `LoopExit`
- `Break`
- `Continue`
- `Goto`
- `Fallthrough`
- `SwitchCase`
- `SelectCase`
- `Return`
- `Panic`
- `Recover`
- `Defer`
- `RunDefers`
- `ShortCircuit`
- `Spawn`
- `NoReturn`
- `Unknown`

## Key Constructs

| Construct | Modeling decision |
|---|---|
| `defer` | Emit `Defer` node and synthetic cleanup/`RunDefers` edges. Do not treat as ordinary statement only. |
| `panic` | Edge to exceptional exit by default; recover-aware mode can connect to function recover block where proven. |
| `recover` | Only meaningful in deferred calls; exact interprocedural recovery is future work. |
| `go f()` | Emit spawn fact. Do not add intraprocedural edge to `f`. |
| `select` | Emit nondeterministic `SelectCase` edges. |
| `range` | Loop structure with iterator/assignment operation nodes. |
| `fallthrough` | Explicit switch edge to next case. |
| `goto` | Explicit edge, but validate label target and block split. |
| `os.Exit` / fatal APIs | No-return summaries should be fact layer input, not hardcoded forever. |

## Complexity

For a native Go provider:

- CFG construction: `O(N + E)` where N is lowered operations and E emitted edges.
- Short-circuit lowering adds operation nodes but remains linear.
- `select` and switch add edges proportional to cases.
- Defer/finally-like cleanup can add synthetic edges; cap duplication and label precision.
- Dominance/postdominance derived facts use shared algorithms.

## Implementation Path

1. Use existing Go lifecycle configuration from `[languages.go]`: module roots, package patterns, build tags, include tests.
2. Build a syntax-level Go CFG first for tree-sitter-backed files if semantic package loading is not yet available.
3. Add semantic Go provider once package loading/type facts are stable.
4. Compare fixture output against `go/ssa` for typed packages.
5. Emit `UnsupportedControlFlowFact` instead of placeholder exact CFG when package loading fails.

## Validation Fixtures

Required Go fixtures:

- `if_else_join`
- `short_circuit_and_or`
- `for_break_continue`
- `range_loop`
- `switch_fallthrough`
- `type_switch`
- `select_cases`
- `goto_label`
- `defer_return`
- `panic_defer_recover`
- `go_spawn`
- `known_no_return`
- `build_tags_variants`
- `tests_package_variants`

## Final Go Decision

Use Go SSA as semantic truth for design, but keep polint facts native and stable. `go/cfg` is a reference and fallback for simple syntactic CFG, not the state-of-the-art target.
