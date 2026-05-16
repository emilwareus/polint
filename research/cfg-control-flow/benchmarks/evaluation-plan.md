# Evaluation Plan: CFG And Control Dependence

## Strategy

Use layered validation. There is no single external benchmark that gives exact CFG ground truth across languages.

```text
micro CFG fixtures
  + semantic rule oracles
  + differential checks against mature tools
  + external corpora for end-to-end behavior
  + performance/cache regression gates
```

## Fixture Matrix

### Common Fixtures

- `if_else_join`
- `nested_conditionals`
- `short_circuit`
- `ternary_conditional`
- `loops`
- `labeled_flow`
- `switch_match`
- `return_throw_exit`
- `try_catch_finally`
- `closures`
- `unreachable`
- `parse_error_recovery`

### Go

- `defer_order`
- `named_return_modified_by_defer`
- `panic_recover`
- `select`
- `type_switch`
- `fallthrough`
- `goto`
- `range`
- `short_circuit`
- `os_exit_no_return`
- `build_tags`
- `test_variants`

### TS/JS

- `try_finally_return_throw_break`
- `async_await`
- `generator_yield_star`
- `optional_chaining`
- `nullish_coalescing`
- `logical_assignment`
- `destructuring_default_initializer`
- `class_static_block`
- `top_level_await`
- `type_guard_discriminated_union`

### Java

- `explicit_implicit_exception`
- `multi_catch`
- `try_with_resources`
- `suppressed_close_paths`
- `synchronized_block`
- `enhanced_for`
- `switch_expression`
- `lambda_method_reference`
- `finally_overrides_return`
- `multiple_tail_postdominance`

### Python

- `try_except_else_finally`
- `with_exit_suppression`
- `raise_from`
- `except_star`
- `match`
- `comprehension_scope`
- `generator_yield_from`
- `async_generator`
- `async_with`
- `async_for`
- `no_return`

## Snapshot Format

Use normalized JSON/TOML-like snapshots:

```text
function key
nodes:
  id, kind, source anchor, block, precision
blocks:
  id, kind, node range, reachable
edges:
  id, from, to, kind, label, precision
derived:
  idom, ipostdom, control-dependence
unsupported:
  construct, span, reason
```

Avoid raw line-only IDs. Use stable anchors:

```text
file path + function stable key + statement index + span hash
```

## Differential Oracles

| Language | Differential references |
|---|---|
| Go | `go/ssa`, `go/cfg`, CodeQL Go control-flow tests |
| TS/JS | Oxc CFG, ESLint code-path tests, TypeScript compiler tests, CodeQL JS tests |
| Python | CodeQL Python control-flow tests, Pyright samples, Pyre tests, CPython bytecode for selected semantic cases |
| Java | Checker Framework CFG, Soot/SootUp/WALA/OPAL fixtures, CodeQL Java control-flow tests |

Mismatches should be review signals, not automatic failures. Each tool has different precision and view choices.

## External Corpora

Use external corpora mostly for end-to-end behavior, not exact CFG shape:

- Test262 for JS syntax/control construct coverage.
- CodeQL library tests for flow/query behavior.
- OWASP Benchmark Java for diagnostic precision/recall.
- Juliet Java for synthetic data/control variants.
- DroidBench for lifecycle/callback/taint interaction.
- SecBench.js for executable server-side JS vulnerabilities.
- Pyright samples for Python narrowing/reachability constructs.
- Pyre/Pysa integration tests for taint/fixpoint behavior.

## Metrics

CFG metrics:

- node count;
- edge count by kind;
- reachable/unreachable nodes;
- entry/exit count;
- SCC count;
- loop back edges;
- exceptional edge count;
- cleanup edge count;
- unsupported construct count.

Derived metrics:

- dominator agreement on differential fixtures;
- postdominator agreement where available;
- control-dependence edge count;
- path evidence length;
- infeasible/unchecked path labels.

Product metrics:

- precision/recall/F1 for rules using CFG;
- default-vs-extension improvement;
- number of unknowns removed by extension;
- false-negative risk from extension suppressions;
- runtime/memory/cache hit rate.

## Gates

### PR Gate

- all micro snapshots pass;
- CFG invariant validator passes;
- deterministic output hash stable under parallel execution;
- panic isolation passes;
- new construct support has at least one fixture;
- unsupported constructs documented.

### Nightly Gate

- CodeQL/ESLint/TypeScript/Pyright/Go SSA differential subset;
- external corpus smoke runs;
- parser/CFG fuzzing with timeout;
- performance regression report.

### Release Gate

- full fixture matrix green for supported languages;
- no high-severity validation gaps;
- public docs updated;
- cache-key regression tests pass;
- default-vs-extension delta report generated;
- unsupported/heuristic behavior list is explicit.
