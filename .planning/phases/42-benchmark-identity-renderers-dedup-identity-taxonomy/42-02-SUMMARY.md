---
phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy
plan: 02
subsystem: analysis
tags: [identity, renderers, go-relstring, jelly-span, crlf-normalization, eval, jelly-oracle-coverage]

# Dependency graph
requires:
  - phase: 42-01
    provides: analysis::identity::facts (IdentityRecord, IdentityKind, LanguageTag, SignatureDigest), db.identity_records(), polint.identity provider, cache-key trip-wires go_relstring_v1 / jelly_span_v1
provides:
  - analysis::identity::render::go_relstring::render(&IdentityRecord) -> String (Go RelString format)
  - analysis::identity::render::jelly_span::render(&IdentityRecord, &SourceFile) -> String (Jelly span with CRLF normalization at render time)
  - eval adapters (jelly_callgraph, go_x_tools_callgraph) consume the renderers as the single source of truth; inline jelly_span_identity deleted
  - eval::report::JellyOracleCoverageSection { matched, total, ratio, unmatched } + JellyUnmatchedSpan { file, span, reason }
  - eval::report::MetricSections.jelly_oracle_coverage (#[serde(default)]) wired into the metrics-build path
  - tests/eval-fixtures/identity/crlf_normalization (LF + CRLF repos, byte-identical renderer proof)
  - tests/eval-fixtures/identity/jelly_oracle_coverage (>=0.99 ratio fixture)
affects: [42-03-identity-taxonomy, 42-04-public-surface-leak-gate, 43-reachability-roots, v1.3-semantic-graph]

# Tech tracking
tech-stack:
  added: []  # No new third-party deps (T-42-02-SC)
  patterns:
    - "Pure pub(crate) renderers over &IdentityRecord (+ &SourceFile for Jelly); no kernel handle (D-06)"
    - "CRLF->LF normalization at render time via a single linear pass that re-derives 1-based line/1-based col/half-open end col from byte offsets (D-12)"
    - "Renderer shape encoding in container_path: `$N` -> anonymous parent$N; `*Recv.Method`/`Recv.Method` -> pointer/value method; `Base[Args]` on display_name -> generic instantiation"
    - "Jelly oracle coverage as deterministic matched/total over distinct oracle endpoint spans vs renderer-produced observed spans (D-20)"

key-files:
  created:
    - crates/polint/src/analysis/identity/render/mod.rs
    - crates/polint/src/analysis/identity/render/go_relstring.rs
    - crates/polint/src/analysis/identity/render/jelly_span.rs
    - tests/eval-fixtures/identity/crlf_normalization/expected.polint-eval.toml
    - tests/eval-fixtures/identity/crlf_normalization/repo-lf/.polint.toml
    - tests/eval-fixtures/identity/crlf_normalization/repo-lf/src/foo.ts
    - tests/eval-fixtures/identity/crlf_normalization/repo-crlf/.polint.toml
    - tests/eval-fixtures/identity/crlf_normalization/repo-crlf/.gitattributes
    - tests/eval-fixtures/identity/crlf_normalization/repo-crlf/src/foo.ts
    - tests/eval-fixtures/identity/jelly_oracle_coverage/expected.polint-eval.toml
    - tests/eval-fixtures/identity/jelly_oracle_coverage/repo/tests/micro/app.js
    - tests/eval-fixtures/identity/jelly_oracle_coverage/repo/tests/micro/app.json
  modified:
    - crates/polint/src/analysis/identity/mod.rs
    - crates/polint/src/eval/external/jelly_callgraph.rs
    - crates/polint/src/eval/external/go_x_tools_callgraph.rs
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/runner.rs

key-decisions:
  - "Renderer shape is driven by container_path encoding because Plan 01 populates container_path from the function name; the renderer interprets that shape rather than requiring new fact fields"
  - "Jelly adapter uses an inline output.db borrow (not an extracted helper) for the per-edge identity lookup; the records slice + files map are read directly inside normalize_kernel_output (WARNING #3 option B)"
  - "Go RTA adapter routes endpoints through go_relstring::render (exercising the single source of truth) then projects to the bare RTA WANT: oracle key via display_name with main. stripped — the RelString feeds the Plan 03 taxonomy, the oracle key stays bare"
  - "CRLF==LF byte-identical equality is proven by a dedicated runner test loading both repos; the eval fixture loads only the LF repo (one fixture = one repo) and pins render invariants as observed"
  - "Jelly oracle coverage fixture is driven through the real JellyCallgraphAdapter; oracle JSON files array uses the workspace-relative path so it aligns with the renderer's workspace-relative output"
  - "jelly_span_file extracts the file portion via rsplitn(5, ':') so unmatched-span file fields stay correct even when the path contains colons"

patterns-established:
  - "Render module: pub(crate) mod with no pub use lifting; call sites use explicit render::{go_relstring,jelly_span}::render paths"
  - "MetricSections extension is additive via #[serde(default)] field; MetricSummary shape stays frozen and is locked by a destructure test"
  - "Coverage section ratio = matched/total, or 1.0 when total == 0 (empty oracle is vacuously covered)"

requirements-completed: [IDENT-02]

# Metrics
duration: 1h 5m
completed: 2026-05-29
---

# Phase 42 Plan 02: Benchmark Identity Renderers Summary

**Go RelString and Jelly span renderers as pure pub(crate) functions over `&IdentityRecord`, CRLF-at-render-time normalization with a byte-identical CRLF/LF fixture, single-source-of-truth wiring into both eval adapters, and a `jelly_oracle_coverage` MetricSections section proving >=99% oracle coverage.**

## Performance

- **Duration:** ~1h 5m
- **Started:** 2026-05-29 (Plan 02 execution)
- **Completed:** 2026-05-29
- **Tasks:** 3
- **Files modified:** 20 (12 created, 8 modified)

## Accomplishments

- `analysis::identity::render` module with two pure `pub(crate)` renderers:
  - `go_relstring::render(identity: &IdentityRecord) -> String` — package function (`module/path/pkg.Foo`), pointer-receiver method (`(*module/path/pkg.Receiver).Method`), value-receiver method (`(module/path/pkg.Receiver).Method`), generic instantiation (`Func[T0,T1]`), anonymous `package.parent$N` with deterministic 1-based ordinal (D-07).
  - `jelly_span::render(identity: &IdentityRecord, source: &SourceFile) -> String` — `file:start_line:start_col:end_line:end_col` with 1-based line, 1-based column, half-open end column, CRLF normalized at render time (D-08, D-12).
- CRLF normalization happens only inside the Jelly renderer (D-12); on-disk byte spans and `SourceFile.source` are untouched. A multi-line CRLF/LF fixture pair (with `.gitattributes` `* -text` preserving literal `0d 0a` bytes through git) proves byte-identical output (D-13, D-25, T-42-02-01).
- Both eval adapters consume the renderers as the single source of truth (D-05); the `#[cfg(test)] fn jelly_span_identity` inline formatter is deleted.
- `eval::report::MetricSections` gains `jelly_oracle_coverage` (four-field shape) wired into the metrics-build path; the existing `MetricSummary` shape is unchanged and locked by a destructure test.
- Jelly oracle coverage micro fixture asserts `ratio >= 0.99` on the host platform (D-20, D-22).

## Renderer Signatures (for Plan 03 categorize + Plan 04 leak gate)

```rust
pub(crate) fn go_relstring::render(identity: &IdentityRecord) -> String
pub(crate) fn jelly_span::render(identity: &IdentityRecord, source: &SourceFile) -> String
```

Both are `pub(crate)`; the leak gate (Plan 04) verifies they never reach `polint::sdk::prelude`.
`grep -rn "pub fn render" crates/polint/src/analysis/identity/render/ | grep -v "pub(crate)"` returns 0.

## normalize_kernel_output wiring (WARNING #3)

Chosen option B (inline `output.db` borrow). Inside `jelly_callgraph.rs::normalize_kernel_output`:
- `let records = output.db.identity_records();` and a `files: BTreeMap<FileId, &SourceFile>` map are read inline.
- A per-edge `render_span(span)` closure finds the `IdentityRecord` whose `span == *span` (`records.iter().find(...)`) and calls `jelly_span::render(record, source)`; an edge whose span has no matching record (or missing source file) drops via the existing `else { continue; }` short-circuit.
- `Span` does not implement `Ord`/`Hash`, so the lookup uses `.iter().find(|r| r.span == *span)` rather than a map keyed by span (the plan's suggested approach).
- The Go adapter routes endpoints through `go_relstring::render` via `go_rta_oracle_identity`, then projects to the bare RTA `WANT:` oracle key.

## Jelly oracle coverage fixture + achieved ratio

- Fixture path: `tests/eval-fixtures/identity/jelly_oracle_coverage/` (oracle JSON at `repo/tests/micro/app.json`, source at `repo/tests/micro/app.js`).
- Runner test `identity_jelly_oracle_coverage_fixture` drives the real `JellyCallgraphAdapter`; achieved **ratio = 1.0 (3/3 oracle spans matched)** on the executor host (macOS). Threshold asserted `>= 0.99` per-platform (D-22, no cross-platform averaging).
- The oracle JSON `files` array uses the workspace-relative path `tests/micro/app.js` so the oracle endpoint span strings align with the renderer's workspace-relative output.

## CRLF fixture + .gitattributes

- Fixture path: `tests/eval-fixtures/identity/crlf_normalization/`.
- `repo-lf/src/foo.ts` (`\n`) and `repo-crlf/src/foo.ts` (`\r\n`) hold identical logical 6-line TS source.
- `repo-crlf/.gitattributes` content: `* -text` — disables git line-ending normalization so the CRLF file is committed with literal `0d 0a` bytes (verified via `git show :...repo-crlf/src/foo.ts | xxd`).
- Runner test `identity_crlf_normalization_fixture` loads both repos and asserts each LF identity record's rendered Jelly span is byte-identical to its CRLF counterpart.

## JellyOracleCoverageSection / JellyUnmatchedSpan field set (for Plan 03 sibling-placement)

```rust
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct JellyUnmatchedSpan { pub(crate) file: String, pub(crate) span: String, pub(crate) reason: String }

#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct JellyOracleCoverageSection {
    pub(crate) matched: u32, pub(crate) total: u32, pub(crate) ratio: f64,
    pub(crate) unmatched: Vec<JellyUnmatchedSpan>,
}
```

`jelly_oracle_coverage` is the last field of `MetricSections` (after `adaptation`), `#[serde(default)]`.
Plan 03's `categorized_failures` should sibling next to it on `MetricSections`.

## MetricSummary shape — UNCHANGED (downstream gates lock it)

Confirmed: the `MetricSummary` field set in `crates/polint/src/eval/report.rs` is exactly the locked list (`true_positives` … `sections`) — no fields added or removed. The destructure test `metric_summary_layout_unchanged` fails to compile if the shape changes. All Phase 42 reporting extension lives on `MetricSections` only.

## eval/metrics.rs population location (for Plan 03 categorized_failures sibling)

`jelly_oracle_coverage` is populated in three places, all reading the per-case expected/observed items:
- `crates/polint/src/eval/runner.rs` build paths (~line 113 and ~line 220): `metrics.sections.jelly_oracle_coverage = crate::eval::metrics::jelly_oracle_coverage_for_cases(&case_results);`
- `crates/polint/src/eval/fixtures.rs::evaluation_run_for_fixture` (~line 916): `metrics.sections.jelly_oracle_coverage = crate::eval::metrics::jelly_oracle_coverage(&fixture.manifest.expected, &observed);`
- The computation functions `jelly_oracle_coverage(expected, observed)` and `jelly_oracle_coverage_for_cases(cases)` live in `crates/polint/src/eval/metrics.rs` immediately before `status_label` (≈line 250). Plan 03's `categorized_failures` wiring should sibling next to these calls.

## Task Commits

1. **Task 1: Go RelString + Jelly span renderers + CRLF fixture** - `361f223` (feat) — render module, both renderers with co-located tests, CRLF/LF fixture, render invariants in eval observation.
2. **Task 2: Wire eval adapters into renderers; delete inline rendering** - `c04b188` (refactor) — jelly_callgraph + go_x_tools_callgraph consume the renderers; `jelly_span_identity` deleted.
3. **Task 3: JellyOracleCoverageSection + metrics wiring + coverage fixture** - `04c9a70` (feat) — report struct, metrics-build path, oracle coverage fixture, serde/reverse-compat/layout-lock tests.

_Note: renderers and their unit tests are co-located in one file, so each TDD task landed as a single commit rather than separate test/feat commits._

## Decisions Made

See `key-decisions` in frontmatter. In brief:
- Renderer shape is driven by `container_path` encoding (Plan 01 populates it from the function name).
- Inline `output.db` borrow for the Jelly per-edge identity lookup; `Span` lacks `Ord`/`Hash` so `.iter().find()` is used.
- Go RTA adapter exercises `go_relstring::render` (single source of truth) but projects to the bare RTA oracle key.
- CRLF==LF byte-identical equality proven by a dedicated runner test (one eval fixture loads one repo).
- Oracle JSON `files` uses the workspace-relative path so it aligns with renderer output.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Jelly per-edge identity lookup uses `.iter().find()` instead of a span-keyed map**
- **Found during:** Task 2 (jelly adapter wiring)
- **Issue:** The plan suggested keying a map by span (`records_by_span`), but `core::Span` derives neither `Ord` nor `Hash`, so it cannot be a `BTreeMap`/`HashMap` key.
- **Fix:** Used `records.iter().find(|record| record.span == *span)` (the plan's alternative phrasing) — O(records) per edge, deterministic, no key trait requirement.
- **Files modified:** crates/polint/src/eval/external/jelly_callgraph.rs
- **Verification:** `cargo test -p polint eval::external::jelly_callgraph` passes (7/7); clippy clean.
- **Committed in:** c04b188 (Task 2)

**2. [Rule 1 - Bug] Jelly oracle coverage fixture path alignment**
- **Found during:** Task 3 (oracle coverage fixture)
- **Issue:** The renderer emits workspace-relative paths (`tests/micro/app.js`), while a `case_dir`-relative oracle `files` array (`app.js`) produced zero matches (matched 0/3). The old inline formatter stripped `case_dir`; the renderer (correctly, per D-08/D-25) does not.
- **Fix:** Authored the oracle JSON `files` array with the workspace-relative path `tests/micro/app.js` so oracle endpoint spans align with the renderer's output. `entries` stays `app.js` (resolved relative to the JSON dir for source loading).
- **Files modified:** tests/eval-fixtures/identity/jelly_oracle_coverage/repo/tests/micro/app.json
- **Verification:** `identity_jelly_oracle_coverage_fixture` passes with ratio 1.0 (3/3).
- **Committed in:** 04c9a70 (Task 3)

**3. [Rule 2 - Missing Critical] Render invariants added to eval observation**
- **Found during:** Task 1 (CRLF fixture wiring)
- **Issue:** A native eval fixture loads exactly one repo, so the CRLF fixture had nothing genuinely observable to assert (the byte-identical comparison needs two repos). Without an observed value the fixture would assert a phantom.
- **Fix:** Added `identity_render_invariants` to `eval::observed::identity_invariants`: renders every identity record through both renderers and emits `identity.render.jelly.no_absolute_path` (T-42-02-02) and `identity.render.jelly.rendered_count.nonzero`. The CRLF fixture asserts these on the LF repo; the byte-identical CRLF==LF equality is proven by a dedicated runner test.
- **Files modified:** crates/polint/src/eval/observed.rs, crates/polint/src/eval/runner.rs
- **Verification:** `identity_crlf_normalization_fixture` + `identity_crlf_normalization_fixture_observed_through_eval_harness` pass.
- **Committed in:** 361f223 (Task 1)

---

**Total deviations:** 3 auto-fixed (1 blocking, 1 bug, 1 missing-critical)
**Impact on plan:** All deviations preserve plan intent and the D-05/D-08/D-12/D-20/D-25 contracts. No new third-party deps (T-42-02-SC). No scope creep — the renderer remains the single source of truth and CRLF normalization stays at render time.

## Issues Encountered

- **Jelly oracle coverage matched 0/3 on first run:** The renderer's workspace-relative path (`tests/micro/app.js`) did not match the oracle's `case_dir`-relative file name (`app.js`). Resolved by aligning the oracle JSON `files` array to the workspace-relative path (deviation 2). This is the expected consequence of the renderer no longer taking a `case_dir` parameter (D-06 purity) — the eval edge-key alignment is the fixture's responsibility, not the renderer's.

## Threat Flags

None — no new network endpoints, auth paths, file-access patterns, or schema changes at trust boundaries were introduced. The renderer is read-only over `SourceFile` (string-only path operations, no canonicalize/symlink dereference) and produces deterministic byte-identical output; the threat register dispositions T-42-02-01 (CRLF byte-identical) and T-42-02-02 (no absolute-path leak) are covered by the `crlf_and_lf_produce_byte_identical_output` and `no_absolute_host_path_substrings` tests plus the render invariants.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- IDENT-02 status (reconciled by Plan 05 — the original wording here overstated the Go half): the Jelly half is fully delivered end-to-end — the Jelly span renderer is a pure `pub(crate)` function consumed by the eval adapter as the single source of truth, CRLF normalization is at render time with a byte-identical fixture, `MetricSections.jelly_oracle_coverage` is populated, and the oracle coverage fixture asserts >=0.99 (>=99% coverage). The Go RelString renderer now produces package-NAME-qualified output (`pkg.Func`) on real records (closed by Plan 05: the provider resolves the Go `PackageFact` name, proven by a provider-level test). However the FULL Go module import-path RelString (`module/path/pkg.Func`) and its consumption in the Go RTA oracle scoring path are deferred to Phase 46 (Go Semantic Frontend & Sidecar), which supplies the package/import path the v1.2 substrate lacks; the Go RTA oracle key intentionally stays on `display_name` until then.
- Plan 03 (identity taxonomy) can sibling `categorized_failures` next to `jelly_oracle_coverage` on `MetricSections` (population sites listed above) and reference the renderer signatures above without re-exploring.
- Plan 04 (public-surface-leak gate) can rely on the renderer functions being `pub(crate)` (no-leak smoke returns 0).
- The cache-key trip-wires `go_relstring_v2` / `jelly_span_v1` must be bumped if either renderer's logic changes (the Go trip-wire was bumped `v1` -> `v2` in Plan 05 when the provider switched Go `package_or_module` from the file path to the package name).

## Self-Check: PASSED

---
*Phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy*
*Completed: 2026-05-29*
