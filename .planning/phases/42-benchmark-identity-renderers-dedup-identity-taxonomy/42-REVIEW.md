---
status: issues_found
phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy
reviewed: 2026-05-29T00:00:00Z
depth: deep
reviewer: gsd-code-reviewer (adversarial)
files_reviewed: 27
findings:
  high: 2
  medium: 4
  low: 7
  total: 13
---

# Phase 42: Code Review Report

**Reviewed:** 2026-05-29
**Depth:** deep (cross-file: identity provider → renderers → eval metrics → leak gate)
**Scope:** Rust source changed in `35daac0..HEAD` (identity subtree, renderers, kernel wiring, eval, leak gate, fixtures)
**Status:** issues_found

## Summary

Phase 42 lands a well-structured identity substrate (`analysis::identity`), two pure renderers, a closed failure taxonomy, an eval-report extension, and a public-surface-leak CI gate. The visibility discipline (`pub(crate)` throughout) is clean, the closed-enum exhaustiveness contracts are real (no wildcard arms), the kernel manifest ordering is correct (`polint.identity` between `polint.calls` and `polint.abstract_domains`), serde reverse-compat is properly tested, and production code is panic-free (all `unwrap`/`expect`/`panic` live under `#[cfg(test)]`). The leak gate's parser self-test (BLOCKER #6) is genuinely present and the probe witnesses match the 97-entry allow-list.

However, the review surfaced two **HIGH** correctness defects where the renderers' stated contracts diverge from how they are wired against production data, plus several **MEDIUM** determinism/semantics gaps. None are security vulnerabilities (no new deps, no unsafe, no injection/path-traversal surface introduced), and the threat model T-42-SC "no new deps" holds (`sha2` is in fact NOT a workspace dep, so the FNV choice is consistent — see WR-04). The HIGH items are correctness bugs that current in-scope tests do not catch because the tests construct synthetic records rather than exercising the real provider→renderer path.

## High

### CR-01: Go `RelString` renderer receives the file path as `package_or_module`, so it cannot produce Go `RelString` form on real records (D-07 violated)

**File:** `crates/polint/src/analysis/identity/provider.rs:197-199`, `:95`, `:112`; consumed at `crates/polint/src/analysis/identity/render/go_relstring.rs:28-65`

**Issue:** The provider sets `package_or_module` via `package_or_module_for_file(db, file)` which is `db.path_for(file)` — i.e. the **workspace-relative file path** (e.g. `src/main.go`), not the Go import path / package (`module/path/pkg`). `go_relstring::render` then builds `format!("{package}.{display}")`, producing `src/main.go.Foo` instead of the `module/path/pkg.Foo` that D-07 and the module doc-comment promise to match `golang.org/x/tools/go/callgraph`. Every unit test in `go_relstring.rs` passes only because it hand-constructs records with `package_or_module = "module/path/pkg"`; no test exercises a record built by the actual provider. The defect is currently *masked* because `go_x_tools_callgraph.rs::go_rta_oracle_identity` calls the renderer purely for its side effect (`let _rel_string = ...render(record);`) and emits `go_x_tools_function_identity(record.display_name)` as the real oracle key (`go_x_tools_callgraph.rs:144-149`). The renderer's output is computed and discarded. The moment any consumer (Phase 43+) actually uses the RelString, it will be wrong on every real Go record.

**Fix:** Populate `package_or_module` with the Go package/import path (resolvable from package facts / module graph), not the file path, for `Language::Go` records; keep the file path only where the language genuinely has no package concept. Add a provider-level test that builds a record from a real Go `FunctionFact` and asserts `go_relstring::render(record)` yields `pkg.Func` form — not the synthetic-record tests that currently pass vacuously.

### CR-02: Jelly renderer re-derives line/column from byte offsets, replacing the precomputed span line/col the deleted formatter used — silent behavior change to observed Jelly spans

**File:** `crates/polint/src/analysis/identity/render/jelly_span.rs:20-103`; wiring `crates/polint/src/eval/external/jelly_callgraph.rs:128-151`; deleted formatter (old) `jelly_callgraph.rs` `jelly_span_identity`

