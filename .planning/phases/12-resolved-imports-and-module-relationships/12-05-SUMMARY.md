---
phase: 12-resolved-imports-and-module-relationships
plan: "05"
subsystem: sdk-documentation
tags: [rust, sdk, resolved-imports, module-graph, cli-tests, docs]

requires:
  - phase: 12-01
    provides: public resolved import and module graph fact model, SDK views, and macro capability derivation
  - phase: 12-03
    provides: TS/JS resolution through oxc_resolver and deterministic relationship output
  - phase: 12-04
    provides: Go metadata-backed import resolution and setup-missing capability blocking
provides:
  - External temp-repo proof that rules consume ResolvedImports and ModuleGraphFacts through polint::sdk::prelude::*
  - CLI proof for unresolved import reasons, setup-missing capability diagnostics, and deterministic relationship output
  - Public fact documentation for resolved imports and module graph behavior, fields, query methods, and limits
  - Generated skill guidance for architecture rules using relationship fact views
  - File-origin TS local import edges for architecture boundary rules while preserving module-level external dependency edges
affects: [13-symbols-and-references, architecture-rules, sdk-facts, generated-skills]

tech-stack:
  added: []
  patterns:
    - temp-repo rule hosts prove new SDK views through public prelude imports only
    - TS local resolved imports use file-origin graph edges; external dependencies remain module-level
    - public fact docs describe setup-sensitive uncertainty as data rather than hiding it

key-files:
  created:
    - docs/facts/resolved-imports.md
    - .planning/phases/12-resolved-imports-and-module-relationships/12-05-SUMMARY.md
  modified:
    - crates/polint/tests/cli.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/module_graph/go.rs
    - crates/polint/src/cli/skill.rs
    - docs/facts/README.md
    - docs/facts/capability-plans.md
    - docs/facts/imports.md

key-decisions:
  - "TS/JS local import graph edges originate from the importing file node so architecture rules can detect file-level boundaries."
  - "TS/JS external package imports remain module-level DependsOn edges so project dependency relationships stay compact."
  - "Resolved import docs treat SetupMissing, Dynamic, Unsupported, and Unresolved as public data, not hidden failures."
  - "Test-only Go graph helper methods are cfg(test) rather than suppressed with lint allowances."

patterns-established:
  - "External-consumer CLI fixtures for relationship facts use only polint::sdk::prelude::* and polint::runner::run_cli."
  - "Relationship determinism tests compare parsed diagnostics arrays rather than stdout substrings."
  - "Generated skill text stays aligned with supported typed fact views and avoids manual Capabilities::new() guidance."

requirements-completed: [MOD-01, MOD-02, MOD-03, MOD-04]

duration: 30 min
completed: 2026-05-11
---

# Phase 12 Plan 05: External Relationship Fact Proof Summary

**External SDK proof, public docs, and generated skill guidance for resolved imports and module graph facts**

## Performance

- **Duration:** 30 min
- **Started:** 2026-05-11T16:20:47Z
- **Completed:** 2026-05-11T16:50:48Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added a temp-repo CLI proof where generated local rules import only `polint::sdk::prelude::*`, request `ResolvedImports<'_>` and `ModuleGraphFacts<'_>`, and report rule-owned JSON diagnostics from real relationship facts.
- Proved setup-missing Go relationship capabilities emit `polint/capability` evidence with `status=setup_missing` and block the requesting local rule.
- Proved repeated module relationship checks produce identical parsed diagnostics arrays.
- Added public fact docs for `ResolvedImports<'_>`, `ModuleGraphFacts<'_>`, relationship fact fields, query methods, setup-sensitive behavior, and limits.
- Updated generated polint skill guidance so architecture rules use typed relationship views through the SDK prelude.

## Task Commits

1. **Task 1 RED: external module relationship SDK proof** - `617b145` (test)
2. **Task 1 GREEN: file-origin TS local graph edges** - `3372928` (fix)
3. **Task 2: setup-missing and determinism CLI proof** - `0985a03` (test)
4. **Task 3: public fact docs and skill guidance** - `cf37b73` (docs)
5. **Final verification fix: test-only Go graph helpers** - `c4da2db` (fix)

## Files Created/Modified

- `crates/polint/tests/cli.rs` - Added external-consumer relationship fact tests, setup-missing/determinism proof, and generated-skill assertions.
- `crates/polint/src/module_graph/mod.rs` - Preserves module-level external TS dependencies while emitting local TS import edges from the importing file node.
- `crates/polint/src/module_graph/go.rs` - Gates test-only helper methods with `#[cfg(test)]` for warning-clean clippy.
- `crates/polint/src/cli/skill.rs` - Adds generated skill guidance for `ResolvedImports<'_>` and `ModuleGraphFacts<'_>`.
- `docs/facts/resolved-imports.md` - New public fact contract for resolved imports and module graph facts.
- `docs/facts/README.md` - Links the new resolved imports and module graph fact reference.
- `docs/facts/capability-plans.md` - Lists relationship fact views as supported fact views.
- `docs/facts/imports.md` - Keeps syntactic import docs truthful by pointing resolution users to the relationship views.

