---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 05
status: clean
depth: standard
files_reviewed: 12
findings:
  critical: 0
  warning: 0
  info: 0
  total: 0
reviewed_base: 013ff41c3b350e7918217f0e663c4c462b38ef94
reviewed_head: 4b925a08878d6113016b17a57b780542e1097a77
re_review: true
fix_commit: 4b925a08878d6113016b17a57b780542e1097a77
prior_findings:
  critical: 1
  warning: 1
  total: 2
---

# Phase 65 Plan 05 Code Review

## Verdict

Clean. Fix commit `4b925a08` fully closes the prior Critical and Warning, and
the cumulative R4 range has no remaining Critical, Warning, or informational
finding.

The implementation still mirrors only private `polint.metrics` metadata. It
does not persist metric facts, widen supported surfaces, or enable the R5/R6
normal-run publication/reuse path.

## Fixed Finding History

### CR-01 — Closed: catalog SQL is bounded before Rust decoding

**Evidence:** `crates/polint/src/analysis_kernel/store/migrations.rs:27`,
`crates/polint/src/analysis_kernel/store/migrations.rs:482-529`, and
`crates/polint/src/analysis_kernel/store/tests.rs:487-514`

`validate_table_sql` now performs a scalar preflight that requires exact
cardinality, TEXT storage class, and a BLOB byte length no greater than the
expected declaration plus 4,096 bytes of formatting headroom. It returns a
typed invalid-schema result before calling `Row::get::<String>` when any check
fails. The subsequent decode query repeats the same type and length predicate,
so neither String decoding nor whitespace normalization can receive an
unbounded catalog cell.

The hostile regression inflates an otherwise equivalent provider-table
declaration with 100,000 bytes of whitespace through `writable_schema`, closes
the tamper connection, and proves normal maintenance refuses the store as
`RebuildNeeded(InvalidSchema)`. T-65-05-04 is closed.

### WR-01 — Closed: every non-success outcome rejects dependency rows

**Evidence:**
`crates/polint/src/analysis_kernel/store/provider_mirror.rs:142-152`,
`crates/polint/src/analysis_kernel/store/migrations.rs:840-875`, and
`crates/polint/src/analysis_kernel/store/tests.rs:384-485`

The mirror reader now decodes the closed outcome status immediately after its
bounded header read and rejects any non-`Succeeded` header with a nonzero
source or function count before reading members or child dependencies.
Whole-schema validation independently restates the same status/count invariant,
in addition to checking header-to-child cardinalities.

The regression covers both legal `Failed` forms and every other non-success
status. For each status it disables CHECK enforcement, inserts coherent source
and function rows with matching counts, proves the direct reader rejects the
row before child trust, then closes and reopens the store and proves
current-schema validation fails closed. The exact non-success durable shape is
restored.

## Threat Regression Audit

- **T-65-05-01 — closed.** Cold, validated warm-hit, cache-disabled,
  corrupt-cache, and cache-write-warning paths preserve metric facts and sealed
  provider identity; telemetry remains separate.
- **T-65-05-02 — closed.** One canonical sorted, multiplicity-preserving
  source/function projection drives the metrics key, dependency edges, output
  identity, durable mirror, and invalidate/preserve matrix. Locked exclusions
  remain outside identity.
- **T-65-05-03 — closed.** Static manifest, closed outcome, identity, blocker,
  source, function, count, relationship, and exact legal row shape are checked
  before a succeeded projection can be returned or matched.
- **T-65-05-04 — closed.** Catalog SQL, required and optional headers, child
  counts, aggregate bytes, ordinals, numeric fields, labels, and relationships
  are bounded and type-checked before Rust materialization or semantic trust.
- **T-65-05-05 — closed.** Manifest and provider rows are written and read back
  before completion/selection in one immediate transaction; every failure seam
  rolls the candidate back and preserves prior selected truth.
- **T-65-05-06 — closed.** Only authenticated empty v3 migrates; populated v3
  is preserved and refused before mutation.
- **T-65-05-07 — closed.** Workspace ownership is enforced, and selected handle,
  manifest, and provider state are authenticated in one read snapshot.
- **T-65-05-08 — closed.** The production kernel's sole semantic-store call is
  still `SemanticStore::maintain`; reserve, publish, active-read, and match
  operations remain private and unwired from normal execution.

The earlier requested fixes also remain intact: optional identity header
strings are byte-bounded before decode; blockers are sorted unique actual hard
dependencies; empty source byte/line counts are symmetric; dependency ordinals
are dense; and irrelevant metrics key slots use purpose-specific typed absence.

## Scope, Budget, and Privacy

The exact cumulative product/test range is
`013ff41c3b350e7918217f0e663c4c462b38ef94..4b925a08878d6113016b17a57b780542e1097a77`:

- 12 allowed product/test files, 2,281 additions, and 386 deletions.
- One new durable schema family and one persisted provider family,
  `polint.metrics`.
- No protected CI, ROADMAP, REQUIREMENTS, STATE, docs, examples, CLI, or SDK
  changes.
- No forbidden fact, metadata, validation, telemetry, snapshot, generic-index,
  query, or summary persistence in the provider mirror/schema.
- The supported-surface probe passes all seven tests; store/SQL/provider
  vocabulary remains private.
- R5/R6, Phase 65 completion, and mapped requirements remain outside this
  review and were not certified.

## Verification

- `cargo test -p polint --lib analysis_kernel::store --locked -- --test-threads=1`
  — 65 passed, including both remediation regressions, migrations, tamper
  refusal, publication rollback, workspace isolation, and reopen/match tests.
- Canonical metrics projection — 2 passed.
- Metrics-only key identity — 1 passed.
- Metrics derivation/cache tests — 29 passed.
- Cold/warm/cache-disabled production projection — 1 passed.
- Store-mode JSON/exit parity — 1 passed.
- `cargo test -p polint --test public_surface_leak --locked` — 7 passed.
- Workspace all-target/all-feature strict Clippy — passed.
- Workspace all-target/all-feature check — passed.
- `cargo fmt --all -- --check` — passed.
- Diff hygiene, allowed-file, addition/file-cap, protected-file, forbidden
  persistence, and normal-kernel-wiring audits — passed.

The previous review's overbroad library run exposed three marker-scan failures
whose offending lines already existed at the review base; the remediation did
not touch those lines. They are not findings in this R4 diff.

No product source, test, plan, summary, commit, branch, or remote state was
modified by this re-review. Only this review report was overwritten.

---
_Reviewed: 2026-08-02_
_Reviewer: gsd-code-reviewer_
_Depth: standard remediation re-review_
