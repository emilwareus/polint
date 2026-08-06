---
phase: 65-generation-manifest-and-metadata-mirroring
scope: r1-r3-gap-closure
retained_history: "R1-R3 review body preserved verbatim"
depth: deep
status: clean
iteration: 9
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
  critical: 0
  warning: 0
  info: 0
  total: 0
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

---

## R3 Remediation Re-review — Iteration 1

### Verdict

Issues found. Commits `fde65ff0`, `a1da836e`, and `f7f593ac` close the two
original R3 findings, but the events capability still has one provider-trust
escape when another rule activates the opportunistic call/refinement path.

Current open counts: **1 Critical**, **0 Warning**, **0 Info**.

### Scope

This fresh standard-depth pass reviewed the current contents and
`c453748c..f7f593ac` history of the same 14 product/test files listed in the
frontmatter. It independently traced the fixes through authoritative
validation, provider sealing, capability closure, production dispatch, and the
Events fact view. The R1/R2 body and the original R3 finding history above are
preserved verbatim.

### Original Finding Closure

#### CR-01 — Closed: applicable syntax-provider failures now block `events`

**Evidence:** `crates/polint/src/analysis_kernel/mod.rs:1208-1302` and
`crates/polint/src/runner/mod.rs:424-449`

The `events` capability now receives a hard syntax closure filtered to the
languages actually loaded in `AnalysisDb`. A non-success applicable syntax
outcome creates the existing deterministic capability diagnostic and inserts
the requesting rule ID into the sealed runtime blocker set. Production dispatch
forwards that set before `RuleCtx` construction. The focused Go-only regression
also proves an absent TS corpus does not introduce a false TS blocker.

#### WR-01 — Closed: rendered evidence no longer determines downgrade ownership

**Evidence:** `crates/polint/src/analysis_kernel/validation.rs:44-89`,
`crates/polint/src/analysis_kernel/validation.rs:102-154`, and
`crates/polint/src/analysis_kernel/validation.rs:5801-5827`

Validation aggregation now assigns provider ownership explicitly at each
provider-specific validator boundary. `ValidationReport::downgrades` reads only
the structured `provider_ids`; it does not scan diagnostic messages, evidence
labels, fact references, stable keys, or family text. Diagnostics are rendered
from the issue projection, and the regression proves that changing the stored
reason/evidence leaves the owned downgrade set unchanged. Validators without
authoritative ownership remain global and fail closed.

### New Finding

#### CR-02 — Failed opportunistic call/refinement facts can still reach an `events` rule

**Evidence:** `crates/polint/src/analysis_kernel/validation.rs:133-143`,
`crates/polint/src/analysis_kernel/mod.rs:1007-1012`, and
`crates/polint/src/analysis_kernel/mod.rs:1273-1302`. Supporting read-only
contract evidence:
`crates/polint/src/policy_queries.rs:141-194` and
`crates/polint/src/analysis/refined_calls/validate.rs:13-97`.

The new `events` closure contains only the applicable syntax providers.
However, `Events::matching` prefers non-empty `refined_call_edges`, then
`call_sites`, whenever another rule caused those providers to run. Authoritative
validation can reject either provider after its rows are already present:
`validate_calls` owns `polint.calls` and `validate_refined_calls` owns
`polint.refined_calls`. Sealing correctly marks those outcomes failed or
dependency-blocked, but validation is read-only and the rows remain in
`AnalysisDb`. Because an events-only rule requests neither `calls` nor
`control_flow`, its runtime closure sees successful syntax providers and lets it
execute against the non-success provider's rows.

**Impact:** In a mixed rule pack, malformed or validation-rejected enriched call
facts can determine an events policy answer after their provider has explicitly
lost trust. This violates the fail-closed rule-dispatch invariant and the
documented syntax fallback contract; a policy can report, omit, or misidentify
an event from facts that R3 has sealed as non-reusable. This is a reproducible
HIGH trust-boundary path and blocks R3 completion.

**Recommended fix:** When the call/refinement providers are scheduled for the
run, include their final outcomes in the `events` effective closure (while
keeping planned-absent enrichments optional), or add an explicit outcome-aware
fallback that prevents `Events::matching` from reading their rows after
non-success. Add a mixed-plan production regression with an events-only rule and
a separate calls-triggering rule; force an owned calls/refined validation
rejection and prove the events rule is either blocked before `RuleCtx` or uses
only the certified syntax fallback.

### Regression Review

- Provider outcome rows remain closed, manifest ordered, success-identity-only,
  and separate from cache telemetry.
- Validation ownership is no longer reconstructed from rendered diagnostics;
  unattributable issues remain global.
- Applicable syntax failures now reach the production runtime blocker path for
  events rules.