## Decisions Made

- Local TS/JS import edges should be file-level so architecture rules can flag boundaries such as `src/ui` importing `src/domain`.
- External TS/JS package dependencies should stay module-level so project dependency relationships remain stable and compact.
- The public docs should describe uncertainty states directly and explicitly avoid claims of TypeScript type checking, Go type checking, symbols, call graph, CFG, coverage, or project-level graph caching.
- Test-only helpers should be compiled only for tests instead of using lint suppression.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed TS local import edge ownership**
- **Found during:** Task 1 (External temp-repo SDK proof for module relationships)
- **Issue:** Resolved TS local imports were linked from the project module node, so an external architecture rule could not observe a `src/ui` file importing a `src/domain` file.
- **Fix:** Chose the importing file/package owner for TS non-external resolved imports while preserving module ownership for external package dependencies.
- **Files modified:** `crates/polint/src/module_graph/mod.rs`
- **Verification:** `cargo test -p polint --test cli external_rule_consumes_module_relationship_facts_through_public_sdk --locked`
- **Committed in:** `3372928`

**2. [Rule 2 - Missing Critical] Updated adjacent fact docs for truthfulness**
- **Found during:** Task 3 (Document public fact contract and generated skill guidance)
- **Issue:** `docs/facts/imports.md` still said polint did not resolve TS/JS or Go imports, and `capability-plans.md` omitted the now-supported relationship views.
- **Fix:** Pointed syntactic import docs to the new resolved-imports reference and added `ResolvedImports<'_>` / `ModuleGraphFacts<'_>` to the supported fact view list.
- **Files modified:** `docs/facts/imports.md`, `docs/facts/capability-plans.md`
- **Verification:** Task 3 `rg` checks and `cargo test -p polint --test cli add_skill_installs_claude_skill_non_interactively --locked`
- **Committed in:** `cf37b73`

**3. [Rule 3 - Blocking] Gated test-only Go graph helpers**
- **Found during:** Full phase clippy verification
- **Issue:** `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` failed on helper methods used only by tests.
- **Fix:** Added `#[cfg(test)]` to the two helper methods instead of weakening lints.
- **Files modified:** `crates/polint/src/module_graph/go.rs`
- **Verification:** `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- **Committed in:** `c4da2db`

---

**Total deviations:** 3 auto-fixed (1 bug, 1 missing critical documentation alignment, 1 blocking verification issue)
**Impact on plan:** All fixes were required for correctness, public truthfulness, or verification. No new public API surface was added beyond the planned relationship fact documentation and generated skill guidance.

## Issues Encountered

- Task 2's TDD test passed immediately because prior Phase 12 work and the Task 1 edge-origin fix already satisfied setup-missing and determinism behavior. It was committed as coverage-only proof.
- Full workspace clippy promoted existing dead-code warnings in test-only Go helpers to errors; the helpers are now test-gated.

## Verification

- `cargo test -p polint --test cli external_rule_consumes_module_relationship_facts_through_public_sdk --locked`
- `cargo test -p polint --test cli module_relationship_setup_missing_and_determinism --locked`
- `cargo test -p polint --test cli add_skill_installs_claude_skill_non_interactively --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- Structural `rg` checks for relationship test helpers/rule IDs, public prelude imports, absence of internal API names in `cli.rs`, docs coverage for statuses/limits/setup behavior, fact README link, generated skill guidance, and absence of `Capabilities::new()` in generated skill text.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. Stub-pattern scan hits only existing test fixture literals (`TODO`, empty fixture arrays, and `export const tokens = {};`) plus docs that say rules are not executed with placeholder facts. No production placeholder data or unwired UI/data source was introduced.

## Next Phase Readiness

MOD-01 through MOD-04 now have external-consumer proof, deterministic CLI behavior, setup-missing diagnostics, and public documentation. Phase 13 can build symbol/reference facts on top of relationship views without needing a debug CLI command or internal SDK escape hatch.

## Self-Check: PASSED

- Found `.planning/phases/12-resolved-imports-and-module-relationships/12-05-SUMMARY.md`.
- Found key created/modified files for CLI proof, provider behavior, generated skill text, and fact documentation.
- Found task and verification fix commits `617b145`, `3372928`, `0985a03`, `cf37b73`, and `c4da2db`.

---
*Phase: 12-resolved-imports-and-module-relationships*
*Completed: 2026-05-11*
