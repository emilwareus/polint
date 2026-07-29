---
phase: 65-generation-manifest-and-metadata-mirroring
scope: r1-r3
retained_history: "R1-R2 review body preserved verbatim"
depth: standard
status: issues_found
iteration: 3
diff_base: c453748c
files_reviewed: 14
files_reviewed_list:
  - crates/polint/src/analysis_kernel/outcome.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/incremental/mod.rs
  - crates/polint/src/analysis_kernel/validation.rs
  - crates/polint/src/analysis_kernel/incremental/stats.rs
  - crates/polint/src/analysis_kernel/incremental/run_report.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/go/semantic/provider.rs
  - crates/polint/src/eval/performance.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/eval/semantic_graph_snapshot.rs
  - crates/polint/src/symbol_graph/mod.rs
  - crates/polint/src/runner/mod.rs
  - crates/polint/tests/public_surface_leak.rs
findings:
  critical: 1
  warning: 1
  info: 0
  total: 2
reviewed_at: 2026-07-29
---

# Phase 65 R1-R2 Code Review

## Verdict

Clean. The three iteration-1 findings are closed by commits `48c5cce2`,
`00979854`, and `27506a52`. A deep re-review of the full current contents of
the eight scoped files found no new critical or warning issues and no R1
regression.

## Scope

Reviewed the full `f3f4612f..27506a52` implementation history and current
source, including the iteration-1 fixes rather than relying on
`65-REVIEW-FIX.md`. The review retraced canonical manifest construction,
schema authentication, migration/open policy, generation publication,
one-snapshot active reads and matching, rollback behavior, workspace ownership,
typed error mapping, and tamper tests.

The three fix commits change only
`incremental/run_manifest.rs`, `store/migrations.rs`, and `store/tests.rs`.
The combined R1-R2 implementation remains within the same eight-file private
boundary and does not add provider families, public CLI/config/SDK/output
surface, normal-kernel publication/reuse wiring, or CI workflow changes.

## Iteration-1 Finding Closure

### CR-01 — Closed: complete generations now require the singleton active row

**Evidence:** `crates/polint/src/analysis_kernel/store/migrations.rs:642-695`,
`crates/polint/src/analysis_kernel/store/connection.rs:43-60`,
`crates/polint/src/analysis_kernel/store/connection.rs:107-130`, and
`crates/polint/src/analysis_kernel/store/tests.rs:315-392`

`validate_manifest_rows` now compares `EXISTS(complete generation)` with
`EXISTS(active_generation)` and rejects any mismatch. The exact active-table
shape and lifecycle validation already bound the present case to one singleton
row pointing at a complete generation; manifest ownership validation binds
that selection to an existing manifest. Empty and pending-only stores remain
valid because both existence predicates are false.

The invariant is checked by writer preflight and again inside every immediate
mutation transaction. Active reads and manifest matching initialize/open the
store and then validate the current schema inside their read transaction before
reading selected truth. Deleting the active row can therefore no longer become
normal absence or bypass the workspace-ownership check.

The regression publishes workspace A, reserves a candidate, deletes the active
row, and proves active read, exact match, maintenance, same-workspace
publication, and second-workspace publication all return the typed
`RebuildNeeded(InvalidSchema)` outcome without changing the complete or pending
rows.

### CR-02 — Closed: manifest index authentication is bounded and streamed

**Evidence:** `crates/polint/src/analysis_kernel/store/migrations.rs:537-596`
and `crates/polint/src/analysis_kernel/store/tests.rs:394-436`

The validator first obtains index cardinality with scalar `count(*)` and
rejects unless it is exactly one. It then decodes only that sole row's bounded
metadata. Index columns are read as a stream: exactly the first two names are
decoded and compared with `generation_id` and `relative_path`, while a third
row is checked only for existence. No attacker-controlled index or column
catalog is collected into a Rust vector.

The real writer-open regression installs 128 extra indexes, receives the typed
invalid-schema/rebuild result, and proves the catalog is preserved rather than
mutated.

