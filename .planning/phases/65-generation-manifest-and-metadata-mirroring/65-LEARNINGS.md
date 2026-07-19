---
phase: 65
phase_name: "Generation Manifest and Metadata Mirroring"
project: "polint"
generated: "2026-07-19"
counts:
  decisions: 7
  lessons: 8
  patterns: 6
  surprises: 6
missing_artifacts:
  - "UAT.md"
---

# Phase 65 Learnings: Generation Manifest and Metadata Mirroring

## Decisions

### Keep SQLite as the private local store

SQLite through the private analysis-kernel facade remains the right storage choice. Nothing in the failed delivery invalidated the relational, transactional, offline-store decision.

**Rationale:** The failure concerned scope and readiness, not the database technology.
**Source:** `research/local-semantic-store/README.md`; `65-CONTEXT.md`

### Keep complete-generation publication

A candidate generation must remain invisible until all rows required by that slice are written and validated; activation must preserve the prior complete generation on failure.

**Rationale:** Review findings WR-01, WR-02, WR-05, WR-23, WR-27, and WR-28 repeatedly demonstrated why header-only or partial validation is unsafe.
**Source:** `65-17-PLAN.md`; historical `65-REVIEW.md`; `65-REVIEW-FIX.md`

### Separate run truth from provider reuse identity

Full run/config/rule identity belongs in the run manifest. A provider cache key must contain only inputs that provider actually consumes.

**Rationale:** This prevents unrelated rule changes from destroying reuse while retaining complete run provenance.
**Source:** `65-03-SUMMARY.md`; `65-09-SUMMARY.md`

### Keep telemetry outside semantic identity

Durations, timestamps, cache outcomes, counters, and mtime hints must not alter semantic identities.

**Rationale:** Execution circumstances are not analysis meaning.
**Source:** `65-02-SUMMARY.md`; `65-12-SUMMARY.md`; `65-14-SUMMARY.md`

### Persist canonical edges once

Store one sorted, deduplicated dependency relation and derive forward and reverse indexes from it.

**Rationale:** Independently writable directional indexes can diverge.
**Source:** `65-11-SUMMARY.md`

### Treat provider outcome as a typed contract

Succeeded, failed, dependency-blocked, unsupported, and intentionally absent outcomes cannot be inferred from optional digests or empty fact sets.

**Rationale:** Several review findings showed failed providers being labeled trusted or consumed as partial universes.
**Source:** historical `65-REVIEW.md` findings WR-08, WR-10, WR-14, WR-15, WR-21, WR-22, and WR-26

### Do not keep the implementation as one delivery unit

The current implementation must be decomposed and restarted from main rather than merged or mechanically rebased.

**Rationale:** The phase and review both exceeded safe review and CI boundaries.
**Source:** `report-20260719-phase-65-scope-collapse.md`

## Lessons

### “Reuse existing vocabulary” requires a readiness audit

Existing types with the right names are not automatically canonical persistence contracts. Their completeness, status semantics, codecs, equality laws, and dependency coverage must be proven before schema planning.

**Context:** Fifteen plans of prerequisite redesign preceded the database lifecycle.
**Source:** `65-01-PLAN.md` through `65-15-PLAN.md`

### A plan with nineteen sequential parts is already multiple phases

Detailed planning did not make the phase safer; it concealed that the delivery unit was far too large.

**Context:** The planned implementation reached 119 files and 36,456 additions before review.
**Source:** `65-VERIFICATION.md`; git history at `8392cc47`

### Open-ended “missing critical” rules cause silent scope growth

Nearly every plan summary records auto-fixed blocking dependencies or private-scope adjustments. Those additions accumulated across the whole kernel without a re-planning checkpoint.

**Context:** Plans 06–19 repeatedly expanded beyond their declared file lists.
**Source:** `65-06-SUMMARY.md` through `65-19-SUMMARY.md`

### Review findings need disposition, not automatic implementation

A valid finding may be core to the changed feature, a prerequisite, an independent product bug, or an implementation-specific issue that disappears with a simpler design.

**Context:** Go process containment and filesystem hardening were valid concerns but not part of any original plan.
**Source:** historical `65-REVIEW.md`; `65-REVIEW-FIX.md`

### Security level must match the product boundary

Persisted analysis identity needs deterministic tool inputs, but that does not automatically require sealing an entire local Go toolchain closure in the same PR.

**Context:** SEC-01 through SEC-05 grew into more than twenty thousand lines of Go process code after phase completion.
**Source:** historical `65-REVIEW.md`; git diff after `8392cc47`

