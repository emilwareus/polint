# Semantic Index Evaluation Plan

## Goal

Measure semantic-index quality as facts, not only as downstream diagnostics.

## Suite Types

| Suite Type | Purpose |
|---|---|
| Native microfixtures | Exact expected scopes, symbols, imports, references, aliases, generated symbols. |
| External oracle comparisons | Compare against gopls, TypeScript language service, Pyright/Ty/Pyrefly, JDT. |
| Real repo smoke tests | Measure unresolved rate, runtime, memory, cache hits, and extension delta. |
| Extension fixtures | Verify generated symbols, alias hints, and resolution overrides. |
| Export fixtures | Verify SCIP/Kythe projection shape without making export internal. |

## Metrics

```text
symbol_precision
symbol_recall
reference_precision
reference_recall
import_resolution_accuracy
unknown_rate
ambiguous_rate
external_rate
extension_delta_precision
extension_delta_unknown_reduction
runtime_ms
peak_rss_mb
cache_hit_rate
stable_key_churn
```

## Oracle Strategy

Use external tools as comparison sources:

- Go: gopls or `go/types` output.
- TS/JS: TypeScript compiler/language service.
- Python: Pyright, Ty, and Pyrefly where applicable.
- Java: JDT for source bindings.

Store mismatches as:

```json
{
  "case": "python-dynamic-attribute",
  "polint": "Dynamic",
  "oracle": "Unresolved",
  "decision": "acceptable-polint-more-explicit",
  "reviewed_by": "human-or-agent",
  "date": "2026-05-15"
}
```

The goal is not blind oracle parity. The goal is reviewed semantic evidence.

## Extension Delta

For agent-authored extensions, report before/after:

```text
default_unknown_refs: 120
extended_unknown_refs: 15
new_exact_refs: 91
new_ambiguous_refs: 14
native_exact_conflicts: 0
extension_validation_failures: 0
runtime_delta_ms: +42
```

Extensions should improve precision/recall or reduce unknowns without hiding rejected conflicts.

## Stable Key Tests

For each language:

- whitespace-only edit: keys unchanged;
- body-only edit: public symbols unchanged where signatures unchanged;
- rename: changed symbol key and reference targets reflect rename;
- move file: decide expected key churn by language and document it;
- generated symbol provider edit: extension digest invalidates dependent resolution facts.

## CI Gates

Before adding public semantic claims:

- microfixtures pass;
- stable key churn below expected thresholds;
- no unproven exact facts in debug audit;
- extension conflicts are visible;
- deterministic output across parallelism modes;
- cache hits/misses match expected invalidation cases.
