---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 06
reviewed: 2026-08-03
depth: standard
files_reviewed: 13
files_reviewed_list:
  - crates/polint/src/analysis_kernel/go_syntax_projection.rs
  - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/analysis_kernel/store/generation.rs
  - crates/polint/src/analysis_kernel/store/go_syntax_mirror.rs
  - crates/polint/src/analysis_kernel/store/migrations.rs
  - crates/polint/src/analysis_kernel/store/mod.rs
  - crates/polint/src/analysis_kernel/store/tests.rs
  - crates/polint/src/go/adapter.rs
  - crates/polint/src/go/tests.rs
  - crates/polint/src/runner/mod.rs
  - crates/polint/tests/public_surface_leak.rs
findings:
  critical: 0
  blocker: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
reviewed_base: 4b925a08878d6113016b17a57b780542e1097a77
reviewed_head: 4a255b51
---

# Phase 65 Plan 06 Code Review

## Verdict

Plan 65-06 is not clean. The private `polint.go.syntax` increment is broadly
well-contained and all requested focused verification passes, but three
warnings remain: unrelated-language volume can reject a Go-only projection,
catalog authentication ignores whitespace inside quoted SQLite literals, and
the current-schema validator accepts extra tables in the Go provider's owned
namespace.

This verdict covers only the 13 product/test files in Plan 65-06 and the R5 Go
syntax increment. It does not certify the remaining TypeScript R5 work, R6
normal-run publication/reuse, mapped requirements, or Phase 65 completion.

## Warnings

### WR-01 — Unrelated-language rows can exceed Go projection bounds

**Evidence:**
`crates/polint/src/analysis_kernel/go_syntax_projection.rs:40-69`,
`crates/polint/src/analysis_kernel/go_syntax_projection.rs:130-148`, and
`crates/polint/src/analysis_kernel/go_syntax_projection.rs:151-268`

`CanonicalGoSyntaxInputs::from_db` rejects when the total number of files in
`AnalysisDb` exceeds `MAX_GO_ROWS` before it filters to `Language::Go`.
`CanonicalGoSyntaxOutput::from_db` likewise checks the unfiltered lengths of
every shared fact collection and the complete diagnostics vector before its
later Go-ownership and `parser/go` filters.

Consequently, a repository with a small, unchanged Go projection but more than
one million TypeScript/JavaScript files, facts, or unrelated diagnostics fails
the Go provider as an invalid projection. That makes excluded TypeScript-only
state affect Go success/match behavior and conflicts with the locked Go-only
identity and must-preserve contract. It is also a local denial-of-service seam
in a sufficiently large mixed-language repository.

**Fix:** Apply the bound to owned Go files, owned Go fact rows, and `parser/go`
diagnostics rather than to the shared backing collections. Keep the operation
bounded by filtering with an early `take(MAX_GO_ROWS + 1)` or an equivalent
checked counter before materialization. Add a regression around a test-sized
bound/helper proving arbitrarily many unrelated-language rows do not change or
reject an otherwise identical Go projection.

### WR-02 — Catalog normalization changes quoted SQL literals

**Evidence:** `crates/polint/src/analysis_kernel/store/migrations.rs:81-85`,
`crates/polint/src/analysis_kernel/store/migrations.rs:376-414`, and
`crates/polint/src/analysis_kernel/store/migrations.rs:584-638`

Each new Go table declaration is authenticated by comparing
`normalize_schema_sql(actual)` with the expected declaration. The normalizer
removes every ASCII whitespace character without tracking SQLite quoting. It
therefore also removes whitespace inside string literals. For example, a
`writable_schema` mutation from `'unsupported'` to `'unsup ported'` produces a
semantically different CHECK constraint but normalizes to the same string as
the expected catalog declaration. Existing successful rows need not exercise
that altered branch, so the remaining row and foreign-key checks can still
accept the forged catalog.

The oversized-catalog regression only inserts formatting whitespace outside a
literal; it does not cover this semantic collision. This falls short of the
plan's exact owned-catalog authentication requirement and can make a current
store fail later writes under a declaration that maintenance previously
trusted as exact.

**Fix:** Make catalog normalization quote-aware and discard formatting
whitespace only outside SQLite string/identifier literals, or compare against
a canonical representation that preserves literal contents. Add a
`writable_schema` regression that inserts whitespace inside a quoted CHECK
literal, reopens the store, and requires `RebuildNeeded(InvalidSchema)`.

### WR-03 — Extra Go-owned tables are accepted by schema validation

**Evidence:** `crates/polint/src/analysis_kernel/store/migrations.rs:27-38`,
`crates/polint/src/analysis_kernel/store/migrations.rs:300-328`,
`crates/polint/src/analysis_kernel/store/migrations.rs:376-414`, and
`crates/polint/src/analysis_kernel/store/tests.rs:311-317`

The validator authenticates the five expected Go tables one by one and checks
indexes/triggers attached to those tables, but it never enumerates the reserved
`go_syntax_provider_*` namespace and compares it with the five-name allowlist.
`validate_owned_names_absent` has the same limitation for pre-v5 schemas: it
checks collisions only for the five exact expected names.

As a result, `CREATE TABLE go_syntax_provider_shadow (...)` survives current
schema validation; an empty v4 store containing that unexpected owned object
can also migrate to v5. The fresh-schema test proves that initialization emits
five tables, while the tamper matrix covers an extra index and trigger on
expected tables but not an extra owned table. This violates the one-and-only-one
five-table family and colliding-store refusal requirements.

**Fix:** Enumerate tables and other catalog objects in the reserved Go provider
namespace, compare them case-safely with the exact allowlist, and perform the
same namespace collision check before migration. Add current-v5 and empty-v4
regressions for an extra `go_syntax_provider_*` table (and any attached
index/trigger), requiring refusal without mutation.

## Verified Behavior

- The provider manifest declares the exact six Go-owned outputs, including
  `string_literals`, and mixed-language capability ownership is source-driven.
- Canonical Go input/output identity, parser identity, exact raw layer
  dependency validation, cold/warm/disabled parity, and cache-warning isolation
  pass their focused tests.
- Schema-v5 migration/refusal, Go mirror storage/reopen/match, publication
  rollback, generation lifecycle, runner parity, and public-surface probes pass.
- `cargo fmt --all -- --check`, strict workspace all-target/all-feature Clippy,
  and workspace all-target/all-feature check pass.
- Diff hygiene, allowed-file scope, protected-file, forbidden-persistence, and
  normal-kernel-wiring audits pass. Normal execution still calls only
  `SemanticStore::maintain`; no Go store publication/reuse path was wired.

## Review Boundary

No product source, tests, existing planning artifacts, state/roadmap files,
branches, commits, remotes, pull requests, or CI configuration were modified by
this review. Only this review report was created.

---
_Reviewed: 2026-08-03_
_Reviewer: gsd-code-reviewer_
_Depth: standard_