**Issue:** The removed `#[cfg(test)] fn jelly_span_identity` emitted `span.start_line:span.start_col:span.end_line:span.end_col` straight from the edge's `Span` (the line/col the analysis already computed). The new `jelly_span::render` ignores `identity.span.{start_line,start_col,...}` entirely and recomputes them from `start_byte`/`end_byte` over `source.source` via `line_columns`. If the analysis's stored span line/col were computed with any different convention than `line_columns` (tab expansion, 0- vs 1-based columns, multi-byte/grapheme handling, BOM, or a half-open vs inclusive end), the rendered spans will silently differ from what Jelly's oracle expects and the broader Jelly micro-suite match rate will regress. The only in-scope guard is `identity_jelly_oracle_coverage_fixture`, which exercises a **single** hand-authored micro case (`app.json` with 2 functions / 1 call) at `ratio >= 0.99` — that one case can pass while real fixtures regress. `line_columns` also counts columns per UTF-8 *character* (`utf8_char_len`, `jelly_span.rs:105-114`), whereas Jelly/most byte-oracle tools count UTF-16 code units or bytes; on any non-ASCII source the column will diverge.

**Fix:** Confirm the column convention Jelly actually uses (byte offset, UTF-16 unit, or codepoint) and make `line_columns` match it exactly; add a regression test that renders a record whose `Span` already carries known-good line/col and asserts the renderer reproduces those values for an LF file (proving the recompute agrees with the stored span on the common path). Run the full Jelly micro-suite (not just the one-case coverage fixture) before relying on this path.

## Medium

### CR-03: Dedup canonical-record selection is order-dependent when sort keys tie but records differ in a non-sort-key field (D-11 violated for that case)

**File:** `crates/polint/src/analysis/identity/dedup.rs:69-95`

**Issue:** On a duplicate dedup-key hit the canonical record is replaced only when `record_sort_key(&record) < record_sort_key(existing)` (line 81). `record_sort_key` (lines 42-60) excludes `signature_digest` and `originating_call_site_id`. Two records can share an identical `DedupKey` (so they collapse) *and* an identical `record_sort_key` (so neither `<` holds) while differing in `originating_call_site_id` — e.g. two distinct call sites emitted at the exact same `(file, span)`. In that tie the first-inserted record wins, and "first inserted" depends on input iteration order. The retained record's `originating_call_site_id` therefore varies with run/file/provider order, violating the D-11 byte-stability contract the Phase 43 determinism gate inherits. The existing dedup tests never construct a tie with differing `originating_call_site_id`, so they don't catch it.

**Fix:** Make the canonical-selection total: break sort-key ties on the remaining identity fields (`originating_call_site_id`, `originating_call_target_id`, `signature_digest`) so the kept record is fully order-independent, and add a test that feeds two same-span/same-semantic records with different `originating_call_site_id` in both orders and asserts byte-identical output.

### CR-04: `categorized_failures_from_db` treats wrong-identity as exact span equality, not overlap, contradicting D-16 and the `oracle_overlap` name

**File:** `crates/polint/src/eval/metrics.rs:386-401`; `crates/polint/src/analysis/identity/categorize.rs:135-147`

**Issue:** D-16 and the `category_for_wrong_identity` doc say wrong-identity fires when the observed callsite "file/span *overlaps* an oracle entry." The implementation computes `oracle_overlap` as exact set membership of `(file_id.0, start_byte, end_byte)` against `oracle_callsite_spans` (metrics.rs:391-396). Exact byte-range equality is strictly narrower than overlap: a callsite whose span partially overlaps (or is contained within) an oracle span but is not byte-identical is classified as a plain miss, never `wrong_identity`. This under-counts `wrong_identity` for the precise case the category exists to capture ("polint named the right place wrong"), and the parameter name `oracle_overlap` actively misleads. The single unit test only checks the byte-identical case, so the gap is invisible.

**Fix:** Either implement true interval overlap (`observed.start < oracle.end && oracle.start < observed.end` within the same file) and rename the boolean accordingly, or — if exact match is the deliberate, determinism-driven choice — rename `oracle_overlap` to `oracle_span_exact_match` and update D-16's wording so the contract and code agree. Add a test exercising a partial-overlap (non-identical) span.

### CR-05: Identity provider output digest is computed from records that may never be stored on store-validation failure

**File:** `crates/polint/src/analysis/identity/provider.rs:48-71`

**Issue:** `derive_identity_with_cache_stats` computes `output_digest` from `output` (phase 5) and then calls `db.replace_identity_facts(output)`. If the store rejects the output (dangling originating ref), the `Err` arm still returns `output_digest: Some(output_digest)` (lines 66-70) describing records that were *not* persisted — `db.identity_records()` retains its prior (empty) state. A downstream cache keyed on this digest would record a "successful" output digest for a run whose facts were rejected, decoupling the digest from the stored state. The run does surface a diagnostic, but the digest/state inconsistency is a latent cache-coherence hazard.