- Cold/warm outcomes, store-mode public bytes, strict linting, and private
  vocabulary gates remain green.
- The remediation stays in exactly 14 product/test files at 2,497 additions and
  729 deletions, with no store, public API, CI, STATE, ROADMAP, or REQUIREMENTS
  change.

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
- `cargo test -p polint --lib policy_queries::tests::events_matching_call_returns_provider_backed_violation --locked`
  — 1 passed and confirms events prefer provider-backed refined facts.
- `cargo test -p polint --lib analysis::refined_calls::validate::tests::validation_catches_dangling_call_site --locked`
  — 1 passed and confirms authoritative validation can reject rows retained in
  the database.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p polint --lib --tests --all-features --locked -- -D warnings`
  — passed.
- `cargo check --workspace --all-features --locked` — passed.
- `git diff --check c453748c..HEAD -- crates/polint` — passed.

No product source, test, fix report, summary, plan/context, tracking file, CI
workflow, commit, branch, or remote state was modified by this re-review.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: standard_
_Remediation iteration: 1_

---

## R3 Remediation Re-review — Iteration 2

### Verdict

Issues found. Commit `28689ca7` closes CR-02, and the original CR-01 and WR-01
closures remain intact. One adjacent provider-readiness defect still turns a
controlled dependency failure into forbidden provider execution and a possible
kernel panic.

Current open counts: **1 Critical**, **0 Warning**, **0 Info**.

### Scope

This fresh standard-depth pass reviewed the current contents and
`c453748c..28689ca7` history of the same 14 product/test files listed in the
frontmatter. It retraced the events fact-selection path, row-sensitive effective
closure, validation ownership, provider sealing, production dispatch, and the
reachability-to-semantic-graph identity handoff. The R1/R2 body, original R3
findings, and iteration-1 history above are preserved verbatim.

The cumulative product/test diff remains exactly 14 files, 2,500 additions, and
732 deletions. It contains no store, public API, CI, STATE, ROADMAP, or
REQUIREMENTS change.

### Finding Closure

#### CR-01 — Closed: applicable syntax-provider failures still block `events`

**Evidence:** `crates/polint/src/analysis_kernel/mod.rs:1278-1307`,
`crates/polint/src/analysis_kernel/mod.rs:1201-1275`, and
`crates/polint/src/runner/mod.rs:434-449`

The base events closure still includes only syntax providers for languages
present in the loaded corpus. Any applicable non-success outcome produces the
existing deterministic capability diagnostic and adds the requesting rule to
the sealed runtime blocker set. The production adapter forwards that set to the
core blocker check before `RuleCtx` construction.

#### WR-01 — Closed: rendering remains a one-way validation projection

**Evidence:** `crates/polint/src/analysis_kernel/validation.rs:42-99`,
`crates/polint/src/analysis_kernel/validation.rs:101-156`, and
`crates/polint/src/analysis_kernel/validation.rs:5802-5827`

`ValidationReport::downgrades` reads only structured `provider_ids`. It never
parses the diagnostic message, evidence labels, fact references, stable keys,
or family display text. Provider-specific validator boundaries assign
ownership directly; unowned validators remain global and fail closed. The
focused regression changes the stored reason/evidence while preserving the
downgrade set.

#### CR-02 — Closed: scheduled enrichment outcomes now participate only when their rows can affect `events`

**Evidence:** `crates/polint/src/analysis_kernel/mod.rs:1219-1243`,
`crates/polint/src/policy_queries.rs:141-204`,
`crates/polint/src/runner/mod.rs:539-610`, and
`crates/polint/src/analysis_kernel/mod.rs:706-715`

The events closure now adds `polint.calls` only when call-site rows exist and
adds `polint.refined_calls` only when refined rows exist. A planned-absent
provider is excluded, so events-only plans and rowless optional enrichment do
not acquire false blockers. A scheduled enrichment with influencing rows must
be sealed `Succeeded`; otherwise the events rule is blocked.

The mixed-plan production regression uses an events rule plus a separate calls
rule, retains rejected refined rows, confirms authoritative ownership by
`polint.refined_calls`, derives two runtime blockers, and proves both affected
rule counters remain zero while an unrelated rule executes once through the
real dispatch adapter. The core blocker check occurs before `RuleCtx`
construction. The reachability digest is also now read only after the
reachability projection is recorded, so the mixed deep plan reaches
semantic-graph execution with the correct identity.

### New Finding

#### CR-03 — A dependency-blocked calls provider still executes its fallback and can panic

**Evidence:** `crates/polint/src/analysis_kernel/mod.rs:49-53`,
`crates/polint/src/analysis_kernel/mod.rs:68-73`,
`crates/polint/src/analysis_kernel/mod.rs:422-450`, and
`crates/polint/src/analysis_kernel/mod.rs:1129-1142`

The semantic and CFG/call pipeline gates currently contain the same capability
set. Therefore, in a selected calls/control-flow/dataflow run,
`calls_ready == false` means `begin_provider("polint.calls")` found a
non-success hard producer and already finalized `polint.calls` as
`DependencyBlocked`. The following `else if run_semantic_pipeline` nevertheless
invokes `derive_call_sites_with_cache_stats`.

That fallback immediately unwraps the semantic-MIR and CFG identities through
`ready_digest!`. A hard-producer failure that blocks calls also leaves one or
both of those identities absent, so the controlled provider failure can become
the production panic `"provider readiness requires identity"` instead of a
complete sealed report. Even if future gate/dependency changes leave those two
digests present, the branch still executes a provider after readiness denied
permission.

**Impact:** This violates the plan-first, no-execution-after-hard-failure
invariant and prevents independent branches from continuing under the exact
failure mode R3 is intended to contain. The current success-path and late
validation tests cannot exercise it because they do not inject an
execution-time upstream non-success.

**Recommended fix:** Separate explicit plan omission from failed readiness.
Execute the lightweight call-site fallback only on a deliberate plan path where
`polint.calls` is not selected; once a selected provider's `begin_provider`
returns blockers, return the default output without invoking either calls
deriver or evaluating any digest. Add a finite test-only upstream failure seam
that proves `polint.calls` becomes dependency-blocked, its telemetry shows no
execution, no readiness `expect` fires, and an independent provider still
succeeds.

### Regression Review

- CR-02's row-sensitive closure matches the actual Events precedence:
  refined rows first, then call-site rows, then syntax-function fallback.
- Planned-absent and rowless enrichment providers remain optional for events.
- Provider outcomes remain manifest ordered, success-identity-only, and
  independent of cache telemetry.
- Validation ownership remains independent of diagnostic rendering.
- Reachability identity sequencing is correct after `28689ca7`.
- Runtime blockers still reach production dispatch before `RuleCtx`.
- Private vocabulary and store-mode byte/exit parity gates remain green.
- No fifteenth product/test file or public/store/CI/tracking change was found.

### Verification

- `cargo test -p polint --lib analysis_kernel::outcome::tests --locked` — 6
  passed.
- `cargo test -p polint --lib analysis_kernel::validation::tests --locked` — 9
  passed.
- `cargo test -p polint --lib analysis_kernel::tests::provider_outcomes --locked`
  — 2 passed.
- `cargo test -p polint --lib core::tests::run_rules_skips_rules_with_runtime_provider_blockers --locked`
  — 1 passed.
- `cargo test -p polint --lib runner::tests::production_dispatch_blocks_events_from_rejected_scheduled_refinement --locked`
  — 1 passed.
- `cargo test -p polint --lib analysis_kernel::tests::events_only_plan_stays_off_semantic_pipeline --locked`
  — 1 passed.
- `cargo test -p polint --lib analysis_kernel::tests::calls_plan_keeps_full_cfg_relation_rows --locked`
  — 1 passed.
- `cargo test -p polint --lib policy_queries::tests::events_matching_call_returns_provider_backed_violation --locked`
  — 1 passed.
- `cargo test -p polint --lib analysis::refined_calls::validate::tests::validation_catches_dangling_call_site --locked`
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
- `git diff --check c453748c..HEAD -- crates/polint` — passed.

No product source, test, fix report, summary, plan/context, tracking file, CI
workflow, commit, branch, or remote state was modified by this re-review. Only
this cumulative review artifact was updated.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: standard_
_Remediation iteration: 2_

---

## R3 Remediation Re-review — Iteration 3 (Final)

### Verdict

Clean. Commit `f86dab67` closes CR-03, and the complete
`c453748c..f86dab67` product/test range re-establishes CR-01, WR-01, and CR-02
as closed. There are zero open critical, warning, or informational findings.

### Scope

Re-reviewed the current contents and cumulative history of the same 14
product/test files, totaling 2,500 insertions and 733 deletions. The review
covered all R3 work and every bounded remediation, not only the iteration-3
commit. No added product/test file, public API expansion, semantic-store schema
change, workflow/CI change, or unrelated tracking change was found.

### Finding Closure

#### CR-01 — Closed

The runtime provider closure for `events` starts from the applicable syntax
providers only: Go syntax is included only when Go files are present and TS
syntax only when TS/JS files are present. The Go-only regression proves that an
irrelevant failed TS provider does not enter the blocker evidence. Production
dispatch forwards the resulting blocker set before constructing `RuleCtx`.

**Evidence:** `crates/polint/src/analysis_kernel/mod.rs:1212-1317`,
`crates/polint/src/analysis_kernel/mod.rs:3308-3347`, and
`crates/polint/src/runner/mod.rs:434-449`

#### WR-01 — Closed

Validation attribution is structured at issue creation. Downgrades consume
only `ValidationIssue::provider_ids`; they do not recover ownership by parsing
rendered diagnostics, reasons, evidence, fact references, or stable keys.
Provider-specific validators assign their owner directly, while deliberately
global checks retain the global fallback. The regression mutates rendered
reason/evidence without changing ownership and separately proves an unowned
issue remains global.

**Evidence:** `crates/polint/src/analysis_kernel/validation.rs:42-149` and
`crates/polint/src/analysis_kernel/validation.rs:5794-5818`

#### CR-02 — Closed

The `events` closure is row-sensitive to both enrichment layers. A non-absent
`polint.calls` outcome influences Events when call-site rows exist, and a
non-absent `polint.refined_calls` outcome influences Events when refined rows
exist. Planned-absent or rowless optional enrichment does not falsely block
syntax fallback. The production-dispatch regression injects a failed
refinement after rows exist, then proves Events and Calls rules do not execute
while an unrelated rule still does.

**Evidence:** `crates/polint/src/analysis_kernel/mod.rs:1235-1297` and
`crates/polint/src/runner/mod.rs:540-610`

#### CR-03 — Closed

A selected Calls run now requires both its plan gate and successful
`begin_provider`. When readiness fails, control reaches the neutral default:
it evaluates no readiness digest and invokes neither call deriver. The
lightweight call-site fallback is guarded by the distinct plan-omission
predicate `!run_cfg_call_pipeline`, so a dependency-blocked selected provider
cannot fall through to it.

The failure seam is private, `#[cfg(test)]`, thread-local, and one-shot. Its
regression forces semantic-MIR execution failure and proves Calls becomes
dependency-blocked with no identity, no call rows, zero telemetry, a complete
manifest-ordered report, and an independent Metrics success. Auditing every
neighboring `begin_provider` branch found no analogous execution after failed
readiness.