### WR-01 — Closed: portable absolute/prefixed paths are rejected

**Evidence:**
`crates/polint/src/analysis_kernel/incremental/run_manifest.rs:320-339` and
`crates/polint/src/analysis_kernel/incremental/run_manifest.rs:418-545`

Canonical source validation now rejects native `Prefix`/`RootDir` components,
portable ASCII drive prefixes such as `C:` (including drive-relative forms),
and leading POSIX or Windows separators before applying slash/dot
normalization. Build and stored-decode tests cover POSIX absolute,
drive-absolute, drive-relative, UNC, and verbatim-prefix spellings.

The checks are limited to actual root/prefix forms. Ordinary canonical relative
names such as `src/app.ts`, `src/a.ts`, and `src/b.go` still construct,
round-trip, sort, encode, and decode successfully, so the fix does not reject
the valid repo-relative path contract.

## Regression Review

- Publication still writes the header and sources, reads and recomputes the
  stored identity, completes the supplied reserved handle, and rotates the
  singleton selection in one immediate transaction.
- Every publication failure seam still rolls back the candidate manifest,
  leaves it pending, and preserves the prior active manifested generation
  across reopen.
- Active handle, header preflight, source preflight, row decode, and exact
  comparison still share one validated read transaction/snapshot.
- Header/source storage classes, counts, scalar lengths, aggregate payload
  bytes, checked numeric conversions, closed labels, canonical ordering,
  ownership, and run identity remain authenticated before trust.
- Exact empty-v2 migration, populated-v2 refusal before persistent policy,
  transactional revalidation, current-v3 idempotence, future-schema refusal,
  and malformed-schema preservation remain intact.
- Disabled entry points still return before path creation, workspace
  canonicalization, manifest construction, or SQLite access.
- Production visibility remains crate-private, and the fixes introduce no
  delivery-history comments or supported-surface widening.

## Verification

- `cargo test -p polint --lib analysis_kernel::incremental::run_manifest::tests --locked -- --nocapture`
  — 8 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests --locked -- --nocapture`
  — 37 passed.
- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked -- --nocapture`
  — 21 passed.
