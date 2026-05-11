---
phase: 12-resolved-imports-and-module-relationships
fixed_at: 2026-05-11T17:13:01Z
review_path: .planning/phases/12-resolved-imports-and-module-relationships/12-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 12: Code Review Fix Report

**Fixed at:** 2026-05-11T17:13:01Z
**Source review:** .planning/phases/12-resolved-imports-and-module-relationships/12-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### WR-01: Dotless Go module paths can turn missing local imports into external dependencies

**Files modified:** `crates/polint/src/module_graph/go.rs`
**Commit:** 4023077
**Status:** fixed: requires human verification
**Applied fix:** Changed Go external dependency classification to check the active module path before stdlib-style dotless path detection, and added a regression test for `module mycorp/app` with missing local import `mycorp/app/internal/missing` resolving to `Unresolved(NotFound)`.

### WR-02: Fallback package nodes merge unrelated same-name packages

**Files modified:** `crates/polint/src/module_graph/model.rs`, `crates/polint/src/module_graph/mod.rs`
**Commit:** da4eda3
**Status:** fixed: requires human verification
**Applied fix:** Scoped fallback package-node labels by the package file directory, updated existing fallback expectations, and added a regression test proving same-name Go packages in `cmd/api` and `cmd/worker` create distinct package nodes while metadata-backed Go labels still use import paths.

### WR-03: Docs say setup-missing relationship facts are inspectable by rules that are actually blocked

**Files modified:** `docs/facts/resolved-imports.md`, `crates/polint/src/cli/skill.rs`
**Commit:** aeaf529
**Status:** fixed
**Applied fix:** Updated the public fact docs and generated skill text to describe `Unresolved`, `Dynamic`, and `Unsupported` as inspectable statuses for running relationship rules, while `SetupMissing` is surfaced as a `polint/capability` diagnostic that blocks requesting rules.

---

_Fixed: 2026-05-11T17:13:01Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