### Performance tests cannot govern ordinary correctness concurrency

Serializing all Go semantic tests to protect wall-clock fixture budgets made CI take up to an hour.

**Context:** `TEST_GO_SEMANTIC_MAX_CONCURRENCY` was set to one and allowed a 45-minute wait.
**Source:** `65-REVIEW-FIX.md`; `go/semantic/process.rs`; hosted CI run `29677260451`

### A “smoke” must be small

The supported-boundary smoke performed twelve full analyses and compiled a separate optimized profile. Its name obscured that it was a benchmark suite.

**Context:** The Windows step consumed more than fourteen minutes on a successful run.
**Source:** `eval/bench/gate.rs`; hosted CI runs `29673038157` and `29677260451`

### Verification must include delivery economics

Correctness counts and artifact completeness are insufficient when a change is not human-reviewable or cannot provide timely CI feedback.

**Context:** Verification reported no gaps and no human verification while the PR later reached 154 files and 85,404 additions.
**Source:** `65-VERIFICATION.md`; final branch diff

## Patterns

### Atomic active-pointer rotation

Write and validate a pending generation, mark it complete, then rotate one active pointer within the protected publication boundary.

**When to use:** Every durable store slice that can replace previously readable truth.
**Source:** `65-17-SUMMARY.md`

### Typed decode before trust

Persisted rows are untrusted until storage classes, sizes, codecs, references, canonical identities, and ordering have been validated through typed code.

**When to use:** Store reopen, identical-generation reuse, migration, and activation.
**Source:** historical `65-REVIEW.md` findings WR-01, WR-02, WR-05, WR-12, WR-27, and WR-28

### Provider-scoped dependency projection

Each provider declares and hashes the smallest exact input projection it consumes; downstream providers depend on producer outcomes rather than all raw sibling inputs.

**When to use:** Cache keys, provider output identities, and dependency edges.
**Source:** `65-08-SUMMARY.md`; `65-09-SUMMARY.md`

### Mutation pair tests

For every declared input, pair a must-invalidate mutation with an unrelated must-preserve-hit mutation.

**When to use:** Provider-key and invalidation work, one provider family at a time.
**Source:** `65-09-SUMMARY.md`; `65-CONTEXT.md`

### Conservative omission over speculative certification

If a provider’s complete reusable identity is not ready, omit that provider from durable reuse or mark it non-reusable instead of expanding the current PR to certify the world.

**When to use:** Restart slices encountering missing tool, environment, model, or extension identity.
**Source:** lessons from WR-09, WR-18, WR-19, SEC-04, SEC-05, and WR-25

### Bounded review with backlog routing

Fix findings introduced by or required for the current slice. Record valid transitive findings in a triage document and schedule them separately.

**When to use:** Every review loop on a persistence, security, or cross-platform change.
**Source:** `report-20260719-phase-65-scope-collapse.md`

## Surprises

### The prerequisites were larger than the store

The planned kernel identity and dependency migration occupied fifteen plans before schema work began.

**Impact:** The feature was unreviewable before its central SQLite behavior landed.
**Source:** `65-01-PLAN.md` through `65-16-PLAN.md`

### Review added more code than planned execution

The post-completion range added roughly 50,000 lines, compared with roughly 36,000 lines at phase completion.

**Impact:** Review became a second implementation project.
**Source:** git ranges `origin/main...8392cc47` and `8392cc47...HEAD`

### Most runtime hardening was absent from the plan

The largest changed file and the new Windows/local-filesystem modules were introduced only after deep review.

**Impact:** No plan or phase boundary constrained their threat model or delivery size.
**Source:** original plan file lists; historical `65-REVIEW.md`

### The full suite regressed from twenty minutes to an hour

The prior main workflow completed in about twenty minutes; the branch’s Windows job hit sixty minutes and still had work remaining.

**Impact:** Review/fix feedback became too slow to converge economically.
**Source:** hosted CI runs `29168290927` and `29677260451`

### Individual fixture tests exceeded five minutes

Several Windows fixture tests took six to nine minutes because one assertion test reran multiple complete analyses.

**Impact:** Sharding alone cannot meet a five-minute job target.
**Source:** hosted CI job logs from run `29677260451`

### Complete artifacts did not mean a healthy phase

All expected GSD documents existed and verification passed.

**Impact:** The workflow needs explicit size, reviewability, and CI-latency gates; artifact completeness cannot substitute for them.
**Source:** `65-VERIFICATION.md`; phase directory inventory
