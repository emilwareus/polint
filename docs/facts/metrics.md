# Metric Facts

Metric facts are reusable derived signals. They let several rules read the same
file-size, function-size, and complexity evidence without rules calling other
rules.

Rules request metric signals through typed fact views:

- `FileMetrics<'_>` maps to the `file_metrics` capability.
- `FunctionMetrics<'_>` maps to the `function_metrics` capability.
- `ComplexityMetrics<'_>` maps to the `complexity_metrics` capability.

polint derives these facts after the Go and TS/JS adapters have populated syntax
facts and before rules run.

Query helpers are intentionally small and bounded over stored facts:

| View | Helpers |
|------|---------|
| `FileMetrics<'_>` | `iter()`, `get(file)`, `for_language(language)`, `over_line_count(max)`, `over_byte_count(max)`, `over_function_count(max)` |
| `FunctionMetrics<'_>` | `iter()`, `for_file(file)`, `get(function)`, `over_line_count(max)`, `over_byte_count(max)` |
| `ComplexityMetrics<'_>` | `iter()`, `for_file(file)`, `get(function)`, `over(max)`, `over_complexity(max)` |

## FileMetricFact

| Field | Meaning |
|-------|---------|
| `file` | Stable `FileId` for the source file. |
| `language` | Detected source language. |
| `line_count` | Logical source lines from the loaded source text. |
| `non_empty_line_count` | Source lines whose trimmed text is not empty. |
| `byte_count` | Source text byte length. |
| `function_count` | Number of function facts in the file. |

## FunctionMetricFact

| Field | Meaning |
|-------|---------|
| `function` | Stable `FunctionId` for this analysis run. |
| `file` | Stable `FileId` for the source file. |
| `name` | Function name from the syntax fact. |
| `span` | Function syntax span. |
| `language` | Source language family. |
| `line_count` | Lines covered by the function span. |
| `byte_count` | Bytes covered by the function span. |

## ComplexityMetricFact

| Field | Meaning |
|-------|---------|
| `function` | Stable `FunctionId` for this analysis run. |
| `file` | Stable `FileId` for the source file. |
| `name` | Function name from the syntax fact. |
| `span` | Function syntax span. |
| `language` | Source language family. |
| `cyclomatic_complexity` | Syntax-level heuristic complexity from the language adapter. |

## Composition Pattern

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/code-quality-score",
    description = "Compose size and complexity signals into one heuristic.",
    severity = "warn"
)]
fn code_quality_score(
    ctx: &mut RuleCtx<'_>,
    functions: FunctionMetrics<'_>,
    complexity: ComplexityMetrics<'_>,
) -> RuleResult {
    for function in functions.iter() {
        let Some(complexity_metric) = complexity.get(function.function) else {
            continue;
        };
        if function.line_count > 40 && complexity_metric.cyclomatic_complexity > 8 {
            ctx.warn(&function.span, "Function is both long and branch-heavy.");
        }
    }
    Ok(())
}
```

This is the preferred shape for higher-level policies: reusable signals are
fact views, and policy rules compose them into diagnostics.

## Limits

- Metrics are derived from currently available syntax facts; they are not
  semantic analysis.
- Complexity is heuristic and adapter-specific.
- Line counts come from source text and spans; generated or invalid parse
  regions may reduce precision.
- Metric facts are shared within a run. Provider-specific persistent caching is
  a future optimization and does not change the rule API.
