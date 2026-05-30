---
phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy
verified: 2026-05-29T00:00:00Z
status: passed
score: 5/5 roadmap success criteria verified (IDENT-02 Go half closed for feasible scope; full import path deferred to Phase 46)
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 4/5
  gaps_closed:
    - "Per-benchmark renderers produce Go RelString-style names with benchmark-grade fidelity on real Go records (IDENT-02, D-07, SC2) — feasible (package-NAME) portion"
    - "Dedup canonical selection is order-independent even when record_sort_key ties but originating_call_site_id differs (CR-03 / IDENT-01)"
  gaps_remaining: []
  regressions: []
deferred:
  - truth: "Full Go module import-path RelString (`module/path/pkg.Func`) fed by a real import path and consumed in the Go RTA oracle scoring path"
    addressed_in: "Phase 46"
    evidence: >-
      Phase 46 (Go Semantic Frontend & Sidecar) Success Criteria #1/#2: the
      `polint-go-frontend` Go sidecar uses `go/packages` + `go/ssa` +
      `golang.org/x/tools` to emit NDJSON facts (functions, methods, receiver
      types, method sets, call sites, types), and `src/go/semantic/` maps them to
      semantic-graph constraints "with stable identities and exact source spans".
      This is the substrate that supplies the full Go import path the Phase 42
      provider cannot derive: `FunctionFact`/`PackageFact` in the v1.2 substrate
      carry only the package-clause NAME, not the import path. Pulling it forward
      would regress the x/tools RTA bare-name `WANT:` oracle (which the Go RTA
      adapter key intentionally still derives from `display_name`).
  - truth: "≥99% Jelly oracle-span coverage demonstrated across the FULL JS/TS Jelly fixture set (not a single micro fixture)"
    addressed_in: "Phase 45"
    evidence: >-
      Phase 45 (JS/TS Inventory) Goal/Success Criteria: polint enumerates every
      JS/TS function and callsite "with Jelly-shaped spans" — the broad,
      multi-fixture Jelly oracle-span coverage over the full set. Phase 42's SC2
      contract is explicitly scoped to "≥99% ... on micro fixtures", which is met.
---

# Phase 42: Benchmark Identity, Renderers, Dedup & Identity Taxonomy — Verification Report

**Phase Goal:** polint can render benchmark-grade identity for every function and callsite, dedupe by semantic identity, and distinguish identity-vs-unsupported categories so every downstream metric becomes trustworthy.
**Verified:** 2026-05-29
**Status:** passed
**Re-verification:** Yes — after gap-closure plan 42-05 (commits 32bb0e9, 070d10b, 7866435)

## Re-Verification Context

The initial verification (status `gaps_found`, 4/5) found a single goal-level gap: the
**Go RelString half of IDENT-02**. The Go renderer was format-correct but (a) fed the
workspace-relative FILE PATH (`db.path_for`) as `package_or_module`, so real Go records
rendered `src/main.go.Foo` instead of a package-qualified name, and (b) had its output
discarded at every call site (`let _ = …`). A medium correctness item (CR-03) left dedup's
canonical selection input-order-dependent when `record_sort_key` tied but
`originating_call_site_id` differed.

