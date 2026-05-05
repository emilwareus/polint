# Phase 7: Cache and Performance - Context

**Gathered:** 2026-05-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 7 makes the existing cache/performance surface real and safe: cache parse/fact metadata under `.polint/cache`, make `--no-cache` fully disable cache reads and writes, run parsing and rule execution in parallel where deterministic output can be preserved, and harden `polint profile-rules` timing output.

This phase does not add production CI/SARIF hardening, graph command expansion, dynamic repo-local rule compilation, external cache services, exact semantic type checking, or speedup claims. Those remain later phases or out of scope.

</domain>

<decisions>
## Implementation Decisions

### Cache key and payload boundary
- **D-01:** Use conservative content-addressed cache keys that include file content hash, config hash, rule hash, cache schema/version, and language/parser/fact schema inputs where relevant.
- **D-02:** Cache parse/fact metadata per file/language where safe. Do not cache full ASTs, full source text, secrets, process-local IDs, or data that cannot be validated after a read.
- **D-03:** Keep cache values JSON/serde-based under `.polint/cache` for v1 so they remain inspectable and easy to invalidate. A stale hit is not acceptable; a miss or safe fallback is acceptable.
- **D-04:** Treat the existing `polint-cache::CacheKey`, `Cache`, and `CACHE_VERSION` as the baseline, but extend the contract where needed so keys prove all success-criteria inputs.

### Disabled cache semantics
- **D-05:** `--no-cache` must bypass both reads and writes. It is not enough to instantiate a disabled cache and then leave the rest of the pipeline unaware.
- **D-06:** Cache read/write failures should become controlled fallbacks or internal diagnostics where practical, not crashes that prevent analysis when the source can still be parsed normally.

### Deterministic parallelism
- **D-07:** Use Rayon only at boundaries with deterministic collection/reduction. Final file order, fact insertion order, diagnostic order, and fingerprints must match repeated runs.
- **D-08:** Per-file parsing/fact extraction may run in parallel, but insertion into `AnalysisDb` and any exposed IDs must remain deterministic by root-relative file order.
- **D-09:** Preserve the Phase 3 rule-runner contract that sequential and parallel rule execution produce equivalent deduped diagnostics.
- **D-10:** Keep the Phase 5 source-storage constraint: avoid cloning large source strings while adding cache or parallel parsing paths.

### Per-rule profiling output
- **D-11:** `polint profile-rules` should report per-rule timings without changing diagnostic semantics.
- **D-12:** The rule ordering and row structure should be deterministic. Duration values are inherently variable, so tests should assert rule IDs, row shape, nonnegative/parseable timings, and exit-code behavior rather than exact time.
- **D-13:** JSON output for profiling can be deferred unless it is the smallest clean way to test the contract; Phase 8 owns broader CLI output hardening.

### Validation and performance proof
- **D-14:** Add focused unit tests for cache key inputs, disabled cache behavior, cache read/write round trips, and stale/invalid cache fallback.
- **D-15:** Add integration tests proving `--no-cache` avoids `.polint/cache` writes and repeated runs remain deterministic with cache and parallelism enabled.
- **D-16:** Add property tests only where they give leverage for cache keys or deterministic ordering. Do not create flaky microbenchmarks or claim fixed speedups.

### Auto-selected defaults
- **D-17:** `[auto]` Cache key and payload boundary -> selected content-addressed parse/fact metadata only.
- **D-18:** `[auto]` Disabled cache semantics -> selected bypass both reads and writes.
- **D-19:** `[auto]` Deterministic parallelism -> selected safe parallelism with deterministic reduction.
- **D-20:** `[auto]` Per-rule profiling output -> selected deterministic rows with variable durations.
- **D-21:** `[auto]` Validation and performance proof -> selected cache on/off, deterministic repeated-run, and timing-structure tests.

