# Java / JVM CFG Research

## Recommendation

Java should eventually support two precision tiers:

```text
java/source-cfg
java/bytecode-cfg
```

Source CFG is best for repo-local policy rules and diagnostics. Bytecode CFG is best for JVM-exact control flow, exceptional flow, desugared `finally`, try-with-resources, synchronized, lambdas, and generated paths.

Do not claim exact Java control dependence from a source-only adapter until `finally`, try-with-resources, synchronized, exceptional exits, lambdas, and multiple-exit postdominance are validated.

## Inspected Implementations

| System | Source paths | Takeaway |
|---|---|---|
| Checker Framework Dataflow | `repos/checker-framework/dataflow/src/main/java/org/checkerframework/dataflow/cfg` | Best source-level Java CFG reference. |
| Soot | `repos/soot/src/main/java/soot/toolkits/graph` | Mature Jimple/bytecode exceptional CFG and PDG reference. |
| SootUp | `repos/sootup/sootup.core/src/main/java/sootup/core/graph` | Modern Soot graph API with normal/exceptional successors. |
| WALA | `repos/wala/core/src/main/java/com/ibm/wala/cfg`, `ssa`, `ipa/cfg` | SSA CFG, exploded CFG, exception-pruned CFG, control dependence. |
| OPAL | `repos/opal/OPAL/br`, `OPAL/ai`, `OPAL/tac` | Bytecode/TAC CFG, postdominance/control-dependence reference. |
| CodeQL Java | `repos/codeql/java/ql/lib/semmle/code/java` | Query-facing source/control-flow and dominance predicates. |

Source validation found Soot `ExceptionalUnitGraph`, `buildExceptionDests`, `buildExceptionalEdges`, `mightHaveSideEffects`, `MHGPostDominatorsFinder`, and `HashMutablePDG`. It found SootUp `ControlFlowGraph`, `MutableBlockControlFlowGraph`, `exceptionalSuccessors`, and a postdominator caveat for multiple tail blocks. It found WALA `ShrikeCFG`, `SSACFG`, `ExceptionPrunedCFG`, `ExplodedControlFlowGraph`, and `ControlDependenceGraph`. It found Checker Framework `ControlFlowGraph`, `CFGTranslationPhaseOne`, `TryStack`, `visitTry`, `visitSynchronized`, and `extendWithNodeWithExceptions`.

## Java Semantics That Matter

Java CFG has to model abrupt completion:

- `break`
- `continue`
- `return`
- `throw`
- VM/runtime exceptions
- `finally` overriding prior abrupt completion
- try-with-resources close ordering and suppressed exceptions
- `synchronized` monitor enter/exit on normal and abrupt paths
- lambdas/method references through `invokedynamic`

The JLS and JVMS are primary references for these semantics.

## Checker Framework Findings

Checker Framework is the strongest source-level CFG reference:

- `ControlFlowGraph` has entry, regular exit, and exceptional exit blocks;
- `CFGTranslationPhaseOne` lowers Java AST constructs;
- method invocations can extend with exception edges;
- `try`, resources, `finally`, synchronized blocks, lambdas, and throws are represented;
- dataflow analyses consume the CFG.

Product lesson: a source CFG can be very useful for rules, but exception modeling is conservative and tied to source-level type information.

## Soot / SootUp Findings

Soot’s `ExceptionalUnitGraph` is the canonical exceptional JVM CFG reference. It distinguishes:

- explicit throw edges;
- implicit VM exception edges;
- handler/trap destinations;
- side-effect-sensitive exception edges.

SootUp modernizes the API and exposes normal and exceptional successor queries. The source has a caveat that postdominance can be incorrect with multiple tail blocks, which validates polint’s requirement for artificial unified exits.

Product lesson: JVM CFG needs explicit normal and exceptional edge APIs; postdominance must handle multiple exits carefully.

## WALA Findings

WALA provides:

- bytecode CFG (`ShrikeCFG`);
- SSA CFG (`SSACFG`);
- exceptional successors/predecessors;
- exception-pruned views;
- exploded instruction-level CFG;
- control-dependence graph computed over reverse dominance frontiers.

Product lesson: it is useful to provide multiple graph views. Some analyses intentionally ignore exceptional edges, but the view name must say so.

## OPAL Findings

OPAL is valuable for:

- bytecode CFG construction;
- TAC/SSA;
- abstract-interpretation-informed control flow;
- artificial exits for postdominance;
- control-dependence computations.

Product lesson: bytecode CFG can be precise but expensive and source-noisy. It should be a separate capability.

## Required Java Edge Kinds

- `Normal`
- `True`
- `False`
- `SwitchCase`
- `LoopBack`
- `LoopExit`
- `Break`
- `Continue`
- `Return`
- `Throw`
- `ImplicitException`
- `Catch`
- `Finally`
- `ResourceClose`
- `SuppressedException`
- `MonitorEnter`
- `MonitorExit`
- `LambdaBody`
- `InvokeDynamic`
- `ClassInit`
- `Unknown`

## Precision Split

| Capability | Good for | Limits |
|---|---|---|
| `java/source-cfg` | source diagnostics, repo rules, annotations, source-level policy | conservative exceptions, generated/desugared paths may be missing |
| `java/bytecode-cfg` | JVM-exact handlers, resources, synchronized, generated code, classpath behavior | source mapping and diagnostics harder |

## Complexity

Source CFG construction is roughly `O(N + E)` over AST/lowered nodes and emitted edges. Bytecode CFG is `O(I + H + E)` where I is instructions, H is handlers/traps, and E includes normal plus exceptional edges. Exception edge explosion is the main risk; Soot-style side-effect-sensitive and declared-exception pruning are important.

## Validation Fixtures

Required Java fixtures:

- `if_else_join`
- `for_while_enhanced_for`
- `switch_statement_expression`
- `break_continue_labels`
- `throw_catch_finally`
- `finally_overrides_return`
- `try_with_resources_suppressed`
- `multi_catch`
- `synchronized_block`
- `synchronized_method`
- `lambda_method_reference`
- `invokedynamic_summary`
- `implicit_null_pointer_exception`
- `class_initialization`
- `multiple_tail_postdominance`

## Final Java Decision

When Java is added, start with a source CFG for rule ergonomics and diagnostics. Add bytecode CFG as a separate high-precision capability. Do not blend the two without provenance and precision labels.
