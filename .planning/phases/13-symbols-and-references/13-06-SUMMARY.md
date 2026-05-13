---
phase: 13-symbols-and-references
plan: 06
subsystem: sdk
tags: [symbols, references, facts, cli, docs, cache]

requires:
  - phase: 13-04
    provides: TypeScript and JavaScript symbol/reference provider facts
  - phase: 13-05
    provides: Go symbol/reference provider facts and setup diagnostics
provides:
  - Supported symbols and references capability promotion
  - External SDK proof for TS/JS and Go symbol/reference rule packs
  - Public symbol/reference fact documentation and generated skill guidance
affects: [sdk, analysis-plan, symbol-graph, cli-tests, facts-docs]

tech-stack:
  added: []
  patterns:
    - Typed SDK views expose symbol/reference queries while internal graph helpers remain private
    - External rule tests prove public prelude consumption only

key-files:
  created:
    - docs/facts/symbols-and-references.md
  modified:
    - crates/polint/src/analysis_plan.rs
    - crates/polint/tests/cli.rs
    - crates/polint/src/cli/skill.rs
    - crates/polint/src/sdk/facts.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/symbol_graph/query.rs
    - docs/facts/README.md
    - docs/facts/capability-plans.md

key-decisions:
  - "Promote symbols and references to supported capabilities now that TS/JS and Go providers exist."
  - "Keep Symbols<'_> and References<'_> as typed SDK views; rule authors do not declare capabilities manually."
  - "Document precision/status limits as part of the public contract instead of implying whole-program exactness."

patterns-established:
  - "References<'_> implies internal symbol identity derivation for resolved targets."
  - "External CLI tests parse PolintReport JSON and assert evidence, not stdout substrings."

requirements-completed: [SYM-01, SYM-02, SYM-03, SYM-04]

duration: recovered
completed: 2026-05-13
---

# Phase 13-06: Symbol Promotion And Public Contract Summary

**Symbols and references are promoted to supported rule-author facts with external SDK proof, stable cache coverage, setup-missing behavior, and public precision documentation.**

## Performance

- **Duration:** recovered from stalled executor; final verification completed 2026-05-13T07:44:28Z
- **Tasks:** 3
- **Files modified:** 12 in the final task commit, plus earlier task commits

## Accomplishments

- Promoted `symbols` and `references` support in analysis planning while preserving setup-missing capability diagnostics for unsupported local setup.
- Added temp-repo CLI proof that external rule packs consume `Symbols<'_>` and `References<'_>` through `polint::sdk::prelude::*` for TS/JS and Go.
- Added cache/determinism coverage showing stable symbol/reference evidence across repeated cached checks.
- Documented public symbol/reference facts, ID stability, query methods, precision/status variants, TS/JS limits, Go setup behavior, and non-claims for call graph, CFG, dataflow, coverage, TS type checker, and Go SSA.
- Updated generated skill guidance so rule authors request typed fact views and inspect precision/status fields.

## Task Commits

1. **Task 1: Promote symbol capabilities and prove plan/cache semantics**
   - `9fb81fd` test(13-06): add failing tests for symbol capability promotion
   - `4957ae6` feat(13-06): promote symbol capabilities
2. **Task 2: Add external-consumer symbol/reference CLI proof**
   - `c40bfeb` test(13-06): add failing external symbol SDK tests
   - `b3c57b1` test(13-06): prove external symbol SDK coverage
3. **Task 3: Document public facts and generated skill guidance**
   - `0d5b291` test(13-06): require symbol guidance in generated skill
   - `850d472` docs(13-06): document symbol reference contract

## Verification

- `cargo test -p polint --lib analysis_plan_supports_symbol_capabilities --locked`
- `cargo test -p polint --test cli symbol_reference_cache_and_setup --locked`
- `cargo test -p polint --test cli external_rule_consumes_ts_symbols_and_references_through_public_sdk --locked`
- `cargo test -p polint --test cli external_rule_consumes_go_symbols_and_references_through_public_sdk --locked`
- `cargo test -p polint --test cli symbol_reference_macro_mapping_and_determinism --locked`
- `cargo test -p polint --test cli add_skill_installs_claude_skill_non_interactively --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`

## Files Created/Modified

- `docs/facts/symbols-and-references.md` - Public fact contract, examples, precision/status coverage, language limits, and cache/determinism expectations.
- `docs/facts/README.md` - Fact docs index now links the symbol/reference page and describes typed fact views.
- `docs/facts/capability-plans.md` - Supported fact-view list includes `Symbols<'_>` and `References<'_>`.
- `crates/polint/src/cli/skill.rs` - Generated skill text includes symbol/reference guidance.
- `crates/polint/tests/cli.rs` - External SDK, cache, setup-missing, macro mapping, and generated-skill regressions.
- `crates/polint/src/sdk/facts.rs` and `crates/polint/src/symbol_graph/query.rs` - SDK views route through the internal query helpers.

## Decisions Made

- `References<'_>` remains a typed public view but implies `symbols` internally so resolved `ReferenceFact::target` values are backed by symbol identity.
- Public docs expose stable polint IDs, status, precision, spans, names, and stable keys only; they do not expose raw Oxc IDs, Go object values, sidecar DTOs, or internal indexes.
- TS/JS claims are limited to Oxc local lexical facts and module-linked imports; Go claims are limited to typed package information when root Go setup is available.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Final clippy/test cleanup after stalled executor**
- **Found during:** Task 3 final verification
- **Issue:** `clippy -D warnings` exposed unused helper wrappers, needless lifetimes, `len() > 0`, clone-on-copy, type complexity, and too-many-argument warnings. Full workspace tests also found stale expectations from the pre-promotion symbol-provider behavior.
- **Fix:** Routed public SDK symbol/reference methods through internal query helpers, made test-only stable-ID helper constructors test-only, cleaned test lifetimes/collections/copy values, added narrow justified `#[expect]` annotations for semantic identity constructors, and updated stale assertions to expect promoted provider facts.
- **Files modified:** `crates/polint/src/sdk/facts.rs`, `crates/polint/src/symbol_graph/*.rs`, `crates/polint/tests/cli.rs`
- **Verification:** Full fmt, clippy, and workspace test suite passed.
- **Committed in:** `850d472`

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Verification cleanup only. No scope expansion beyond making the promoted symbol/reference behavior pass the planned gates.

## Issues Encountered

- The original executor stopped returning through the agent channel after completing most of Task 3. The orchestrator spot-checked commits and dirty files, closed the stalled agent, finished the remaining task locally, and preserved the executor's completed commits.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 13 now has all plan summaries through 13-06. It is ready for phase-level code review, regression verification, security review routing, and final GSD phase completion.

---
*Phase: 13-symbols-and-references*
*Completed: 2026-05-13*