**Fix:** On the `Err` path, return `output_digest: None` (or the digest of the empty/rolled-back state) so the reported digest always matches what is actually stored, and assert this in a test that forces a dangling-ref rejection through the provider entry point.

### CR-06: `jelly_oracle_coverage` empty-oracle returns `ratio = 1.0`, masking a totally-broken renderer as full coverage

**File:** `crates/polint/src/eval/metrics.rs:307-311`; threshold check `crates/polint/src/eval/runner.rs:identity_jelly_oracle_coverage_fixture` (guarded by `coverage.total > 0`)

**Issue:** When `total == 0` the function returns `ratio = 1.0` (documented as "vacuously fully covered"). The suite-wide aggregate `jelly_oracle_coverage_for_cases` flattens every case's expected/observed; if a future Jelly fixture stops emitting `jelly.call_graph.*` expected edges (e.g. an enumeration/parse regression), `total` collapses to 0 and the `>= 0.99` gate passes trivially — a renderer that matches *nothing* reports `1.0`. The in-scope runner test only guards `coverage.total > 0` for its own inline fixture, not for the suite-wide path used in `runner.rs`/`fixtures.rs`. This is the classic "vacuous green" coverage trap.

**Fix:** Keep `1.0` for the genuinely-empty case but have the suite-level threshold assertion require `total > 0` (fail if a coverage run produces zero oracle spans), so an empty oracle is treated as "coverage not measured," not "coverage perfect."

## Low

### WR-01: `SignatureDigest` doc-comment and D-03 say "SHA-256"; implementation is double-pass FNV-1a

**File:** `crates/polint/src/analysis/identity/facts.rs:47-52`, `:209-230`

**Issue:** The struct doc says "SHA-256-style signature digest truncated to 16 bytes" and the digest helper's comment claims it "preserv[es] collision resistance." The actual `digest_16` is two seeded FNV-1a passes over the same input — a non-cryptographic hash with far weaker collision resistance than SHA-256. The choice is *defensible* (FNV-1a matches the existing `cache::stable_hash` convention and avoids a new dependency — `sha2` is NOT actually a workspace dep, so T-42-SC is honored), but the "SHA-256" wording will mislead any future maintainer reasoning about collision bounds. **Fix:** Reword the doc to "deterministic 16-byte FNV-1a digest (non-cryptographic; collision-resistant enough at repo scale because inputs are length-prefixed and domain-separated)"; drop the "SHA-256-style" phrasing.

### WR-02: Dedup fixture source comment claims `multiplicity = 2`; fixture asserts `multiplicity = 1`

**File:** `tests/eval-fixtures/identity/dedup/repo/src/main.go:5-11` vs `tests/eval-fixtures/identity/dedup/expected.polint-eval.toml`

**Issue:** `main.go`'s comment says the two `helper()` callsites "collapse … into one record with multiplicity = 2." The actual fixture (correctly) asserts `identity.dedup.multiplicity == 1` because in-file callsites keep their span and stay distinct (D-09). The `expected.toml` comment explains this correctly, but the `main.go` comment is stale and contradicts both the assertion and Plan 01's stated must-have ("fixture asserting multiplicity = 2"). **Fix:** Update the `main.go` comment to match reality (distinct in-file callsites are preserved, not collapsed) and note in the SUMMARY that the plan's "multiplicity = 2" snapshot expectation moved to the co-located dedup unit test rather than the eval fixture.

### WR-03: `jelly_oracle_coverage/expected.polint-eval.toml` carries no `[[expected]]` rows — it is decorative

**File:** `tests/eval-fixtures/identity/jelly_oracle_coverage/expected.polint-eval.toml`

**Issue:** The real coverage assertion lives in `runner.rs::identity_jelly_oracle_coverage_fixture`, which builds a `SuiteManifest` inline and never loads this TOML. The TOML has only comments and `[repo]`/`[budget]` — no assertions. A reader expecting the fixture file to drive the gate will be misled, and the file can rot without any test noticing. **Fix:** Either add the actual coverage assertion as a fixture primitive the harness reads, or add a top-of-file note that the assertion is enforced by the named Rust test and the TOML exists only for repo layout.

### WR-04: `IdentityStore` indexes and `identity_store()` getter are dead in production

**File:** `crates/polint/src/analysis/identity/store.rs:30-105`; `crates/polint/src/core/mod.rs:1093-1096` (`#[allow(dead_code)] identity_store()`)