Gap-closure plan **42-05** has executed. I re-verified the closed gaps with full 3-level
checks (exists / substantive / wired) and ran the specific gap-closure tests in my own
process. Every closed gap holds; no regressions. The remaining unbuilt piece — the FULL Go
module import path and its consumption in the oracle scoring path — has a hard data
dependency on the Phase 46 `go/packages`+`go/ssa` semantic frontend and is a legitimate
cross-phase deferral (recorded below), not a Phase 42 gap. Judged against what is feasible
in the v1.2 substrate today, the phase GOAL is achieved.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth (Success Criterion) | Status | Evidence |
|---|---------------------------|--------|----------|
| 1 | IDENT-01: stable identity record `(file, span, language, package/module, container, display, signature digest)` for every function and callsite, deduplicated by semantic identity before scoring (snapshot fixtures) | ✓ VERIFIED | `provider.rs` 5-phase pipeline extracts from `db.functions()`/`db.call_sites()` (composition, D-04); `dedup.rs` BTreeMap semantic collapse with `multiplicity`. **CR-03 closed:** `record_total_order_key` (dedup.rs:93-107) extends `record_sort_key` with `(originating_call_site_id, originating_call_target_id, signature_digest)` and is applied to BOTH the collision canonical-selection compare (dedup.rs:130) AND the final `output.sort_by_key` (dedup.rs:144), so selection is now a literal total order. `dedup_canonical_selection_is_total_order_on_call_site_id_tie` (built two records tying on every sort-key field but `Some(CallSiteId(1))` vs `Some(CallSiteId(2))`) passes byte-identical across input orders; `identity_dedup_fixture_determinism` + the existing dedup/sort tests all pass |
| 2 | IDENT-02: Go `RelString`-style names AND Jelly `file:start:col:end:end` spans with ≥99% Jelly oracle-span coverage on micro fixtures + CRLF/LF normalization | ✓ VERIFIED | **Jelly half (unchanged):** `jelly_span::render` consumed end-to-end in `jelly_callgraph.rs`; `identity_jelly_oracle_coverage_fixture` asserts `ratio ≥ 0.99` with a `total > 0` guard (passes). **Go RelString half — CR-01 CLOSED:** `package_or_module_for_record` (provider.rs:214-224) is language-aware — `Language::Go → package_name_for_go_file(db, file).unwrap_or_else(\|\| db.path_for(file))`; `_ → db.path_for(file)` (non-Go byte-identical). `package_name_for_go_file` (provider.rs:232-237) scans `db.packages()` for the first `PackageFact` with `file == file && language == Language::Go` and returns its `name`. **Real-provider proof:** `go_function_renders_package_qualified_through_real_provider` (provider.rs:571) builds a `package foo` file + Go `PackageFact{name:"foo"}` + `FunctionFact{name:"Bar"}`, runs `derive_identity_with_cache_stats`, and asserts `go_relstring::render(record) == "foo.Bar"` — NOT a hand-built record. Three unit tests cover Go-with-package→`foo`, Go-without-package→path fallback, TS→path-regardless-of-PackageFact. All pass. Full import path (`module/path/pkg.Func`) deferred to Phase 46 (see Deferred) |
| 3 | CRLF/LF normalization fixture passes and produces byte-identical renderer output | ✓ VERIFIED | `jelly_span.rs::line_columns` collapses `\r\n→\n` at render time (D-12); `identity_crlf_normalization_fixture` loads `repo-lf/`+`repo-crlf/` and asserts byte-identical Jelly output; passes (unchanged by Plan 05) |
| 4 | IDENT-03: distinct categories `wrong_identity`, `unsupported_edge`, `unresolved_edge`, `package_load_limitation`, `model_missing` in evaluation output | ✓ VERIFIED | Closed 5-variant `#[repr(u8)]` enum, exhaustive matches, 5 distinct `u32` counters on `CategorizedFailureSection`, all 5 proven to fire (`identity_categorized_failures_fixture` + unit tests). Unchanged by Plan 05 |
| 5 | Public-surface-leak CI gate installed: external rule crate compiles against `polint::sdk::prelude::*` and reaches zero v1.3 solver types | ✓ VERIFIED | All 5 leak tests pass (re-run this verification: `probe_crate_compiles_against_prelude_only`, `allowlist_matches_prelude_source`, `allowlist_has_no_duplicates_and_expected_count`, `ensure_no_private_namespace_in_probe`, `parser_self_test_detects_synthetic_leak`). Confirms every new identity type (including the new `package_name_for_go_file`, `record_total_order_key`, `SortKey`/`TotalOrderKey` aliases) stayed `pub(crate)` |

**Score:** 5/5 success criteria verified. The one prior gap (Go RelString on real data) is closed for the feasible package-NAME scope; the full Go import path is a Phase 46 data dependency, properly deferred.

### Deferred Items

