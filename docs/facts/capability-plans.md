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
- `Packages<'_>`
- `Symbols<'_>`
- `References<'_>`
- `StringLiterals<'_>`
- `JsxAttributes<'_>`
- `TsClasses<'_>`
- `TsComponents<'_>`

`References<'_>` implies symbol identity internally, so rules that request only
references still cause polint to derive the `symbols` capability needed to bind
resolved `ReferenceFact::target` values.

Reserved future capabilities such as `cfg`, `call_graph`, `dataflow`,
`coverage_facts`, and `test_suite_metrics` must stay unsupported until a rule
can consume real public SDK facts for them. Rules that request unsupported or
setup-missing hard capabilities produce `polint/capability` diagnostics during
`polint check` and are not executed with placeholder facts.
