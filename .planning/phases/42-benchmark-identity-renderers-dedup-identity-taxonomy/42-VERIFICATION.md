---
phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy
verified: 2026-05-29T00:00:00Z
status: gaps_found
score: 4/5 roadmap success criteria verified (IDENT-02 partial)
overrides_applied: 0
re_verification:
gaps:
  - truth: "Per-benchmark renderers produce Go RelString-style names with benchmark-grade fidelity on real Go records (IDENT-02, D-07, SC2)"
    status: partial
    reason: >-
      The Go RelString renderer is format-correct for synthetic records but is fed the
      WRONG `package_or_module` on real records and its output is never consumed. The
      provider sets `package_or_module = db.path_for(file)` (the workspace-relative FILE
      PATH, e.g. `src/main.go`), not the Go import/package path. `go_relstring::render`
      then produces `src/main.go.Foo` instead of the `module/path/pkg.Foo` form D-07
      promises. Every unit test passes only because it hand-builds records with a correct
      package string; no test exercises a record built by the real provider. Separately,
      EVERY call site of `go_relstring::render` in the crate discards the output
      (`let _ = ...` / `let _rel_string = ...`) — the Go RTA oracle key is derived from
      `display_name` with `main.` stripped, not from the RelString. So the Go RelString
      renderer is neither correct on real data nor consumed by any scoring path. The Jelly
      half of IDENT-02 (the ≥99% coverage half) IS genuinely delivered end-to-end and
      passes; this gap is the Go RelString half only.
    artifacts:
      - path: "crates/polint/src/analysis/identity/provider.rs"
        issue: "Lines 95, 112, 197-199: `package_or_module_for_file` returns `db.path_for(file)` (file path) for Go records instead of the Go import/package path. PackageFact and module-graph package nodes exist in the substrate but are not joined."
      - path: "crates/polint/src/eval/external/go_x_tools_callgraph.rs"
        issue: "Line 151 (`#[cfg(test)]`): `let _rel_string = ...go_relstring::render(record);` — output discarded; oracle key is `go_x_tools_function_identity(record.display_name)` (line 152)."
      - path: "crates/polint/src/eval/observed.rs"
        issue: "Line 670 (`#[cfg(test)]`): `let _ = go_relstring::render(record);` — output discarded (panic-only smoke)."
    missing:
      - "Populate `package_or_module` with the Go package/import path (resolvable from PackageFact / module-graph package nodes) for Language::Go records, keeping the file path only where the language has no package concept."
      - "Add a provider-level test that builds an IdentityRecord from a real Go FunctionFact and asserts `go_relstring::render` yields `pkg.Func` form (not the vacuously-passing synthetic-record tests)."
      - "Either consume the RelString in a scoring/oracle path or document that the Go RelString renderer is dormant scaffolding until its real consumer (Phase 46 Go semantic frontend) lands."
deferred:
  - truth: "Go RelString renderer fed a real Go package/import path and consumed by a scoring path"
    addressed_in: "Phase 46"
    evidence: >-
      Phase 46 (Go Semantic Frontend & Sidecar) success criteria: `src/go/semantic/`
      (sidecar client + lowering) maps go/packages + go/ssa NDJSON facts — including
      receiver types and method sets — to semantic-graph constraints with "stable
      identities and exact source spans". This is the substrate that supplies the real
      Go package/import path the Phase 42 provider currently lacks (FunctionFact carries
      no package; the v1.2 substrate Phase 42 composes over has only file paths).
  - truth: "≥99% Jelly oracle-span coverage demonstrated across the FULL JS/TS Jelly fixture set (not a single micro fixture)"
    addressed_in: "Phase 45"
    evidence: >-
      Phase 45 (JS/TS Inventory) success criteria: polint enumerates JS/TS functions and
      callsites "with Jelly-shaped spans matching ≥99% of Jelly fixture oracle spans" —
      the broad, multi-fixture Jelly oracle-span coverage that CR-02/CR-06 flag as guarded
      by only one Phase 42 fixture is an explicit Phase 45 deliverable over the full set.