Items not fully met in Phase 42 but explicitly addressed by later milestone phases (hard data dependencies, not Phase 42 gaps).

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | Full Go module import-path RelString (`module/path/pkg.Func`) + consumption in the Go RTA oracle scoring path | Phase 46 | Phase 46 SC1/SC2: `polint-go-frontend` sidecar (`go/packages`+`go/ssa`) → `src/go/semantic/` lowering "with stable identities and exact source spans" — supplies the import path the v1.2 substrate lacks; pulling it forward would regress the x/tools RTA bare-name oracle |
| 2 | ≥99% Jelly coverage across the FULL JS/TS Jelly fixture set (not one micro fixture) | Phase 45 | Phase 45 Goal: JS/TS enumeration with Jelly-shaped spans over the full set; Phase 42 SC2 is explicitly scoped to "micro fixtures", which is met |

### Required Artifacts (Re-Verified — Plan 05 delta)

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `analysis/identity/provider.rs` | language-aware Go package resolution + real-provider test | ✓ VERIFIED | `package_or_module_for_record` (L214) + `package_name_for_go_file` (L232) present and wired into both record builders (L95, L117); `go_function_renders_package_qualified_through_real_provider` (L571) runs the real pipeline and asserts `foo.Bar`. Production code panic-free (`Option` + `unwrap_or_else` path fallback) |
| `analysis/identity/cache_key.rs` | bumped `go_relstring_v2` trip-wire | ✓ VERIFIED | `go_relstring_v2` appears exactly twice (digest fn L25 + locked test L46); `go_relstring_v1` only in the deliberate pre-bump literal of `go_renderer_version_bump_invalidates_the_pre_bump_digest` (L57), which asserts the live digest differs from v1 |
| `analysis/identity/dedup.rs` | total-order tie-break + determinism test | ✓ VERIFIED | `record_total_order_key` (L93) used in collision compare (L130) AND final sort (L144); `SortKey`/`TotalOrderKey` aliases keep clippy clean; `dedup_canonical_selection_is_total_order_on_call_site_id_tie` (L321) passes |
| `analysis/identity/render/go_relstring.rs` | Go RelString renderer (now fed correct package) | ✓ VERIFIED (no longer orphaned) | Format-correct; now fed the Go package NAME on real records and its output is asserted at both call sites — the prior ORPHANED status is resolved |
| `eval/observed.rs` | asserted render output | ✓ VERIFIED | L672-677: `let go_rel = go_relstring::render(record); assert!(!go_rel.is_empty(), …)` — no discarded `let _` |
| `eval/external/go_x_tools_callgraph.rs` | asserted render + Phase 46 deferral note; oracle key on display_name | ✓ VERIFIED | L152-157: `let rel_string = …render(record); debug_assert!(!rel_string.is_empty(), …)`; L158-162 inline Phase 46 deferral NOTE; L163 oracle key remains `go_x_tools_function_identity(record.display_name.as_ref())` — adapter tests (8) all pass, no oracle regression |
| `analysis/identity/facts.rs` | WR-01 doc fix | ✓ VERIFIED | SignatureDigest doc (L47) now reads "Length-prefixed two-pass FNV-1a 16-byte signature digest"; zero `SHA-256` occurrences in the file |
| `tests/eval-fixtures/identity/dedup/repo/src/main.go` | WR-02 comment fix | ✓ VERIFIED | Comment (L5-10) now states the live fixture asserts `multiplicity = 1` and the collapse-to-2 contract is proven by co-located dedup unit tests |
| `42-02-SUMMARY.md` | IDENT-02 overstatement reconciled | ✓ VERIFIED | "Next Phase Readiness" (L218) now explicitly says the original wording overstated the Go half, states the renderer produces `pkg.Func` on real records, and defers the full import path to Phase 46 |

(All other Phase 42 artifacts — facts/store/validate/categorize/render-jelly, eval report/metrics/runner, the leak gate, and the four eval fixtures — were VERIFIED in the initial report and are unchanged by Plan 05.)

