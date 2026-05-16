# Standard Research Vocabulary

Use this vocabulary when comparing call graph implementations.

## Core Objects

```python
CallSite(
    id,
    file,
    span,
    enclosing_callable,
    callee_syntax,
    receiver_syntax,
    argument_syntax,
)

Callable(
    id,
    language,
    kind,          # function, method, constructor, module, lambda, synthetic
    symbol_id,
    file,
    span,
    signature,
)

CallEdge(
    site_id,
    caller_id,
    callee_id,     # optional for unresolved edges
    kind,          # direct, method, constructor, virtual, interface, higher_order, implicit, dynamic
    algorithm,     # syntactic, binding, cha, rta, vta, points_to, type_tracking, heuristic, repo_model
    confidence,    # exact, high, medium, low, unknown
    status,        # resolved, ambiguous, unresolved, unsupported, setup_missing
    reason,        # e.g. dynamic_import, missing_classpath, computed_property
    provider,
    provenance,    # native, builtin_model, repo_model, agent_generated_model
    model_id,      # optional; present for repo-local model facts
    validation,    # native, validated, unvalidated, failed
)
```

## Precision Labels

- `syntactic`: the parser saw a call-like expression, but target resolution may be absent.
- `bound`: lexical/import/type binding found a declaration.
- `direct`: target is statically known by language semantics.
- `possible`: one of multiple potential runtime targets.
- `ambiguous`: analysis found targets, but cannot choose one.
- `unresolved`: the call exists, but no target could be found.
- `unsupported`: language feature is known but not modeled by this provider.
- `setup_missing`: external setup such as module roots, classpath, type environment, or build output is missing.
- `repo_model`: edge was emitted by a repo-local model that bound to native facts.

## Standard Implementation Template

Each inspected implementation should be described with:

1. **What it builds**: call sites, function-to-function edges, call-site-to-function edges, context-sensitive nodes, or graph export.
2. **Inputs**: source, bytecode, type checker, package loader, classpath, build database, installed dependencies.
3. **Algorithm family**: syntactic, name binding, CHA, RTA, VTA, points-to, abstract interpretation, Datalog, query/dataflow.
4. **Entry model**: whole repository, configured entrypoints, main/test roots, public API roots, query-driven/on-demand.
5. **Dynamic feature handling**: reflection, function values, methods, framework callbacks, imports, generated code.
6. **Uncertainty model**: missing edges, explicit unresolved facts, imprecision flags, warnings, diagnostics.
7. **Extension model**: whether repo-local call edges, framework semantics, generated-code semantics, or entrypoints can be supplied externally.
8. **Cost profile**: expected speed, memory, whole-program requirements, incremental viability.
9. **Polint lesson**: what to copy, what to avoid.

## Minimal Provider Interface

```python
class CallGraphProvider:
    language: str
    algorithm: str

    def required_inputs(self) -> list[str]:
        ...

    def available(self, repo_context) -> bool:
        ...

    def emit_call_sites(self, file_facts) -> list[CallSite]:
        ...

    def resolve_edges(self, facts, setup) -> list[CallEdge]:
        ...

    def diagnostics(self) -> list[CapabilityDiagnostic]:
        ...
```

This interface keeps cheap call-site extraction separate from expensive target resolution.

Repo-local model providers should use the same output shape, but must emit `model_id`, provenance, and validation status.
