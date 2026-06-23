# Rule Fact Reference

These pages describe the fact families exposed through typed views exported by
`polint::sdk::prelude::*`, plus the `RuleCtx` helpers rules use to report
diagnostics.

Rule authors should treat these facts as the supported building blocks for
repo-local policies. The examples are only consumers of these facts, not special
internal entry points.

Phase 41 keeps promotion explicit. Stable public rule-author views are the pages
listed below. Reserved or internal analysis families such as CFG, call graph,
data flow, evidence, effects/summaries, type/value/alias facts, benchmarks, and
eval reports are documented only as limits or future gates until they have
public SDK views, docs, temp-repo tests, bounded query behavior, setup behavior,
cache/input proof, and no-leak proof. See
[`docs/API-VISIBILITY-PLAN.md`](../API-VISIBILITY-PLAN.md#phase-41-promotion-audit)
for the disposition table.

- [Functions](functions.md)
- [Metrics](metrics.md)
- [Imports](imports.md)
- [Resolved imports and module graph](resolved-imports.md)
- [Symbols and references](symbols-and-references.md)
- [Branches](branches.md)
- [Go tests](go-tests.md)
- [Capability support](capability-plans.md)
- [TypeScript / JavaScript](ts-js.md)
- [Literals and JSX attributes](literals.md)
- [Changed files (review rules)](changed-files.md)

Reserved/internal references:

- [Reserved capability plans](capability-plans.md)
- [Data-flow limits and future gates](data-flow.md)
- [Diagnostic evidence limits](evidence.md)
