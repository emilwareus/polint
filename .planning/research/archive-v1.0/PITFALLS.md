# Pitfalls Research: polint

## Pitfall: Overbuilding Optional Subsystems Before Core

Warning signs:
- Secondary hosts or extra toolchains block CLI, SDK, or built-in rule delivery.
- Rule-facing APIs grow faster than the fact model and diagnostics pipeline.

Prevention:
- Ship native SDK and example rules first.
- Keep optional execution surfaces behind clear interfaces and defer them until the core workflow is proven.

Phase mapping: Phase 6 and Phase 10.

## Pitfall: Building A Generic Ruleset Instead Of A Framework

Warning signs:
- Most effort goes into built-in lint rules.
- README reads like a replacement for ESLint or golangci-lint.

Prevention:
- Keep example rules focused on SDK dogfooding.
- Make docs and CLI copy center custom repo-local policies.

Phase mapping: Phase 5 and Phase 10.

## Pitfall: Nondeterministic Output Under Parallelism

Warning signs:
- Snapshot tests fail intermittently.
- Diagnostics come out in different order by run.

Prevention:
- Assign stable IDs deterministically.
- Sort files, facts, and diagnostics before output.
- Add property/snapshot tests for ordering.

Phase mapping: Phase 2 and Phase 7.

## Pitfall: Heuristic Rules Claim Too Much

Warning signs:
- Branch/test rules say a branch is untested with certainty.
- Diagnostics ignore parser limitations.

Prevention:
- Use wording such as "No nearby test evidence found".
- Include evidence and help text.
- Keep coverage facts in the model for future exact coverage.

Phase mapping: Phase 3 and Phase 5.

## Pitfall: Rule Authors Must Understand Parser Internals

Warning signs:
- Example custom rules manipulate tree-sitter or Oxc AST nodes directly.
- SDK examples are longer than the rule logic.

Prevention:
- Make `RuleCtx` expose high-level facts and query helpers.
- Keep parser details in adapters and core.

Phase mapping: Phase 6.

## Pitfall: Cache Unsafety

Warning signs:
- Diagnostics persist after source/config/rule changes.
- Cache cannot be disabled.

Prevention:
- Cache by source hash, config hash, rule hash, SDK/cache version.
- Add `--no-cache` and tests for cache on/off behavior.

Phase mapping: Phase 7.