- `cargo test -p polint --test public_surface_leak --locked -- --nocapture`
  — 7 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked -- --nocapture`
  — 1 passed with byte-identical JSON and exit semantics.
- `make lint` — passed, including workspace/all-target/all-feature Clippy with
  warnings denied and formatting validation.
- `git diff --check f3f4612f..HEAD -- <eight reviewed files>` — passed.

No product source, test, fix report, backup artifact, or commit was modified by
this review.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: deep_

---

## R3 Provider Outcome Review

### Verdict

Issues found. The R3 provider-outcome implementation passes its focused tests
and workspace checks, but one supported capability can still dispatch a rule
after its authoritative provider closure has failed. A second issue makes
rendered diagnostic evidence part of the trust-sealing mechanism.

Counts: **1 Critical**, **1 Warning**, **0 Info**.

### Scope

This standard-depth review covers the 14 files listed in the current
frontmatter, using `c453748c` as the diff base and product commits `3a00f148`,
`cd5aed91`, and `91db8313`. Supporting contract files were read only to verify
the established capability semantics; they are not included in the reviewed
file count. No store-schema, migration, CI, or other product file was added to
the review scope. The complete R1/R2 review body above is retained verbatim.

### Findings

#### CR-01 — Supported `events` rules bypass sealed syntax-provider failures

**Evidence:** `runtime_capability_blockers` skips any supported capability whose
`capability_providers` result is empty
(`crates/polint/src/analysis_kernel/mod.rs:1213-1223`), while the static mapping
has no `events` arm and therefore returns the empty default
(`crates/polint/src/analysis_kernel/mod.rs:1267-1283`). The same file selects
both syntax providers for every run
(`crates/polint/src/analysis_kernel/mod.rs:1156-1171`) and its events-only test
confirms that `events` deliberately remains on that lightweight syntax path
(`crates/polint/src/analysis_kernel/mod.rs:2340-2391`). Production dispatch
only skips IDs already present in `runtime_blocked_rules`
(`crates/polint/src/runner/mod.rs:434-449`).

The established supporting contract also marks `events` as supported
(`crates/polint/src/analysis_plan.rs:700-705`), keeps events-only rules on the
syntax path (`crates/polint/src/analysis_plan.rs:1202-1210`), and documents
syntax-derived function-call matching
(`crates/polint/src/sdk/facts.rs:851-863`).

**Impact:** If authoritative validation downgrades `polint.go.syntax`,
`polint.ts.syntax`, or their source closure, an `events` rule is not runtime
blocked and receives no capability diagnostic. The runner can execute it
against partial or invalid facts, violating the fail-closed invariant for a
supported hard capability.

**Recommended fix:** Give `events` an explicit syntax-provider closure and
model its opportunistic refined-call upgrade without weakening the hard syntax
dependency. Add an outcome-level regression and a production-dispatch test
that force a syntax-provider non-success and prove an events rule cannot run.

#### WR-01 — Validation downgrade ownership is reconstructed from diagnostics

**Evidence:** Validation first builds and sorts `Vec<Diagnostic>`, then clones
each rendered diagnostic into `validation_issue`
(`crates/polint/src/analysis_kernel/validation.rs:82-122`). That conversion
parses evidence labels such as `family`, `fact_ref`, `stable_key`, and
`producer_id`, then scans fact metadata to infer the provider IDs to downgrade
(`crates/polint/src/analysis_kernel/validation.rs:125-173`). Those inferred IDs
directly determine provider-specific versus global trust downgrades
(`crates/polint/src/analysis_kernel/validation.rs:61-70`).

**Impact:** Diagnostic wording and evidence are now semantic trust inputs.
Changing renderer evidence, or matching an ambiguous stable key or family to a
valid provider, can narrow a downgrade that should have remained global. This
reverses the intended ownership flow: structured validation issues should own
provider/family attribution, with diagnostics as an observation projection.

**Recommended fix:** Have validators emit structured issues containing reason,
fact family, and authoritative provider ownership at the detection site, then
render diagnostics from those issues. Keep ambiguous or unknown ownership
fail-closed as global, and test that diagnostic-rendering changes cannot alter
the downgrade set.

### Confirmed R3 Invariants

- The deterministic inventory records one terminal state for every provider,
  preserves manifest order, and assigns identities only to succeeded outcomes.
- Direct hard-provider dependencies and the fixed-point downgrade propagation
  align with the provider inputs inspected in this slice.
- Typed provider-failure signals, cache validation failures, recomputes, and
  cache-write warning telemetry remain distinct and deterministically reported.
- Runtime blockers reach production dispatch before rule execution for the
  capabilities present in the static mapping.
- The reviewed public-surface leak test passes, and no store-schema or migration
  boundary was expanded by the R3 diff.

### Verification

- `cargo test -p polint --lib analysis_kernel::outcome::tests --locked` — 6
  passed.
- `cargo test -p polint --lib analysis_kernel::validation::tests --locked` — 9
  passed.
- `cargo test -p polint --lib analysis_kernel::tests::provider_outcomes --locked`
  — 2 passed.
- `cargo test -p polint --lib core::tests::run_rules_skips_rules_with_runtime_provider_blockers --locked`
  — 1 passed.
- `cargo test -p polint --lib runner::tests::production_dispatch_forwards_runtime_provider_blockers --locked`
  — 1 passed.
- `cargo test -p polint --test public_surface_leak semantic_store_markers_do_not_leak_into_supported_public_surfaces --locked`
  — 1 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked`
  — 1 passed.
- `cargo test -p polint --lib eval::performance::tests --locked` — 6 passed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p polint --lib --tests --all-features --locked -- -D warnings`
  — passed.
- `cargo check --workspace --all-features --locked` — passed.
- `git diff --check c453748c..HEAD -- <14 reviewed files>` — passed.

No product source, test, fix report, backup artifact, commit, or push was
modified by this review.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: standard_