---

# Phase 42: Benchmark Identity, Renderers, Dedup & Identity Taxonomy — Verification Report

**Phase Goal:** polint can render benchmark-grade identity for every function and callsite, dedupe by semantic identity, and distinguish identity-vs-unsupported categories so every downstream metric becomes trustworthy.
**Verified:** 2026-05-29
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

This phase carries three functional requirements (IDENT-01/02/03) plus a CI-gate
deliverable (Plan 04, SC5). I verified each ROADMAP Success Criterion against the
codebase — not against SUMMARY claims — reading the identity substrate, both
renderers, the eval wiring, the categorize module, and the leak gate, and running
the relevant targeted tests (78 identity lib tests + the 5 leak-gate tests, all green).

### Observable Truths (ROADMAP Success Criteria)

| # | Truth (Success Criterion) | Status | Evidence |
|---|---------------------------|--------|----------|
| 1 | IDENT-01: stable identity record `(file, span, language, package/module, container, display, signature digest)` for every function and callsite, deduplicated before scoring (snapshot fixtures) | ✓ VERIFIED | `facts.rs` `IdentityRecord` carries all 7 fields + `multiplicity` + `stable_key`; `provider.rs` 5-phase pipeline extracts from `db.functions()`/`db.call_sites()` (composition, D-04 — calls store unmodified), `dedup.rs` BTreeMap-keyed semantic collapse with `multiplicity` counter and locked sort key; `identity_dedup_fixture` + `identity_dedup_fixture_determinism` pass; digest uses stable payloads not dense IDs (`identity_output_digest_uses_stable_payloads_not_dense_ids` passes); manifest order `polint.calls → polint.identity → polint.abstract_domains` asserted in 4+ provider-order tests |
| 2 | IDENT-02: Go `RelString`-style names AND Jelly `file:start_line:start_col:end_line:end_col` spans with ≥99% Jelly oracle-span coverage on micro fixtures + CRLF/LF normalization | ✗ PARTIAL | **Jelly half VERIFIED:** `jelly_span::render` is the single source of truth, genuinely consumed in `jelly_callgraph.rs::normalize_kernel_output` (`render_span` produces the actual observed edge `from`/`to`); `identity_jelly_oracle_coverage_fixture` runs the full adapter end-to-end and asserts `ratio ≥ 0.99` with a `total > 0` guard (passes at 1.0, 3/3). **Go RelString half FAILED on real data:** provider feeds the file path as `package_or_module` (CR-01) so real records render `src/main.go.Foo`, and every `go_relstring::render` call site discards the output. See Gaps. |
| 3 | CRLF/LF normalization fixture passes and produces byte-identical renderer output | ✓ VERIFIED | `jelly_span.rs::line_columns` collapses `\r\n→\n` at render time leaving on-disk byte spans true (D-12); `identity_crlf_normalization_fixture` loads both `repo-lf/` and `repo-crlf/` (`.gitattributes: * -text`) and asserts byte-identical Jelly output per record; passes |
| 4 | IDENT-03: distinct categories `wrong_identity`, `unsupported_edge`, `unresolved_edge`, `package_load_limitation`, `model_missing` in evaluation output | ✓ VERIFIED | `IdentityCategory` closed `#[repr(u8)]` enum, exactly 5 variants in pinned source order, no `Other`/wildcard; exhaustive `match` over all 17 `UnresolvedCallReason` + 7 `CallTargetStatus` variants (compile-error on new variant); `CategorizedFailureSection` has the 5 distinctly-named snake_case `u32` counters, `#[serde(default)]` for v1.2 reverse-compat; all 5 proven to fire — 2 from real source (`identity_categorized_failures_fixture`), 3 via unit tests (`..._package_load_limitation_fires_on_setup_missing`, `..._model_missing_fires_on_rejected_target`, `..._wrong_identity_fires_on_oracle_span_overlap`, `drive_record_category_model_missing`) |
| 5 | Public-surface-leak CI gate installed: external rule crate compiles against `polint::sdk::prelude::*` and reaches zero v1.3 solver types | ✓ VERIFIED | `crates/polint/tests/public_surface_leak.rs` + probe crate (`#![no_implicit_prelude]` + single `use ::polint::sdk::prelude::*;`), `ALLOWED_PRELUDE` 97-entry source-of-truth list, workspace-excluded probe; CI `leak-gate` job runs `cargo test --package polint --test public_surface_leak` on `[ubuntu-latest, macos-latest]` with `fail-fast: false`; all 5 leak tests pass (compile-against-prelude, allowlist snapshot, parser self-test, no-private-namespace) — confirms every new identity/renderer/category type stayed `pub(crate)` |

