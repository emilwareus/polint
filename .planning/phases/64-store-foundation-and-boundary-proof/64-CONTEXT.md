# Phase 64: Store Foundation and Boundary Proof - Context

**Gathered:** 2026-07-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 64 establishes the private SQLite/rusqlite ownership boundary under `analysis_kernel`, including safe cache-root placement, connection policy, numbered migrations, bounded writer contention, controlled open/recovery outcomes, and parity/no-leak/performance proofs. It does not persist kernel manifests, generations, facts, summaries, graph indexes, or query results; those remain in Phases 65–68.

</domain>

<decisions>
## Implementation Decisions

### Activation and Store Ownership
- **D-01:** Keep the Phase 64 store integration crate-private and disabled by default on the normal CLI path while it has no user-visible value. Internal tests and harnesses explicitly enable it. Do not add a public CLI flag or `.polint.toml` contract in this phase.
- **D-02:** Treat `--no-cache` as a hard store disable once normal store activation is introduced. The disabled path must short-circuit before path creation, schema checks, connection opens, or other store I/O.
- **D-03:** Place the database under the existing `CacheLayout` / `POLINT_CACHE_DIR` root in a dedicated semantic-store directory. Reuse existing repo-path safety and gitignored cache ownership rather than introduce another storage root.

### Schema and Recovery Posture
- **D-04:** Use a small, explicit numbered migration runner driven by `PRAGMA user_version`. Empty and supported prior schemas migrate transactionally; reopening the current schema is idempotent.
- **D-05:** Never mutate or downgrade a future schema. Return a typed internal refusal/skipped outcome that preserves the database for the newer polint version.
- **D-06:** Invalid or corrupt stores return typed internal rebuild-needed/skipped outcomes. Policy analysis remains usable and byte-identical to the disabled-store path. Any destructive replacement must be explicit and restricted to a verified polint-owned cache path.

### Writer Contention and Connection Discipline
- **D-07:** Use one read-write connection and `BEGIN IMMEDIATE` as the write/generation lease. Apply foreign keys, WAL, and a short bounded busy timeout through the private connection layer.
- **D-08:** A losing concurrent `check` invocation skips persistence with a typed internal busy outcome instead of blocking indefinitely or changing policy answers. Read-only access uses separately opened read-only connections.
- **D-09:** Do not expose `rusqlite::Connection`, statements, rows, transactions, SQL strings, or raw database identifiers across the store module boundary.

### Kernel Integration and Failure Semantics
- **D-10:** Integrate only through a typed crate-private store facade after the existing kernel has computed and validated its output. Providers, language adapters, rules, `RuleCtx`, SDK, runner APIs, and public CLI renderers receive no SQL handle or SQL vocabulary.
- **D-11:** Store open, migration, contention, corruption, and skip status belongs in private run telemetry. Phase 64 must not add store-derived public `polint check` diagnostics or alter exit semantics.
- **D-12:** Because this phase reads no persisted facts, enabled, disabled, busy, future-schema, invalid-schema, and corrupt-store paths must all produce the same normalized policy answers as the store-disabled run. Store failure cannot create partial confident answers.

### Verification and Scope Boundary
- **D-13:** Require migration fixtures for empty, supported previous, idempotent current, future, invalid, and corrupt schemas; verify connection pragmas, read-only separation, and bounded contention with independent connections/processes.
- **D-14:** Prove the disabled path performs no filesystem touch, and prove byte-identical `check` diagnostics and exit behavior across disabled, enabled, and store-failure modes.
- **D-15:** Extend public-surface leak gates to reject store module paths, rusqlite/SQL vocabulary, table names, and raw database IDs from SDK exports, the external probe, public CLI JSON, docs, examples, and generated skill text.
- **D-16:** Run the Phase 63 regression-budget gate at the Phase 64 boundary. Peak RSS above +20%, cold wall-clock above +25% (subject to the locked noise floors), or diagnostics-digest drift blocks completion.
- **D-17:** Keep Phase 64 schema deliberately minimal: connection/migration scaffolding and only private bookkeeping necessary to prove it. Store manifests, active/pending/complete generations, persisted facts, summaries, graph indexes, query services, cache-status UX, and public activation controls remain in later mapped phases.