### Key Link Verification (Plan 05 delta)

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `provider.rs` | `db.packages()` | `package_name_for_go_file` joins `PackageFact` by `file` + `Language::Go` | ✓ WIRED | provider.rs:233-236; `db.packages()` confirmed `pub fn packages(&self) -> &[PackageFact]` at core/mod.rs:3114 |
| `provider.rs` | `cache_key.rs` | `identity_provider_parameter_digest` feeds the output digest | ✓ WIRED | provider.rs:263 calls `identity_provider_parameter_digest()`; parts include `go_relstring_v2` |
| `dedup.rs` collision compare + final sort | `record_total_order_key` | single comparison key for both selection and ordering | ✓ WIRED | dedup.rs:130 (compare) + 144 (sort) use the same key |
| `go_x_tools_callgraph.rs` | `go_relstring::render` | renderer exercised + asserted; oracle key on display_name | ✓ WIRED (intent-correct) | Output now asserted, not discarded; oracle key intentionally stays on `display_name` with inline Phase 46 deferral (the previous ⚠️ PARTIAL is resolved as designed) |
| `observed.rs` | `go_relstring::render` | render invariant asserts non-empty | ✓ WIRED | observed.rs:672-677 |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `go_relstring::render` on a Go record | `record.package_or_module` | `db.packages()` Go `PackageFact.name` via the live provider pipeline | ✓ Yes — `derive_identity_with_cache_stats` over a real Go FunctionFact yields `foo.Bar` (proven, not synthetic) | ✓ FLOWING (was DISCONNECTED) |
| identity dedup output | `db.identity_records()` | provider pipeline, total-order key | ✓ Yes (byte-stable across input order, proven) | ✓ FLOWING |
| `jelly_span::render` output | observed edge `from`/`to` | live provider over real kernel output | ✓ Yes (oracle coverage 1.0) | ✓ FLOWING |
| Go RTA oracle key | `record.display_name` | live identity records (RelString asserted alongside) | ✓ Yes (oracle key on display_name by design; full-import-path RelString deferred to Phase 46) | ✓ FLOWING (display_name key) |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| All identity lib tests (incl. all Plan 05 gap-closure tests) | `cargo test -p polint --lib --all-features --locked identity` | 84 passed, 0 failed | ✓ PASS |
| Real-provider Go renders `foo.Bar` | (within above) `go_function_renders_package_qualified_through_real_provider` | ok | ✓ PASS |
| Go-with/without-package + TS resolution | `go_function_with_package_resolves_package_name`, `go_function_without_package_falls_back_to_path`, `typescript_function_keeps_file_path_regardless_of_package_fact` | ok | ✓ PASS |
| Dedup determinism on call-site-id tie | `dedup_canonical_selection_is_total_order_on_call_site_id_tie` | ok | ✓ PASS |
| Cache trip-wire bump invalidates v1 | `go_renderer_version_bump_invalidates_the_pre_bump_digest` + `identity_provider_parameter_digest_locks_parts_list` | ok | ✓ PASS |
| Go x/tools RTA adapter (no oracle regression) | `cargo test -p polint --lib eval::external::go_x_tools` | 8 passed, 0 failed | ✓ PASS |
| Public-surface-leak gate | `cargo test -p polint --test public_surface_leak --locked` | 5 passed, 0 failed | ✓ PASS |
| Full workspace suite | (orchestrator pre-ran `make test`) | 1627 lib + 140 integ + doctests, 0 failures | ✓ PASS |

### Probe Execution

No `scripts/*/tests/probe-*.sh` probes are declared or conventional for this phase (Rust library/eval phase, not a migration/tooling phase). The verification gates are Rust `#[test]` functions, executed above. Step 7c: not applicable.

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|---------------|-------------|--------|----------|
| IDENT-01 | 42-01, 42-04, 42-05 | Stable internal identity records, deduped by semantic identity before scoring | ✓ SATISFIED | SC1 verified; substrate + dedup + total-order determinism (CR-03 closed) pass |
| IDENT-02 | 42-02, 42-04, 42-05 | Per-benchmark renderers (Go RelString + Jelly span), ≥99% Jelly coverage on micro fixtures, CRLF/LF normalization | ✓ SATISFIED | SC2/SC3 verified; Jelly + CRLF delivered; Go RelString now package-NAME-qualified on real records (CR-01 closed). Full import path is a Phase 46 data dependency (deferred) |
| IDENT-03 | 42-03, 42-04 | Distinct identity-vs-unsupported categories in eval output | ✓ SATISFIED | SC4 verified; closed enum + 5 distinct counters + all-5-fire proof |

