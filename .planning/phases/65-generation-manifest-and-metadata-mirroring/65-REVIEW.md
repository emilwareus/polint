---
phase: 65-generation-manifest-and-metadata-mirroring
scope: r1-r3
retained_history: "R1-R2 review body preserved verbatim"
depth: standard
status: clean
iteration: 6
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