**Issue:** `by_file`/`by_language`/`by_kind` and `records_for_file/language/kind`, plus `AnalysisDb::identity_store()`, are never read outside store tests — hence the explicit `#[allow(dead_code)]`. Plan 01 (Pattern I) anticipated building only what consumers read, yet three indexes + a getter were built with zero consumers. This is acceptable scaffolding but the `#[allow(dead_code)]` will hide genuinely-unused future additions. **Fix:** Track the consumer (Phase 43+) that will read these, or trim to what `replace_identity_facts` actually needs (it only uses `store.records()`); prefer `#[expect(dead_code, reason="…")]` over `#[allow]` so it self-cleans when a consumer lands.

### WR-05: `compute_signature_digest` length prefix truncates field length to `u32`

**File:** `crates/polint/src/analysis/identity/facts.rs:194-197`

**Issue:** `(bytes.len() as u32).to_le_bytes()` wraps if a field exceeds 4 GiB, theoretically defeating the boundary-disambiguation guarantee (T-42-01). Not reachable with realistic package/container/display names, but the cast is silent. **Fix:** Use `u64` length prefixes (or `usize` via `to_le_bytes`) to remove the theoretical wrap, or document the bound.

### WR-06: `allowlist_matches_prelude_source` defensive block-end uses fragile `\n}` scan instead of the depth-aware parser it already has

**File:** `crates/polint/tests/public_surface_leak.rs:313-327`

**Issue:** The main parse (`parse_prelude_reexports`) walks brace depth correctly, but the secondary defensive check re-derives the block end with `block.find("\n}")` (lines 318-322). This happens to work today only because every grouped `pub use` closes with `\n    };` (indented), so the first `\n}` is the module's own close brace. A future reformat (e.g. a single-line `pub use crate::analysis::X;` followed by a top-level `}` mid-block, or rustfmt collapsing a group) could truncate `prelude_block` early and let a `pub use crate::analysis::` line escape the substring check. **Fix:** Reuse the depth-aware boundary already computed in `parse_prelude_reexports` (return the block slice, or factor the boundary scan into a shared helper) instead of the naive `\n}` find.

### WR-07: Provider `output_digest` part embeds `kind={:?}` (Debug) — couples the digest to Rust `Debug` formatting

**File:** `crates/polint/src/analysis/identity/provider.rs:227-234`

**Issue:** The per-record digest part uses `kind={:?}` (Debug of `IdentityKind`), while `language` uses the stable `as_str()`. `{:?}` on an enum emits the Rust variant name (`Function`/`Callsite`), which is stable today but is a `Debug` impl, not a documented serialization contract — a future derive/rename or a `#[derive(Debug)]` customization would silently change the cross-platform output digest. The plan's Pattern F text even specified `{:?}` here, but mixing a stable label for one field and `Debug` for another is inconsistent. **Fix:** Use an explicit stable label for `kind` (e.g. the existing `identity_kind_label`) so no digest input depends on `Debug`.

---

## Notes on what is sound (verified, not flagged)

- Visibility discipline holds: no bare `pub` on any Phase 42 identity/renderer/category/report type; the leak gate's probe (97 witnesses) matches `ALLOWED_PRELUDE` (97 entries) and `sdk/mod.rs` prelude exactly.
- Closed-enum exhaustiveness is real: `category_for_unresolved` / `category_for_unsupported` / `record_category` have explicit arm-per-variant `match`es with no `_ =>` wildcard; `IdentityCategory` is `#[repr(u8)]` with a source-order lock test.
- Kernel manifest ordering is correct and asserted in 4+ provider-order tests (`provider.rs`, `mod.rs`, `run_report.rs`, `fixtures.rs`, `observed.rs`).
- serde reverse-compat (`#[serde(default)]` on both new `MetricSections` fields) is tested; `MetricSummary` layout-lock destructure test is present and lists every field with no rest pattern.
- Production code is panic-free (all `unwrap`/`expect`/`panic`/`unreachable` are under `#[cfg(test)]`).
- No new third-party dependencies (T-42-SC honored: leak gate chose Approach B / direct cargo over `trybuild`; `sha2` was never added); no `unsafe`; no path canonicalization / traversal surface; renderer output is string-only and forward-slash-normalized (no absolute-host-path leak — verified by `identity_render_invariants` + renderer tests).
- CI leak gate runs on `ubuntu-latest` + `macos-latest` with `fail-fast: false` (D-18 satisfied); BLOCKER #6 parser self-test is present and exercises both a synthetic leak and a clean positive control.

_Reviewed: 2026-05-29_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