**Score:** 4/5 success criteria verified (SC2 / IDENT-02 partial — Jelly delivered, Go RelString not delivered on real data)

### Deferred Items

Items not fully met in Phase 42 but explicitly addressed by later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | Go RelString fed a real Go package/import path and consumed by a scoring path | Phase 46 | Phase 46 SC: `src/go/semantic/` maps `go/packages`+`go/ssa` facts (receiver types, method sets) to constraints with "stable identities and exact source spans" — supplies the package path Phase 42's provider lacks |
| 2 | ≥99% Jelly coverage across the FULL JS/TS Jelly fixture set (not one micro fixture) | Phase 45 | Phase 45 SC: JS/TS enumeration "with Jelly-shaped spans matching ≥99% of Jelly fixture oracle spans" — the broad multi-fixture coverage CR-02/CR-06 flag |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `analysis/identity/facts.rs` | IdentityRecord + IdentityKind + SignatureDigest + LanguageTag + IdentityRecordId (pub(crate)) | ✓ VERIFIED | All present, `pub(crate)`; signature digest length-prefixed + domain-separated (FNV-1a, not SHA-256 despite doc — see WR-01) |
| `analysis/identity/provider.rs` | derive_identity_with_cache_stats + identity_output_digest | ✓ VERIFIED (with defect) | Pipeline present and wired; `package_or_module` for Go set to file path (CR-01 defect — see Gaps) |
| `analysis/identity/dedup.rs` | BTreeMap semantic dedup + multiplicity | ✓ VERIFIED | BTreeMap dedup, multiplicity counter, locked sort key, order-independent canonical selection (tie-on-non-sort-key edge case is CR-03, medium) |
| `analysis/identity/cache_key.rs` | identity_provider_parameter_digest with renderer version strings | ✓ VERIFIED | `Digest::from_parts(DigestKind::ProviderParameters, ...)`; `go_relstring_v1`/`jelly_span_v1` trip-wires present |
| `analysis/identity/store.rs` | IdentityStore::from_output with dangling-ref validation | ✓ VERIFIED | Present; indexes unused in production (WR-04 `#[allow(dead_code)]`, acceptable scaffolding) |
| `analysis/identity/validate.rs` | validate_identity diagnostics pass | ✓ VERIFIED | Present and wired |
| `analysis/identity/render/go_relstring.rs` | Go RelString renderer | ⚠️ ORPHANED (output discarded) | Format-correct on synthetic input; fed wrong field by provider + output never consumed (CR-01) |
| `analysis/identity/render/jelly_span.rs` | Jelly span renderer, CRLF at render time | ✓ VERIFIED | Consumed end-to-end by Jelly adapter; column convention is per-codepoint (CR-02 UTF-16 edge case, low risk on ASCII fixtures) |
| `analysis/identity/categorize.rs` | category projections + closed enum + Reason tag | ✓ VERIFIED | Closed 5-variant enum, exhaustive matches, no new fact family |
| `eval/report.rs` | JellyOracleCoverageSection + CategorizedFailureSection | ✓ VERIFIED | Both present; sibling fields on MetricSections; MetricSummary shape unchanged (layout-lock test) |
| `eval/metrics.rs` | jelly_oracle_coverage + categorized_failures wiring | ✓ VERIFIED | Both populated in the metrics build path |
| `eval/runner.rs` | categorized_failures wiring + coverage fixture test | ✓ VERIFIED | Coverage threshold + categorized fixtures driven here |
| `analysis_kernel/provider.rs` | polint.identity manifest between calls and abstract_domains | ✓ VERIFIED | Ordering asserted in multiple provider-order tests |
| `tests/public_surface_leak.rs` (crate-local) | leak gate + ALLOWED_PRELUDE | ✓ VERIFIED | Lives at `crates/polint/tests/public_surface_leak.rs` (plan recorded root `tests/` — path mismatch only, not missing) |
| probe crate `src/lib.rs` | `#![no_implicit_prelude]` + prelude glob | ✓ VERIFIED | Uses `use ::polint::sdk::prelude::*;` (leading `::` required under no_implicit_prelude — semantically the plan's glob) |
| dedup/crlf/jelly_oracle/categorized eval fixtures | snapshot assertions | ✓ VERIFIED | All four fixtures present, asserted, and pass via their runner tests |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `provider.rs` | `analysis/calls` facts | `db.call_sites()` reads (composition) | ✓ WIRED | `db.call_sites()` at provider.rs:84,266 (manual confirm; SDK regex false-negative) |
| `cache_key.rs` | kernel incremental Digest/DigestKind | `Digest::from_parts(DigestKind::ProviderParameters` | ✓ WIRED | cache_key.rs:13-14 (SDK reported "invalid regex" — false negative) |
| `analysis_kernel/provider.rs` | polint.identity manifest | ordering between calls/abstract_domains | ✓ WIRED | provider.rs:405/429/445 + provider-order tests |
| `jelly_callgraph.rs` | `jelly_span::render` | renderer replaces inline format!() | ✓ WIRED | render_span closure consumes output as observed edge keys |
| `go_x_tools_callgraph.rs` | `go_relstring::render` | renderer replaces inline Go name format | ⚠️ PARTIAL | Called (line 151) but output discarded; oracle key uses display_name |
| `report.rs::MetricSections` | JellyOracleCoverageSection | `#[serde(default)]` field | ✓ WIRED | report.rs:108-109 |
| `report.rs::MetricSections` | CategorizedFailureSection | `#[serde(default)]` field after jelly | ✓ WIRED | report.rs:110-111 |
| `metrics.rs` | category_for_* projections | called per fact in metrics build | ✓ WIRED | metrics.rs:366-398 (plan expected runner.rs; metrics build path is the actual + correct location) |
| `categorize.rs` | calls/facts.rs | exhaustive match over UnresolvedCallReason/CallTargetStatus | ✓ WIRED | categorize.rs:79-124, no wildcard arm (SDK regex false-negative) |
| `public_surface_leak.rs` | probe crate | cargo subprocess compiles probe | ✓ WIRED | `probe_crate_compiles_against_prelude_only` passes |
| `.github/workflows/ci.yml` | leak gate | cargo test on ubuntu + macos | ✓ WIRED | leak-gate matrix job, fail-fast:false |
| `public_surface_leak.rs` | `sdk/mod.rs` | ALLOWED_PRELUDE mirrors prelude | ✓ WIRED | `allowlist_matches_prelude_source` passes (97 entries) |

> Note: the gsd-sdk `verify.key-links` tool produced multiple false negatives for Plan 01/03/04 (regex-escaping bugs — e.g. "Invalid regex pattern" — and the `tests/public_surface_leak.rs` workspace-root path that actually lives crate-local at `crates/polint/tests/`). Every link was re-checked manually against source and confirmed WIRED except the Go RelString link, which is genuinely PARTIAL (renderer called, output discarded).

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `jelly_span::render` output | observed edge `from`/`to` | `db.identity_records()` populated by live provider over real kernel output | ✓ Yes (oracle coverage = 1.0, 3/3) | ✓ FLOWING |
| `categorized_failures` counters | `CategorizedFailureSection` | live `db` call facts via `categorized_failures_from_db` | ✓ Yes (2 categories from real fixture source; 3 from real-DB unit tests) | ✓ FLOWING |
| `go_relstring::render` output | (none) | discarded at every call site | ✗ output not consumed; input field wrong on real records | ✗ DISCONNECTED |
| identity dedup output | `db.identity_records()` | provider pipeline | ✓ Yes (snapshot fixture byte-stable) | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Identity substrate + renderers + categorize + fixtures | `cargo test -p polint --lib --all-features --locked identity` | 78 passed, 0 failed | ✓ PASS |
| Leak gate (compile probe + allowlist snapshot + parser self-test) | `cargo test --package polint --test public_surface_leak --locked` | 5 passed, 0 failed | ✓ PASS |
| End-to-end Jelly oracle coverage | (within above) `identity_jelly_oracle_coverage_fixture` | ok (ratio 1.0, total>0 guard) | ✓ PASS |
| CRLF/LF byte-identical | (within above) `identity_crlf_normalization_fixture` | ok | ✓ PASS |
| Dedup determinism | (within above) `identity_dedup_fixture_determinism` | ok | ✓ PASS |
| Full workspace suite | (orchestrator pre-ran `make test`) | 1621 lib + 140 integ + doctests, 0 failures | ✓ PASS |

### Probe Execution

No `scripts/*/tests/probe-*.sh` probes are declared for this phase (this is a Rust
library/eval phase, not a migration/tooling phase with shell probes). The
verification gates here are Rust `#[test]` functions, executed above. Step 7c: not
applicable (no shell probes declared or conventional).

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|---------------|-------------|--------|----------|
| IDENT-01 | 42-01, 42-04 | Stable internal identity records, deduped by semantic identity before scoring | ✓ SATISFIED | SC1 verified; substrate + dedup + fixtures pass |
| IDENT-02 | 42-02, 42-04 | Per-benchmark renderers (Go RelString + Jelly span), ≥99% Jelly coverage, CRLF/LF normalization | ⚠️ PARTIAL | Jelly + CRLF + ≥99% coverage verified; Go RelString not delivered on real data (CR-01) — Go consumption deferred to Phase 46 |
| IDENT-03 | 42-03, 42-04 | Distinct identity-vs-unsupported categories in eval output | ✓ SATISFIED | SC4 verified; closed enum + 5 distinct counters + all-5-fire proof |

No orphaned requirements: REQUIREMENTS.md maps IDENT-01/02/03 to Phase 42 only, and all three are claimed by phase plans.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `go_x_tools_callgraph.rs` / `observed.rs` | 151 / 670 | `let _ = go_relstring::render(...)` — output discarded | ⚠️ Warning | Renderer not consumed; combined with CR-01 means Go RelString unproven on real data |
| `provider.rs` | 197-199 | `package_or_module_for_file` returns file path for Go | ⚠️ Warning (CR-01) | Go RelString renders `src/main.go.Foo` not `pkg.Foo` on real records |
| `dedup.rs` | 81 | sort-key tie keeps first-inserted when non-sort-key fields differ | ℹ️ Info (CR-03) | `originating_call_site_id` could vary by input order in a same-span tie; Phase 43 determinism gate concern |
| `metrics.rs` / `categorize.rs` | 391-396 / 135 | `oracle_overlap` is exact byte-range equality, not interval overlap | ℹ️ Info (CR-04) | Under-counts `wrong_identity` for partial overlaps; param name misleads; behavior is internally consistent + tested |
| `metrics.rs` | 307-311 | empty-oracle `ratio = 1.0` | ℹ️ Info (CR-06) | In-scope runner test guards `total > 0`; suite-wide path could still go vacuously-green if a future fixture stops emitting oracle spans |
| `facts.rs` | 47-52 | doc says "SHA-256" but impl is FNV-1a | ℹ️ Info (WR-01) | Doc/impl mismatch; FNV choice is correct (no-new-deps), wording misleads |
| `tests/.../dedup/repo/src/main.go` | 5-11 | comment claims multiplicity=2; fixture asserts 1 | ℹ️ Info (WR-02) | Stale comment; behavior + assertion correct |

No `TBD`/`FIXME`/`XXX` debt markers in any Phase 42 file (debt-marker gate clean). No
`TODO`/`HACK`/`PLACEHOLDER`. Production code is panic-free (all `unwrap`/`expect`/`panic`
under `#[cfg(test)]`, confirmed by the review and consistent with the leak gate's
panic-free assertion).

### Human Verification Required

None. All gaps are programmatically observable in code and all gates run as Rust tests.
The single goal-level gap (Go RelString on real data) requires a developer DECISION
(accept the deferral to Phase 46 vs. fix now), not human testing — surfaced below.

### Gaps Summary

The phase delivers four of five ROADMAP success criteria cleanly: the identity substrate
with semantic dedup (SC1/IDENT-01), the Jelly span renderer with ≥99% oracle coverage and
CRLF/LF byte-identical normalization (SC2-Jelly + SC3), the closed 5-category taxonomy in
eval output (SC4/IDENT-03), and the dual-platform public-surface-leak CI gate (SC5). All of
these are verified end-to-end by passing fixtures and tests that exercise the REAL provider
and adapter paths over real source — not synthetic stubs.

The one genuine goal-level gap is the **Go RelString half of IDENT-02**. The renderer exists
and is format-correct, but two compounding facts mean it is not a working benchmark renderer
on real Go records:

1. The provider populates `package_or_module` with the workspace-relative FILE PATH
   (`db.path_for(file)`), not the Go import/package path, so `go_relstring::render` produces
   `src/main.go.Foo` instead of `module/path/pkg.Foo`. The unit tests pass only because they
   hand-construct records with a correct package string.
2. Every call site of the renderer in the crate discards its output (`let _ = ...`); the Go
   RTA oracle key is derived from `display_name`, so the wrong output is never even surfaced.

The ROADMAP's *quantitative* IDENT-02 contract ("≥99% Jelly oracle-span coverage") is
Jelly-specific and is met. The Go RelString clause is delivered as scaffolding only.

**This looks like an intentional deferral, not an accident.** `FunctionFact` (the v1.2 substrate
Phase 42 composes over) carries no Go package, and the substrate that resolves Go package/import
paths via `go/packages`+`go/ssa` is the explicit deliverable of **Phase 46 (Go Semantic Frontend
& Sidecar)**. The renderer-version trip-wire (`go_relstring_v1`) is already in place so a future
fix invalidates the cache cleanly. The SUMMARY, however, overstates this as "IDENT-02 fully
addressed... the exact Go RelString," which is not true on real data.

Decision needed from the developer:

- **Option A (defer):** Accept that the Go RelString renderer is dormant scaffolding whose real
  package path + consumption lands in Phase 46. If chosen, record an override (below) so the
  phase passes, and update the SUMMARY/ROADMAP wording to stop claiming Go RelString is fully
  delivered end-to-end. The deferred-items section already documents Phase 46 coverage.
- **Option B (fix now):** Join Go records to their package via `PackageFact`/module-graph package
  nodes in the provider, add a provider-level test asserting `pkg.Func` form from a real Go
  `FunctionFact`, and either consume the RelString or assert it as a fixture invariant.

If Option A is chosen, add to this file's frontmatter:

```yaml
overrides:
  - must_have: "Go RelString-style function/method names on real Go records (IDENT-02, D-07)"
    reason: "Go package/import-path resolution requires the go/packages+go/ssa semantic frontend delivered in Phase 46; FunctionFact in the v1.2 substrate carries no package. The renderer is format-correct scaffolding with a versioned cache trip-wire; real consumption lands with its data source in Phase 46."
    accepted_by: "<your name>"
    accepted_at: "<ISO timestamp>"
```

(CR-03/CR-04/CR-06 and the WR-* items are correctness/determinism/doc refinements that do not
block the phase goal; CR-03 in particular is worth resolving before the Phase 43 determinism
gate relies on byte-stable `originating_call_site_id`.)

---

_Verified: 2026-05-29_
_Verifier: Claude (gsd-verifier)_
