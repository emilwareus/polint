---
phase: 65-generation-manifest-and-metadata-mirroring
fixed_at: 2026-07-28T22:09:12Z
review_path: .planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 65: Code Review Fix Report

**Fixed at:** 2026-07-28T22:09:12Z
**Source review:** `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### CR-01: Deleting the active pointer erases workspace ownership without invalidating the store

**Status:** fixed: requires human verification
**Files modified:** `crates/polint/src/analysis_kernel/store/migrations.rs`, `crates/polint/src/analysis_kernel/store/tests.rs`
**Commit:** 48c5cce2
**Applied fix:** Current-schema content validation now requires active-selection presence to match complete-generation presence. The regression deletes the active pointer after publishing, then proves active reads, exact matching, maintenance, same-workspace publication, and second-workspace publication all return the typed invalid-schema outcome without changing the complete or pending generations.

### CR-02: Current-schema index preflight allocates an unbounded attacker-controlled vector

**Status:** fixed: requires human verification
**Files modified:** `crates/polint/src/analysis_kernel/store/migrations.rs`, `crates/polint/src/analysis_kernel/store/tests.rs`
**Commit:** 00979854
**Applied fix:** Manifest-source index authentication now checks catalog cardinality with a scalar query, decodes only the sole expected index row, and streams exactly two expected index columns before rejecting any third. A 128-extra-index fixture proves the real writer preflight returns the typed invalid-schema outcome without catalog mutation.

### WR-01: The canonical source codec accepts Windows drive-absolute paths

**Status:** fixed: requires human verification
**Files modified:** `crates/polint/src/analysis_kernel/incremental/run_manifest.rs`
**Commit:** 27506a52
**Applied fix:** Canonical source validation now rejects native root/prefix components plus portable Windows drive, rooted, UNC, and verbatim-prefix spellings before slash/dot normalization. Build and stored-decode regressions cover drive-absolute, drive-relative, UNC, verbatim, and POSIX-absolute paths, and accepted encoded rows are asserted not to retain a workspace prefix.

## Verification

- Run-manifest codec tests: 8 passed.
- Semantic-store tests: 37 passed.
- Store migration tests: 21 passed.
- Public-surface leak tests: 7 passed.
- `make lint`: passed.
- `git diff --check 81baf177..HEAD`: passed.

## Skipped Issues

None.

---

_Fixed: 2026-07-28T22:09:12Z_
_Fixer: the agent (gsd-code-fixer)_
_Iteration: 1_