**Evidence:** `crates/polint/src/analysis_kernel/mod.rs:76-81`,
`crates/polint/src/analysis_kernel/mod.rs:389-470`, and
`crates/polint/src/analysis_kernel/mod.rs:3265-3304`

### Regression Review

- Outcome sealing remains a fixed-point computation: owned validation failures
  are applied first, then direct hard-dependency blockers propagate to
  convergence without contaminating independent providers.
- Hard-dependency declarations match the provider digests actually consumed.
- Reachability captures its output digest only after projection, preserving
  failure attribution and success identity ordering.
- Reports remain complete and manifest ordered, with identity present only for
  success and cache telemetry kept separate for every provider.
- Cold, warm, and semantic-store modes retain byte/exit parity; maintenance
  observes only sealed outcomes.
- New outcome and validation vocabulary remains private and absent from the
  supported SDK, CLI, generated skill text, and persisted-store surface.
- Deterministic ordering, scope limits, and the exact 14-file change cap remain
  intact.

### Verification

- CR-03 execution-failure regression — 1 passed.
- Provider-outcome regressions — 3 passed.
- Mixed production-dispatch blocker regression — 1 passed.
- Validation ownership regression — 1 passed.
- Outcome tracker tests — 6 passed.
- Validation tests — 9 passed.
- Runtime blocker regression — 1 passed.
- Events-only, Calls-plan, Events-policy, and refined-validation regressions —
  1 passed each.
