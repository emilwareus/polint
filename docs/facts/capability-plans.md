# Capability Support

polint derives rule capabilities from typed fact-view parameters in
`#[polint::rule]` function signatures. This planning is an engine detail; normal
users should see it through `polint check` diagnostics rather than a separate
debug command.

## Supported Facts

Current rule-author fact views include:

- `Imports<'_>`
- `ResolvedImports<'_>`
- `ModuleGraphFacts<'_>`
- `GoTests<'_>`
- `BranchObligations<'_>`
- `FileMetrics<'_>`
- `Functions<'_>`
- `FunctionMetrics<'_>`
- `ComplexityMetrics<'_>`
- `Symbols<'_>`
- `References<'_>`
- `StringLiterals<'_>`
- `JsxAttributes<'_>`
- `TsClasses<'_>`
- `TsComponents<'_>`

`References<'_>` implies symbol identity internally, so rules that request only
references still cause polint to derive the `symbols` capability needed to bind
resolved `ReferenceFact::target` values.

## Phase 55 Preview Facts

Phase 55 adds a small policy-query vocabulary to the public SDK:

- `Events<'_>` derives capability `events`
- `Calls<'_>` derives capability `calls`
- `ControlFlow<'_>` derives capability `control_flow`
- `DataFlow<'_>` derives capability `dataflow`

These capabilities are preview and fail closed in Phase 55. A rule can compile,
appear in `polint inspect rule --format json`, and show derived fact views, but
`polint check` reports `polint/capability` and does not execute the rule until
the corresponding provider-backed behavior lands in later phases.

```rust
#[polint::rule(id = "local/no-secret-logs", description = "Secret logs", severity = "error")]
pub(crate) fn no_secret_logs(ctx: &mut RuleCtx<'_>, flow: DataFlow<'_>) -> RuleResult {
    let mut query = FlowQuery::new(
        SourcePattern::secret_like(["token", "password"]),
        SinkPattern::logger(),
    );
    query.barriers = BarrierPattern::call_any(["redact", "mask_secret"]);

    for violation in flow.forbidden(query) {
        ctx.report(violation.diagnostic(ctx.rule_id(), "secret reaches logs"));
    }

    Ok(())
}
```

This is the only public query-object style for the preview surface:
`Query::new(required...)`, explicit option fields, one view method, and
`PolicyViolation::diagnostic(...)`.

## Reserved Capabilities

Reserved raw capabilities such as `cfg`, `call_graph`, `coverage_facts`, and
`test_suite_metrics` must stay unsupported until a rule can consume real public
SDK facts for them. `Cfg<'_>` and `CallGraph<'_>` are not aliases for
`ControlFlow<'_>` or `Calls<'_>`.

Rules that request unsupported or setup-missing hard capabilities produce
`polint/capability` diagnostics during `polint check` and are not executed with
placeholder facts.