### the agent's Discretion
- Exact private Rust type names and the split among `store/mod.rs`, connection, migration, error/status, and test modules.
- The precise bounded busy-timeout value and retry count, provided tests prove it is bounded and contention falls back without changing policy output.
- The minimal initial SQL schema and migration representation, provided it does not pull Phase 65 generation/manifest semantics forward.
- The internal telemetry shape and whether safe rebuild is represented as a separate explicit helper or a typed outcome consumed by the caller.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope and Product Contracts
- `.planning/PROJECT.md` — v2.0 product goal, target features, strict public-boundary discipline, and milestone outcome gates.
- `.planning/REQUIREMENTS.md` — authoritative STORE-01/02/03/06/07/08, PERF-03, PROD-01, and VAL-02 requirements mapped to Phase 64.
- `.planning/ROADMAP.md` — Phase 64 goal, dependencies, success criteria, and the Phase 65+ sequencing boundary.
- `docs/API-VISIBILITY-PLAN.md` — supported public surface and visibility discipline that the store must not widen.

### Locked v2.0 Architecture
- `.planning/research/SUMMARY.md` — locked SQLite/rusqlite direction, kernel/source-of-truth relationship, risks, and build order.
- `.planning/research/STACK.md` — dependency version/features, rejected alternatives, and connection/migration constraints.
- `.planning/research/ARCHITECTURE.md` — private store placement, after-validation data flow, validation hooks, and failure boundaries.

### Local Semantic Store Contract
- `research/local-semantic-store/decisions/DECISIONS.md` — SQLite primary-store decision, registry deferral, graph backend, and reuse of existing kernel identities.
- `research/local-semantic-store/implementation/STORE-CONTRACT.md` — non-public contract, rebuildability, commit protocol, and privacy rules.
- `research/local-semantic-store/RECOMMENDED_IMPLEMENTATION.md` — cache-root lifecycle, pragmas, migration approach, facade boundary, and phased sequencing.

### Phase 63 Gates Carried Forward
- `.planning/phases/63-ground-truth-and-performance-baseline/63-04-SUMMARY.md` — fail-not-silent regression-budget gate Phase 64 must invoke.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/cache/mod.rs`: `CacheLayout`, `POLINT_CACHE_DIR`, disabled-cache short-circuiting, managed-path checks, and atomic/no-symlink filesystem helpers provide the store location and safety model.
- `crates/polint/src/analysis_kernel/incremental/mod.rs`: existing crate-private `InputSnapshot`, provider metadata, typed digest/key, dependency, status, and run-report vocabulary must remain the store's identity source rather than be duplicated.
- `crates/polint/tests/public_surface_leak.rs`: the allowlisted prelude parser and external consumer probe are the established boundary gate to extend for store/SQL leakage.
- `crates/polint/src/eval/bench/gate.rs`: `evaluate_regression_budget` and `is_blocking` already encode the Phase 64+ RSS, cold-latency, and diagnostics-parity gate.

### Established Patterns
- Internal analysis infrastructure uses private modules, curated `pub(crate)` re-exports, typed status enums, deterministic ordering, and controlled fallbacks rather than leaking implementation types.
- Disabled caches return before filesystem work and unsafe/corrupt cache artifacts are treated as misses/evicted state rather than trusted inputs.
- `AnalysisKernel::run` computes provider outputs, validates fact metadata, and returns a private `KernelRunReport`; store telemetry can extend this private seam without changing SDK or CLI contracts.
- Cache paths are repository-managed and symlink-safe when using the default `.polint/cache` root, while `POLINT_CACHE_DIR` remains the existing override boundary.

### Integration Points
- Add `analysis_kernel::store` as a private sibling of `incremental`, `metadata`, and `provider` in `crates/polint/src/analysis_kernel/mod.rs`.
- Add a semantic-store path accessor through `CacheLayout`/`Cache`, preserving the current `enabled` gate before any path or connection work.
- Hook the no-op foundation after kernel validation; do not pass the store into provider constructors or rule execution.
- Extend `crates/polint/tests/public_surface_leak.rs` and the external fixture under `tests/fixtures/public-surface-leak-probe/`.
- Reuse the committed Phase 63 baseline and regression gate for phase-boundary verification.

</code_context>

<specifics>
## Specific Ideas

- The foundation should be invisible to users except through unchanged behavior and improved internal readiness; no public “experimental store” surface is warranted before persisted data creates value.
- Failure and contention are store outcomes, not policy findings. The clearest correctness proof is identical normalized `check` output across every store mode.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. Later capabilities remain assigned to Phases 65–71 rather than being pulled into Phase 64.

</deferred>

---

*Phase: 64-store-foundation-and-boundary-proof*
*Context gathered: 2026-07-10*