- Eval performance tests — 6 passed.
- Public-surface leak regression — 1 passed.
- Semantic-store parity regression — 1 passed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p polint --lib --tests --all-features --locked -- -D warnings`
  — passed.
- `cargo check --workspace --all-features --locked` — passed.
- `make lint` — passed, including workspace/all-target strict Clippy.
- `git diff --check c453748c..HEAD -- crates/polint` — passed.
- Kernel/core/runner searches contain neither `ProviderStatsRow` nor the
  retired `ProviderOutputMeta`.

No product source, test, fix report, summary, plan/context, tracking file, CI
workflow, commit, branch, or remote state was modified by this final re-review.
Only this cumulative review artifact was updated.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: standard_
_Remediation iteration: 3 (final)_

## R3 Verification Gap Closure Review

### Verdict

Issues found: **0 critical, 1 warning, 0 info**.

The Plan 65-04 implementation closes D-16 and D-23/D-25, and the production
fixes for CR-01, WR-01, CR-02, and CR-03 remain correct. One warning remains:
test compaction removed the only regression that proved an Events rule is
blocked when its applicable language syntax provider fails.

This review covered the exact product/test diff
`c453748c..080312c4`: 14 files, 2,499 additions, and 822 deletions. It found no
new supported SDK or CLI surface, persisted-store/schema changes, protected
tracking or CI changes, phase-history comments in shipped code, or nondeterministic
ordering additions.

### Gap-closure assessment

#### D-16 — Closed

Validation ownership is now authenticated structured data. `ValidationIssue`
holds presentation separately from attribution, and `from_pending` derives
provider ownership only from a fixed provider id or from a fact's registered
producer/layer metadata. Missing metadata, family-only attribution, ownership
conflicts, and unknown owners fail closed to global blocking. Provider ids are
deduplicated and ordered before downgrade application.

The real FileMetric precision regression proves that only `polint.metrics` is
downgraded. It then mutates the rendered reason, evidence, and stable
fingerprint and proves the downgrade decision is unchanged. The synthetic
global issue remains unattributed and global. This is non-circular evidence
that presentation strings no longer authorize state transitions.

**Evidence:** `crates/polint/src/analysis_kernel/validation.rs:54-118`,
`crates/polint/src/analysis_kernel/validation.rs:127-181`, and
`crates/polint/src/analysis_kernel/validation.rs:5740-5786`

#### D-23/D-25 — Closed

The production cache proof runs the real isolated TypeScript repository through
the actual kernel and `dispatch_kernel_output_rules` path on cold, warm,
corrupt-cache, and blocked-cache-write runs. Its projection includes capability
support, every provider outcome, runtime blockers, kernel and policy
diagnostics, ordered policy answers, per-rule execution decisions, and the real
exit code. Cold and warm cache telemetry differs while all projected behavior
is equal; corrupt reads are evicted and recomputed; blocked writes preserve
behavior and emit the expected warning. The counted rule consumes a public
typed `FileMetrics` view, while an unrelated rule proves independent dispatch.

**Evidence:** `crates/polint/src/runner/mod.rs:552-623` and
`crates/polint/src/runner/mod.rs:659-705`

### Prior finding closure re-check

- **CR-01 remains closed in production.** Events maps to both Go and TypeScript
  syntax providers, then filters that closure to languages actually present in
  the repository.
- **WR-01 remains closed.** Runtime downgrade reads structured provider ids
  only; it does not parse messages, evidence, fingerprints, or display text.
- **CR-02 remains closed.** Calls and refined-calls outcomes affect Events only
  when corresponding rows exist, and a non-absent failed outcome then blocks
  the consumer.
- **CR-03 remains closed.** A selected Calls provider executes only after
  successful readiness; dependency failure cannot fall through to the
  lightweight derivation path. The failure regression still proves no call
  identity or rows are produced while Metrics succeeds independently.

**Evidence:** `crates/polint/src/analysis_kernel/mod.rs:389-470`,
`crates/polint/src/analysis_kernel/mod.rs:1197-1297`, and
`crates/polint/src/runner/mod.rs:626-658`

### Findings

#### WR-02 — Compaction removed the applicable-syntax failure regression

**Severity:** Warning
**Category:** Regression coverage / contract proof

The current Events-only test verifies only that the deep semantic pipeline
stays off. The surviving production-dispatch blocker test injects failure into
`polint.refined_calls`, after refined rows exist. Neither test makes an
applicable Go or TypeScript syntax provider fail, and the latter is the only
test that calls `runtime_capability_blockers`.

The prior dedicated regression removed during compaction used a single-language
repository, forced syntax outcomes to non-success states, and asserted that the
applicable syntax provider blocked Events while the irrelevant-language
provider did not appear in blocker evidence. Consequently, deleting or
mis-filtering the production mapping
`"events" => ["polint.go.syntax", "polint.ts.syntax"]` would no longer make the
focused suite fail, even though that would reopen CR-01.

**Evidence:**

- `crates/polint/src/analysis_kernel/mod.rs:1197-1297` contains the production
  closure and language filter.
- `crates/polint/src/analysis_kernel/mod.rs:2330` exercises an Events-only plan
  without a syntax failure.
- `crates/polint/src/runner/mod.rs:626-658` exercises a rejected
  refined-calls provider, not a syntax provider.
- A source search finds no other test invocation of
  `runtime_capability_blockers`.
- The removed test is visible in `bbdedff7..080312c4` as
  `established_run_seals_static_manifest_order_after_validation`.

**Impact:** A future regression in applicable-language syntax closure could
silently pass the compacted suite and restore policy execution after a required
syntax provider failed. This weakens proof for a previously critical runtime
trust-boundary fix.

**Minimal fix:** Restore or merge a compact single-language production-dispatch
regression that changes the applicable syntax outcome to `Failed`, proves the
irrelevant language provider is absent from blocker evidence, proves the Events
rule does not execute, and proves an unrelated rule still executes. Preserve
the exact 14-file scope and the 2,500-addition cap; the current diff has one
addition of headroom, so the proof must replace or compact superseded test
lines rather than simply append them.

### Verification

All 15 Plan 65-04 commands passed:

- Validation, outcome, provider-outcome, production cache projection, mixed
  dispatch, core blocker, public-surface, and semantic-store parity tests.
- `cargo fmt --all -- --check`.
- Workspace all-target/all-feature strict Clippy.
- Workspace all-feature check.
- Diff hygiene, exact 14-file set, 2,499-addition cap, and protected-file
  audits.

Additional focused suites passed for run reports, stats, eval performance,
Events-only planning, full-CFG Calls planning, provider-backed Events matching,
refined-call validation, the Go semantic provider, symbol-cache restore, and
semantic-graph snapshots.

No product source, test, plan, summary, verification, fix report, tracking
file, CI workflow, commit, branch, or remote state was modified by this review.
Only this cumulative review artifact was updated, with the preceding review
body retained verbatim.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: deep_
_Remediation iteration: 4 (verification-gap closure)_

## R3 Verification Gap Closure Re-review

### Verdict

Issues found: **0 critical, 1 warning, 0 info**.

The WR-02 remediation restores the applicable-syntax failure proof without
weakening the refined-calls, validation-ownership, cold/warm, cache-corruption,
cache-write-warning, or public-surface proofs. D-16 and D-23/D-25 remain
closed, and the production fixes for CR-01, WR-01, CR-02, and CR-03 remain
correct. One new warning prevents a clean result: the structured
`ValidationReport` wrapper makes a pre-existing validation-diagnostic
non-leakage test fail when that test debug-formats the wrapper rather than its
diagnostic slice.

This re-review covered the full current product/test diff
`c453748c..adc40fc3`: exactly 14 files, 2,500 additions, and 830 deletions. It
found no fifteenth file, supported SDK/CLI/config/output expansion,
persisted-store/schema change, protected tracking or CI change, phase-history
comment in shipped code, or new nondeterministic ordering.

### Independent closure assessment

- **D-16 remains closed.** `ValidationIssue::from_pending` takes only explicit
  `FactFamily`, `FactRef`, fixed provider context, and manifest-authenticated
  `FactMeta` producer/layer IDs. `BTreeSet` supplies sorted/deduplicated
  ownership; missing, family-only, unknown, or unauthenticated ownership
  remains global. Message, evidence, fingerprint, fact-reference display, and
  other rendered strings do not authorize ownership or downgrades. Rendering
  remains a one-way reconstruction of the original Diagnostic fields.
- **D-23/D-25 remain closed.** The representative isolated TypeScript cold/warm
  pair calls the actual `dispatch_kernel_output_rules` and `exit_code_for`.
  Its equality projection contains capability support, complete
  manifest-ordered outcomes with identities and blockers, runtime blockers,
  sorted kernel/policy/combined diagnostics, ordered typed `FileMetrics`
  answers, per-rule decision deltas, and the exit byte. Provider telemetry is
  explicitly unequal. Real cache corruption still causes invalid eviction and
  recomputation; blocked writes preserve outcomes and emit the existing cache
  warning.
- **CR-01 and WR-02 are closed.** A Go-only real kernel run forces both syntax
  outcomes to validation failure, invokes actual
  `runtime_capability_blockers` and production dispatch, and requires exact
  blocker evidence `polint.go.syntax`; the Events counter stays zero and the
  unrelated counter reaches one. Deleting the Events-to-syntax mapping leaves
  no diagnostic, while bypassing language filtering yields both syntax IDs, so
  either mutation breaks the regression.
- **WR-01 remains closed.** Validation downgrade ownership reads structured
  issue fields only; diagnostic presentation is not parsed.
- **CR-02 remains closed.** The adjacent production regression still makes a
  scheduled rejected `polint.refined_calls` outcome block both Events and
  Calls, while the unrelated rule runs.
- **CR-03 remains closed.** A selected Calls provider can execute only after
  successful readiness; dependency blocking cannot enter the lightweight
  derivation branch or evaluate absent identities. The execution-failure
  regression keeps Calls identity/facts absent while Metrics succeeds.
- **Fixed-point, failure containment, deterministic ordering, private
  vocabulary, store/output parity, and semantic/telemetry separation remain
  intact.** No proof removed by the WR-02 compaction was weakened: the compact
  provider-identity helper now centrally asserts non-empty success digests,
  and the prior cache and runner projections retain their semantic fields.

**Evidence:** `crates/polint/src/analysis_kernel/validation.rs:53-125`,
`crates/polint/src/analysis_kernel/mod.rs:1197-1293`, and
`crates/polint/src/runner/mod.rs:595-729`

### Findings

#### WR-03 — Structured report Debug output breaks an existing diagnostic non-leakage test

**Severity:** Warning
**Category:** Regression / validation contract test

`ValidationReport` derives `Debug` and contains both the rendered diagnostics
and private structured `ValidationIssue` rows. The pre-existing
`type_value_alias_validation_reports_malformed_rows_deterministically` test
still binds the return value as `diagnostics` and debug-formats the whole
value. Its private issues now render
`provider_ids: ["polint.type_value_alias"]`, so the assertion that validation
diagnostics do not contain private provider/type vocabulary fails.

The supported Diagnostic projection itself remains generic: iteration through
the report dereferences to `[Diagnostic]`, the focused public-surface leak test
passes, and no supported JSON/CLI output was shown to contain the private ID.
The defect is nevertheless material because a directly affected pre-existing
unit test and the broader validation module suite are red.

**Evidence:**

- `crates/polint/src/analysis_kernel/validation.rs:103-106` derives `Debug` for
  the wrapper containing private issues.
- `crates/polint/src/analysis_kernel/validation.rs:1639-1658` formats the
  wrapper and applies the Diagnostic non-leakage marker assertions.
- `cargo test -p polint --lib analysis_kernel::validation --locked` runs 28
  tests: 27 pass and this test fails at line 1656 after finding
  `polint.type_value_alias` in the wrapper's private `provider_ids`.

**Impact:** The cumulative R3 slice cannot claim a clean regression suite, and
a broader library test run would fail even though the supported diagnostic
bytes remain private-vocabulary-free.

**Minimal fix:** Keep the non-leakage assertion scoped to the rendered
Diagnostic slice, for example by debug-formatting `&*diagnostics`, or give the
wrapper an intentional Debug implementation that does not conflate private
ownership state with rendered diagnostics. Re-run the full
`analysis_kernel::validation` module plus the Plan 65-04 matrix and public leak
gate.

### Verification

All 15 Plan 65-04 commands passed separately:

- Validation 8/8, outcome 6/6, execution-failure outcome 1/1, cold/warm
  production projection 1/1, rejected-refinement dispatch 1/1, core runtime
  blocker 1/1, public-surface leak 1/1, and semantic-store parity 1/1.
- Formatting, workspace all-target/all-feature strict Clippy, and workspace
  all-feature checking.
- Diff hygiene, exact 14-file scope, the 2,500-addition cap, and protected-file
  audits.

The restored applicable-syntax production-dispatch test passed 1/1. Additional
run-report 5/5, telemetry 3/3, eval-performance 6/6, Events-only planning 1/1,
full-CFG Calls planning 1/1, provider-backed Events matching 1/1, refined-call
validation 4/4, Go semantic provider 6/6, symbol-cache restore 2/2, and
semantic-graph snapshot 9/9 suites passed. The broader validation suite found
WR-03 as described above.

No product source, test, plan, summary, verification, fix report, tracking
file, CI workflow, commit, branch, or remote state was modified by this
re-review. Only this cumulative review artifact was updated, with all preceding
review body/history retained verbatim.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: deep_
_Re-review iteration: 2 (verification-gap closure)_

## R3 Verification Gap Closure Final Re-review

### Verdict

Clean. The WR-03 remediation in `e8a1f800` makes the existing non-leak
regression inspect the dereferenced rendered `[Diagnostic]` value, and the
complete cumulative `c453748c..d926c901` product/test range has zero critical,
warning, or informational findings. The prior fixes for CR-01, WR-01, CR-02,
CR-03, and WR-02 remain correct.

This verdict is limited to the R3 provider-truth slice and its verification-gap
closure. Phase 65, R4-R6, STORE-04, STORE-05, META-01, and META-04 remain open;
the deferred sub-five-minute CI work is not part of this review.

### Scope and preservation

- Reviewed all current code in the exact fourteen-file Plan 65-03 boundary
  across `c453748c..d926c901`, not only the WR-02 and WR-03 fix commits.
- The cumulative product/test diff is exactly `+2500/-831`; no fifteenth file
  was introduced.
- No public API, SDK, CLI, configuration, output, diagnostic-byte, exit-code,
  durable-store, schema, migration, provider-persistence, CI, roadmap,
  requirements, or state-tracking surface changed.
- All preceding review body/history was retained verbatim. This section is an
  append-only final re-review, apart from the required frontmatter status,
  iteration, and finding-count update.

### Decision and finding closure

- **D-16 and WR-01 are closed.** Validation ownership originates from explicit
  `Provider`, `FactFamily`, or `FactRef` context. Fact-backed producer/layer
  IDs are accepted only when authenticated against the static manifest
  inventory, and `BTreeSet` collection makes ownership ordered and
  deduplicated. Missing, unknown, unauthenticated, or otherwise non-narrowable
  ownership retains an empty provider set and therefore the global fail-closed
  downgrade. `ValidationReport::downgrades` consumes only this private
  structured state; rendered messages, evidence, fingerprints, stable keys,
  labels, and fact-ref display strings are never parsed as trust inputs.
- **D-23 and D-25 are closed.** One real isolated TypeScript repository is run
  cold and warm through the production kernel, production
  `dispatch_kernel_output_rules`, and production `exit_code_for`. Its compared
  projection includes capability support, full manifest-ordered outcomes,
  success identities, exact blockers, runtime-blocked rules, sorted kernel and
  policy diagnostics, typed policy answers and order, combined diagnostics,
  per-rule execution decisions, and exit byte. The semantic projections are
  equal while provider telemetry is unequal, proving an actual cache-boundary
  crossing. Corrupting a real cached blob still proves invalid eviction plus
  recomputation before success, and a blocked cache write preserves the same
  outcomes while emitting the existing warning.
- **CR-01 and WR-02 are closed.** The restored production regression uses a
  real Go-only kernel database, forces both Go and TypeScript syntax outcomes
  to fail, derives actual runtime capability blockers, and sends them through
  production dispatch. The Events rule executes zero times, the unrelated
  rule executes once, and blocker evidence names exactly
  `polint.go.syntax`. The assertion is mutation-sensitive: removing Events'
  syntax mapping or ignoring language applicability breaks it.
- **CR-02 remains closed.** Scheduled Calls and Refined Calls outcomes affect
  Events only when their rows can participate in the actual Events precedence
  path. The retained production regression rejects scheduled refinement,
  blocks both affected rules, and permits the unrelated rule.
- **CR-03 remains closed.** Calls runs only after
  `ProviderOutcomeTracker::can_run` succeeds. A dependency-blocked Calls
  provider cannot enter its fallback, construct a ready digest, or panic; the
  execution-failure regression proves exact blockers, no output identity, no
  call rows, and independent Metrics success.
- **Fixed-point closure, failure containment, determinism, cache identity, and
  store parity remain intact.** Validation rejection closes downstream
  provisional successes in manifest order until a fixed point, while
  independent branches remain usable. Typed provider failures are consumed
  before success is recorded, non-success states carry no identity, reports
  remain complete and manifest ordered, cache telemetry remains semantically
  separate, and all store modes retain byte-identical JSON and exit behavior.
- **WR-03 is closed.** The non-leak regression now explicitly formats
  `&*diagnostics`, so it examines only the rendered diagnostic slice and no
  longer mistakes private ownership state for public output. Retaining derived
  `Debug` on `ValidationReport` and `ValidationIssue` is acceptable: both types
  are private, production consumers only dereference rendered diagnostics or
  read structured downgrades, and neither value is serialized, emitted, or
  exported. The exact test, the complete 28-test validation module, and the
  supported-public-surface leak gate all pass.
- **Compaction preserved prior proof.** Shared outcome/identity helpers still
  assert successful non-empty identities, cold/warm identity equality remains
  explicit, the refined-call blocker proof still checks both blocked rules,
  and corrupt-cache, write-warning, typed-answer, ordering, and exit assertions
  remain active.

### Verification

All 15 Plan 65-04 commands passed separately:

- Validation 8/8, outcome tracker 6/6, execution-failure outcome 1/1,
  cold/warm production projection 1/1, rejected-refinement dispatch 1/1, core
  runtime blocker 1/1, public-surface leak 1/1, and semantic-store parity 1/1.
- `cargo fmt --all -- --check`.
- Workspace all-target/all-feature strict Clippy with `-D warnings`.
- Workspace all-feature checking.
- Diff hygiene, the exact fourteen-file set, the 2,500-addition cap, and the
  protected CI/tracking-file audit.

The restored Go-only applicable-syntax test passed 1/1. The exact WR-03
rendered-diagnostic test passed 1/1, and the complete
`analysis_kernel::validation` module passed 28/28. Additional run-report 5/5,
telemetry 3/3, eval-performance 6/6, Events-only planning 1/1, full-CFG Calls
planning 1/1, provider-backed Events matching 1/1, refined-call validation
4/4, Go semantic provider 6/6, symbol-cache restore 2/2, and semantic-graph
snapshot 9/9 suites passed. Every focused test completed well below sixty
seconds.

No product source, test, plan, summary, verification, fix report, tracking
file, CI workflow, commit, branch, or remote state was modified by this final
re-review. Only this cumulative review artifact was updated.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: deep_
_Final re-review iteration: 3 (verification-gap closure)_