REQUIREMENTS.md maps IDENT-01/02/03 to Phase 42 only (all marked Complete) and all three are claimed by phase plans. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | Prior `let _ = go_relstring::render(...)` discarded-output smell | ✓ RESOLVED | Both call sites now assert non-empty output; grep for the discarded bindings returns 0 |
| (none) | — | Prior CR-01 (file path as Go `package_or_module`) | ✓ RESOLVED | Language-aware resolution; real-provider test proves `foo.Bar` |
| (none) | — | Prior CR-03 (order-dependent dedup tie-break) | ✓ RESOLVED | Literal total-order key on both selection and sort; determinism test guards it |
| (none) | — | Prior WR-01 (SHA-256 doc) / WR-02 (stale multiplicity comment) | ✓ RESOLVED | FNV-1a doc corrected; fixture comment matches asserted multiplicity=1 |

No `TBD`/`FIXME`/`XXX` debt markers in any Phase 42 identity file (debt-marker gate clean). No `TODO`/`HACK`/`PLACEHOLDER`. Production code panic-free (helper returns `Option`; new asserts are `#[cfg(test)]`/`debug_assert!`). No new dependencies (Cargo.toml/Cargo.lock untouched, T-42-SC honored).

Residual lower-priority items (CR-04 exact-byte oracle overlap, CR-06 empty-oracle ratio=1.0 suite-wide) were ℹ️ Info in the initial report, are internally consistent and test-guarded, and do not block the phase goal. They remain as documented for the Phase 43 determinism gate / future fixture work.

### Human Verification Required

None. All gap-closure deliverables are programmatically observable in code and proven by Rust `#[test]` functions executed during this verification. The deferred items (full Go import path → Phase 46; broad Jelly coverage → Phase 45) are cross-phase data dependencies recorded above, not human-testable Phase 42 items.

### Gaps Summary

No gaps. The single prior goal-level gap — the Go RelString half of IDENT-02 — is closed
for everything the v1.2 substrate makes feasible:

1. **Go records now render package-NAME-qualified** (`foo.Bar`, not `src/main.go.Bar`). The
   provider resolves `Language::Go` records through `package_name_for_go_file` (joining
   `db.packages()` by file + Go language) with a path fallback, keeping all non-Go behavior
   byte-identical. This is proven END-TO-END by a real-provider test that runs
   `derive_identity_with_cache_stats` over a genuine Go `FunctionFact`+`PackageFact` and
   asserts the rendered `foo.Bar` — not the previously vacuous synthetic-record tests.
2. **The cache trip-wire was bumped** (`go_relstring_v1`→`go_relstring_v2`) with a locked
   test and a differs-from-v1 test, so the changed Go `package_or_module` invalidates cached
   identity cleanly.
3. **Dedup is now a literal total order** (CR-03 / IDENT-01) on both canonical selection and
   final sort, byte-stable across input order even on same-span ties — the contract Phase 43's
   determinism gate inherits, proven by a two-order determinism test.
4. **Both renderer call sites assert their output** (no discarded `let _`); the Go RTA oracle
   key intentionally stays on `display_name` with an inline Phase 46 deferral note (routing the
   package-name-only RelString into the oracle would regress the x/tools RTA bare-name `WANT:`
   oracle).
5. **The doc smells and the IDENT-02 overstatement are reconciled** (FNV-1a, multiplicity
   comment, 42-02-SUMMARY).

The remaining unbuilt piece — the FULL Go module import path (`module/path/pkg.Foo`) and its
consumption in the oracle scoring path — has a hard data dependency: `FunctionFact`/`PackageFact`
carry no import path, and the `go/packages`+`go/ssa` semantic frontend that supplies it is the
explicit deliverable of **Phase 46**. This is a legitimate cross-phase deferral, recorded in the
deferred-items section. Judged against what is feasible in the v1.2 substrate today, all five
ROADMAP success criteria are delivered and the phase GOAL — benchmark-grade identity, semantic
dedup, and a distinct identity-vs-unsupported taxonomy — is achieved.

---

_Verified: 2026-05-29_
_Verifier: Claude (gsd-verifier)_
