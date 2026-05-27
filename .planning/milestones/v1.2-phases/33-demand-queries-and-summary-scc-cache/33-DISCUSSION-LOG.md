# Phase 33: Demand Queries and Summary SCC Cache - Discussion Log

**Date:** 2026-05-22
**Mode:** --auto (fully autonomous)
**Phase:** 33 - Demand Queries and Summary SCC Cache
**Refresh:** Re-run on existing Phase 33 context/plans; decisions retained.

## Discussion Summary

Auto mode selected all 4 gray areas and resolved each with recommended defaults from research documents and established patterns. On refresh, the existing context already matched those selections, so no substantive decision changes were made.

## Areas Discussed

### 1. Demand Query Scope and Activation

**Options presented:**
1. Make all providers demand-driven (big rewrite)
2. Keep existing providers eager; add demand infrastructure for summary SCC closure and future expensive views (recommended)
3. Only add layer cache activation, defer demand queries entirely

**Selected:** Option 2 — Keep existing providers eager; add demand infrastructure for summary SCC closure and future expensive views (recommended default)

**Rationale:** The research explicitly recommends keeping cheap providers eager (syntax, imports, symbols) and making only expensive views demand-driven. Rewriting the existing eager pipeline would be scope creep. Summary SCC closure is the first concrete demand consumer.

### 2. SCC Discovery and Scheduling Strategy

**Options presented:**
1. Build SCC discovery from direct call target graph, reverse-topo order, per-SCC caching with backdating (recommended)
2. Simple per-function iteration without SCC awareness
3. Full whole-program fixpoint computation

**Selected:** Option 1 — SCC discovery with reverse-topo scheduling and backdating (recommended default)

**Rationale:** Per-function iteration misses mutual recursion. Whole-program fixpoint is too coarse. SCC-based scheduling is the standard approach and petgraph already provides SCC computation.

### 3. Extension-Aware Cache Quarantine Semantics

**Options presented:**
1. Cache-level quarantine where extension entries are isolated on digest change; native facts preserved (recommended)
2. Hard invalidation — delete extension cache entries on digest change
3. No quarantine — treat extension changes like any other input change

**Selected:** Option 1 — Cache-level quarantine with native fact preservation (recommended default)

**Rationale:** The research and existing invalidation vocabulary already define quarantine as an explicit action. Native facts should never be lost due to extension changes. Hard deletion loses the ability to reinstate entries if the extension reverts.

### 4. Query Trace and Debug Output Shape

**Options presented:**
1. Follow established crate-private debug JSON pattern (recommended)
2. Add new structured trace format
3. No trace output in Phase 33

**Selected:** Option 1 — Crate-private debug JSON following established pattern (recommended default)

**Rationale:** Every prior phase (28-32) has used the same debug JSON pattern. Consistency reduces implementation risk and reviewer cognitive load.

## Deferred Ideas

None — discussion stayed within phase scope.

## Claude's Discretion Items

- Module layout (new `analysis::demand` vs extending `analysis_kernel::incremental`)
- SCC discovery placement (dedicated pass vs integrated into summary provider)
- Whether to implement interprocedural summary improvement or only scheduling/caching infrastructure
- Exact plan split for independently reviewable PRs

---

*Generated: 2026-05-22 (auto mode)*