### the agent's Discretion
- The agent may choose the exact cache file layout, serde structs, atomic write strategy, parser/fact metadata shape, and cache invalidation helper names.
- The agent may choose whether `profile-rules` remains a subcommand with human output only or gets a narrow testable machine-readable option, as long as Phase 7 criteria are met without pulling in Phase 8.
- The agent may split planning by cache foundation, parse/fact cache integration, deterministic parallelism, profiling, and tests.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` - Phase 7 goal, requirement IDs, and success criteria.
- `.planning/REQUIREMENTS.md` - `PERF-01`, `PERF-02`, `PERF-03`, `TEST-01`, and `TEST-04`.
- `.planning/PROJECT.md` - Product value, performance/reliability/truthfulness constraints, active requirements, and no-worktree repository layout.
- `.planning/STATE.md` - Current main-branch execution policy and Phase 7 focus.

### Prior decisions to carry forward
- `.planning/phases/02-cli-config-and-discovery/02-CONTEXT.md` - `--no-cache`, CLI/config/discovery contracts, and integration-test style.
- `.planning/phases/03-core-facts-and-diagnostics/03-CONTEXT.md` - Deterministic file IDs, `AnalysisDb`, rule runner, diagnostics, stable fingerprints, and property-test decisions.
- `.planning/phases/04-go-adapter/04-CONTEXT.md` - Go parser diagnostics, syntax-first extraction, branch fingerprint determinism, and heuristic honesty.
- `.planning/phases/05-typescript-adapter/05-CONTEXT.md` - Oxc parsing, borrowed source handling, syntax-first extraction, and TS/JS determinism.
- `.planning/phases/06-sdk-and-example-rules/06-CONTEXT.md` - SDK/rule helper boundaries, deterministic example-rule diagnostics, and no dynamic rule-loading claims.

### Source surfaces to inspect
- `crates/polint-cache/src/lib.rs` - Current cache key, version, JSON read/write, and disabled-cache skeleton.
- `crates/polint-cli/src/main.rs` - `--no-cache`, `check`, `analyze_and_run`, `profile_rules`, graph command reuse, and current unused cache integration point.
- `crates/polint-core/src/lib.rs` - `SourceFile.content_hash`, `AnalysisDb`, `run_rules`, Rayon rule execution, diagnostics sorting/deduping, and deterministic runner tests.
- `crates/polint-fs/src/lib.rs` - Deterministic file discovery and `AnalysisDb` file loading.
- `crates/polint-go/src/lib.rs` - Go parser/fact extraction path that needs safe cache and parallel integration.
- `crates/polint-ts/src/lib.rs` - TS/JS parser/fact extraction path that needs safe cache and parallel integration without large source clones.
- `crates/polint-cli/tests/cli.rs` - CLI integration patterns and existing `--no-cache`, deterministic-output, and profile-related test coverage.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint-cache/src/lib.rs` already provides `CACHE_VERSION`, `CacheKey { file_hash, config_hash, rule_hash, version }`, stable cache IDs, `Cache::default_for_repo(root, enabled)`, and disabled `read_json`/`write_json` no-ops.
- `crates/polint-core/src/lib.rs` already stores `SourceFile.content_hash` and shared source text, exposes deterministic `AnalysisDb` insertion APIs, and has `run_rules(..., parallel)` implemented with Rayon plus a sequential-equivalence test.
- `crates/polint-cli/src/main.rs` already exposes `--no-cache` on `CheckArgs`, constructs a disabled/enabled cache in `analyze_and_run`, and has `profile_rules` timing each enabled built-in rule.
- `crates/polint-fs/src/lib.rs` already sorts discovered root-relative paths and preserves discovery order in file IDs.
- `crates/polint-cli/tests/cli.rs` already uses temp repos, TOML profiles, parsed JSON/SARIF assertions, and repeated-run determinism tests.

### Established Patterns
- Work directly in `/Users/emilwareus/Development/exlint` on `main`; no GSD worktrees.
- Analysis starts with deterministic discovery, then Go analysis, TS analysis, built-in rule execution, dedupe, render, and exit-code selection.
- Parser errors and rule panics are represented as diagnostics or controlled internal errors.
- Public facts and diagnostics derive serde where they cross output/cache-style boundaries.
- Tests favor focused unit coverage plus CLI integration fixtures; property tests are used for deterministic invariants when they are valuable.

### Integration Points
- `polint-cli::analyze_and_run` is the central place to thread cache and parallel settings through discovery, parsing, rule execution, and tests.
- `polint_go::analyze` and `polint_ts::analyze` currently mutate a shared `AnalysisDb`; parallel work must either collect per-file outputs first or otherwise preserve deterministic insertion.
- `polint_core::run_rules` already supports parallel rule execution and should remain the rule-runner boundary.
- `polint_cache::Cache` should become observable through real read/write behavior in `check` while staying completely inert under `--no-cache`.
- `profile_rules` should reuse the same analysis behavior as `check` where possible so timings measure rule work against the same facts.

</code_context>

<specifics>
## Specific Ideas

- Prefer a small complete cache over a broad query/cache rewrite.
- Cache correctness matters more than hit rate in this phase.
- Keep cache payloads source-free and secret-free.
- Keep output determinism observable through tests, not through implementation claims alone.
- Do not claim specific performance gains unless measured outside the normal acceptance tests.

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within phase scope.

</deferred>

---

*Phase: 07-cache-and-performance*
*Context gathered: 2026-05-01*
