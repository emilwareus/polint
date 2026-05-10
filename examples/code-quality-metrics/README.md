# Code Quality Metrics Example

This example shows reusable derived metric signals. The local rule pack contains
three rules:

- `local/large-file` reads `FileMetrics<'_>`.
- `local/large-function` reads `FunctionMetrics<'_>`.
- `local/code-quality-score` composes `FunctionMetrics<'_>` with
  `ComplexityMetrics<'_>`.

Run it from this directory:

```bash
polint check --fail-on none
```

The important part is that the rules do not call each other. Each rule requests
typed signal views in its `#[polint::rule]` signature, and polint derives and
shares those signals before rule execution.
