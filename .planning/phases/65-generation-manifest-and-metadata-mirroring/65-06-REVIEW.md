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
  warning: 2
  info: 0
  total: 2
status: issues_found
reviewed_base: 4b925a08878d6113016b17a57b780542e1097a77
reviewed_head: 02610e9da6a783b52fafd679ac0171a6ddf2ec1b
---

# Phase 65 Plan 06 Code Review

## Verdict

Repair commit `02610e9d` resolves WR-01, WR-02, and WR-03. Plan 65-06 is
still not clean because two warnings remain in canonical Go output validation:
Go test/branch facts can lose or cross their function relationship, and
duplicate function-row detection is quadratic at a one-million-row boundary.

This verdict covers only the 13 product/test files in Plan 65-06 and the R5 Go
syntax increment. It does not certify the remaining TypeScript R5 work, R6
normal-run publication/reuse, mapped requirements, or Phase 65 completion.

## Resolved Findings

### WR-01 — Resolved: owned bounds are applied after language filtering

`CanonicalGoSyntaxInputs::from_db` now filters non-Go files before incrementing
the bounded counter, and each fact/diagnostic loop counts only rows owned by Go
or `parser/go`
(`crates/polint/src/analysis_kernel/go_syntax_projection.rs:17-23`,
`:49-75`, and `:142-288`). The regression at `:645-663` proves unrelated TS
files, facts, literals, and diagnostics preserve the Go projection and that the
bounded helper rejects only after the owned limit is reached.

### WR-02 — Resolved: schema normalization preserves quoted whitespace

`normalize_schema_sql` now tracks SQLite string and identifier delimiters and
removes formatting whitespace only while outside a quoted region
(`crates/polint/src/analysis_kernel/store/migrations.rs:659-682`). The current-v5
tamper regression changes `'unsupported'` to `'unsup ported'` and requires
`RebuildNeeded(InvalidSchema)`
(`crates/polint/src/analysis_kernel/store/tests.rs:945-953`).

### WR-03 — Resolved: the complete reserved Go namespace is authenticated

`validate_go_provider_namespace` enumerates the case-insensitive
`go_syntax_provider_` namespace and accepts exactly the five binary-cased table
names in v5 and zero objects before v5
(`crates/polint/src/analysis_kernel/store/migrations.rs:281-285`, `:382-440`).
Current-v5 extra table/index/trigger tampering is refused
(`crates/polint/src/analysis_kernel/store/tests.rs:945-953`), and an empty v4
store with the same collision is refused without catalog mutation
(`crates/polint/src/analysis_kernel/store/migrations.rs:1520-1529`).

## Warnings

### WR-04 — Go test and branch function relationships are not exact

**Evidence:**
`crates/polint/src/analysis_kernel/go_syntax_projection.rs:205-210`,
`:226-235`, and `:505-515`; `crates/polint/src/go/adapter.rs:808-823`,
`:946-964`, and `:1599-1627`

The canonicalizer resolves a function through a run-global
`FunctionId -> row digest` map. `function_ref(None, ...)` returns the empty
string, and a present ID is accepted without proving that its function belongs
to the test or branch file. Go extraction always emits `Some` with the
same-file function ID for both families, so either accepted state contradicts
the provider's produced relationship and can be sealed instead of rejected.

This does not meet the plan's requirement that branch/test references resolve
to the exact canonical function occurrence, and the mutation matrix has no
absent, dangling, or cross-file relationship cases.

**Remediation:** Replace the digest-only function map with canonical function
metadata containing at least the digest, file ID/path, and span. Require a
present function for every Go test and branch, require that it resolves in the
same file, and validate the expected span relationship. Add table-driven
mutations for `None`, a dangling ID, and a different-file ID for both families;
each must reject before a provider output digest is sealed.

### WR-05 — Duplicate function-row validation is quadratic

**Evidence:**
`crates/polint/src/analysis_kernel/go_syntax_projection.rs:11` and `:142-170`

For every owned function, `function_rows.contains(&row)` linearly rescans all
previous row digests. Building a projection with `n` functions therefore does
O(n^2) string comparisons even though the accepted bound is one million rows.
Large repositories can spend disproportionate CPU in validation before sorting,
creating a local denial-of-service/performance seam in the provider trust path.

**Remediation:** Track semantic function-row identities in a `BTreeSet` or
`HashSet` and reject when insertion reports an existing row, while retaining
the vector for deterministic final sorting and the ID map for relationships.
Add a synthetic many-function regression (or comparison-counted helper test)
that proves duplicate detection scales no worse than O(n log n).

## Verified Behavior

- All 13 focused test targets required by Plan 65-06 pass; every target
  completed in under five seconds after compilation.
- Provider inventory/ownership, all six Go fact families, parser identity,
  exact raw layer edges, cold/warm/disabled parity, cache-warning isolation,
  schema-v5 migration, mirror reopen/match/tamper refusal, three-projection
  rollback, generation lifecycle, runner parity, and public-surface probes pass.
- `cargo fmt --all -- --check`, strict workspace all-target/all-feature Clippy,
  workspace all-target/all-feature check, and cumulative diff hygiene pass.
- Allowed-file, addition/file-cap, protected-file, forbidden-persistence, and
  normal-wiring audits pass. Production `AnalysisKernel::run` still calls only
  `SemanticStore::maintain`; no Go store publication/reuse path is wired.

A supplemental full-workspace test run, which the plan explicitly excludes
from its required path, reported four pre-existing failures. Their triggering
tests, runner markers, and provider-selection behavior are unchanged from the
review base; they are not attributed to this 13-file increment.

## Review Boundary

No product source, tests, other planning artifacts, state/roadmap files,
branches, commits, remotes, pull requests, or CI configuration were modified by
this review. Only this review report was updated.

---
_Reviewed: 2026-08-03_
_Reviewer: gsd-code-reviewer_
_Depth: standard_
