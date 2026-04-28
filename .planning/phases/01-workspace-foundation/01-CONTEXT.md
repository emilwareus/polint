# Phase 1: Workspace Foundation - Context

**Gathered:** 2026-04-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 1 establishes and verifies the Rust workspace foundation for exlint. The scope anchor is a compiling Rust 2024 workspace with the requested crate boundaries, CI-friendly verification commands, dependency versions aligned to the prompt's crate-version research, and enough minimal public API/tests for later phases to build on.

Because the implementation already exists on `main` at commit `7828215`, Phase 1 planning should reconcile and verify the existing foundation rather than recreate it from scratch.

</domain>

<decisions>
## Implementation Decisions

### Completion stance
- **D-01:** Treat the existing `main` implementation as the Phase 1 baseline.
- **D-02:** Planning should focus on verifying the baseline, identifying any missing Phase 1 criteria, and creating closure tasks only for actual foundation gaps.
- **D-03:** Do not create or use worktrees for this project; all work happens in `/Users/emilwareus/Development/exlint` on `main`.

### Crate boundary source of truth
- **D-04:** Keep the current workspace crate set from `Cargo.toml`: `polint-cli`, `polint-core`, `polint-config`, `polint-diagnostics`, `polint-fs`, `polint-cache`, `polint-sdk`, `polint-go`, `polint-ts`, `polint-graph`, `polint-rules`, and `polint-plugin`.
- **D-05:** The crate responsibilities are defined by `docs/INITIAL_PROMPT.md` and reflected in `.planning/PROJECT.md`, `.planning/REQUIREMENTS.md`, and `.planning/ROADMAP.md`.
- **D-06:** Do not collapse crates for short-term simplicity unless verification proves a boundary is actively harmful.

### Verification baseline
- **D-07:** Phase 1 verification must include `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- **D-08:** The dependency baseline should remain tied to the crate versions checked on 2026-04-28 and recorded in `.planning/research/STACK.md`.
- **D-09:** If verification fails, prefer narrow fixes in the affected crate over broad refactors.

### Handoff boundaries
- **D-10:** Cache persistence wiring belongs to Phase 7, not Phase 1.
- **D-11:** Custom rule auto-compilation/loading belongs to later SDK/plugin work, not Phase 1.
- **D-12:** Go and TypeScript parser precision improvements belong to Phases 4 and 5.
- **D-13:** Expanded snapshot/property coverage belongs to later testing/hardening work unless a missing test directly blocks Phase 1 confidence.

### the agent's Discretion
- The agent may choose the smallest verification or documentation fixes needed to make Phase 1 accurately complete.
- The agent may update GSD status/traceability if Phase 1 is already satisfied by existing committed work.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project intent and phase scope
- `docs/INITIAL_PROMPT.md` — Original full project brief and required workspace/crate structure.
- `.planning/PROJECT.md` — Project value, constraints, decisions, and non-goals.
- `.planning/REQUIREMENTS.md` — Phase 1 requirements `FND-01`, `FND-02`, and shared testing requirement `TEST-01`.
- `.planning/ROADMAP.md` — Phase 1 goal and success criteria.
- `.planning/VERIFICATION.md` — Existing verification record for commit `7828215`.

### Existing foundation implementation
- `Cargo.toml` — Current Rust 2024 workspace members and dependency versions.
- `Cargo.lock` — Locked dependency graph for the current implementation.
- `README.md` — Current project quickstart and development commands.
- `crates/` — Existing crate skeletons and initial implementations.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Cargo.toml`: Defines the workspace, Rust edition, crate members, and dependency baseline.
- `crates/polint-core/src/lib.rs`: Defines stable IDs, core facts, `AnalysisDb`, capabilities, rule trait, registry, and runner.
- `crates/polint-diagnostics/src/lib.rs`: Defines diagnostic model, renderers, fingerprints, sorting, and dedupe.
- `crates/polint-cli/src/main.rs`: Provides the current CLI surface for `init`, `new-rule`, `check`, `explain`, `test-rules`, `profile-rules`, and graph commands.
- `crates/polint-cli/tests/cli.rs`: Provides initial integration coverage for init, new-rule, check clean, and TS raw-color failure.

### Established Patterns
- Workspace dependencies are centralized in root `Cargo.toml`.
- Crates expose small initial APIs rather than deep private implementations.
- Diagnostics are deterministic and rule execution uses explicit capabilities.
- Heuristic behavior is documented as heuristic in diagnostics/docs.

### Integration Points
- Later CLI/config/discovery work should build from `polint-cli`, `polint-config`, and `polint-fs`.
- Later fact model work should extend `polint-core` and `polint-diagnostics`.
- Later Go/TS work should refine `polint-go` and `polint-ts` without changing Phase 1 crate boundaries unless necessary.

</code_context>

<specifics>
## Specific Ideas

- The user wants GSD to operate directly inside `/Users/emilwareus/Development/exlint` on `main`; no worktrees.
- Phase 1 should not duplicate the already committed workspace creation.
- Treat the current implementation as real but still subject to verification and gap closure.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-workspace-foundation*
*Context gathered: 2026-04-28*
