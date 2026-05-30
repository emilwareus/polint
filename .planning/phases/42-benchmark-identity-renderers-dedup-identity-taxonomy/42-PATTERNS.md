# Phase 42 PATTERNS.md — analog map for the identity module

**Mapped:** 2026-05-28
**Phase:** 42 — Benchmark Identity, Renderers, Dedup & Identity Taxonomy

## Summary

- 18 new files / modifications to map (11 in `analysis::identity/`, 2 eval reporting changes, 2 eval external adapter touches, 4 test/fixture artifacts).
- 9 anchor analogs read end-to-end: `analysis::calls::{mod,facts,provider,extract,unresolved,direct,validate,store,cache_key}`, plus `analysis::ids`, `analysis_kernel::provider` (manifest), `analysis_kernel::incremental::run_report` (`KernelRunReport`), `eval::report` (`MetricSections`, `MetricSummary`, `UnknownMetricSection`), `eval::external::{mod,jelly_callgraph}`, `sdk::{mod,facts,scope}` (public-surface anchor), `tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml` (fixture layout).
- The `analysis::calls/` subtree is the single closest analog for the whole `analysis::identity/` module — same structure, same digest discipline, same `pub(crate)` visibility, same `stable_key` normalization, same dangling-reference validation, same provider manifest slot shape.

## New Files → Closest Analog

| New file | Role | Closest analog | What to copy | What NOT to copy |
|---|---|---|---|---|
| `crates/polint/src/analysis/identity/mod.rs` | module root | `crates/polint/src/analysis/calls/mod.rs` | One-line `pub(crate) mod …;` listing per submodule; nothing re-exported as `pub`. | Don't add a `pub use` that surfaces anything outside the crate; don't include unrelated submodules. |
| `crates/polint/src/analysis/identity/facts.rs` | fact family + closed enums + `SignatureDigest` newtype + `IdentityCategory` enum | `crates/polint/src/analysis/calls/facts.rs` | `pub(crate) struct` with `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`; closed `pub(crate) enum` for `IdentityKind`/`LanguageTag`/`IdentityCategory` with `Copy + Ord + Hash` where appropriate; `stable_key: String` field; per-fact `id` newtype referencing `analysis::ids` IDs; `#[cfg(test)] mod tests` co-located. | Do NOT add `Other`/`Unknown` variants to `IdentityCategory` (D-14 closed enum); do NOT define new public types; do NOT re-derive serde on transparent newtypes that hold raw bytes — `SignatureDigest([u8; 16])` should use `serde_bytes` style or `#[serde(with = "hex::serde")]` so hex round-trips deterministically. |
| `crates/polint/src/analysis/identity/provider.rs` | provider entry + output digest + dedup wiring | `crates/polint/src/analysis/calls/provider.rs` | The full extract→resolve→normalize→digest sequence; the `derive_*_with_cache_stats(db, input_snapshot, manifest, upstream_digests…)` signature; `CacheStats::default(); cache_stats.record_recompute();` semantics; `Digest::from_parts(DigestKind::ProviderOutput, "identity_output", &refs)` shape with sorted `parts: Vec<String>`; `db.replace_*` error path returning `Diagnostic`; `polint.identity` manifest lookup by id; `#[cfg(test)] fn …_digest_for_test(parts: &[&str]) -> Digest`. | Do NOT duplicate `extract_call_sites` — read existing `CallSiteFact`/`CallTargetFact`/`UnresolvedCallFact` via `db.call_sites()` etc. and project them into identity records; do NOT digest dense IDs (only stable_key + semantic fields, per the `digest uses stable payloads not dense ids` test pattern at calls/provider.rs lines ≈352–379); do NOT skip the dedup step before the digest (D-09 says dedup once, here). |
| `crates/polint/src/analysis/identity/dedup.rs` | semantic dedup with multiplicity counter | `crates/polint/src/analysis/calls/unresolved.rs` (BTreeMap-keyed dedup with `entry(stable_key).or_insert(...)` pattern) | The `BTreeMap<DedupKey, IdentityRecord>` accumulator with `.entry(...).or_insert_with(...)` insertion and a `+= 1` counter on duplicate hit; deterministic key tuple `(language, package_or_module, container_path, signature_digest, span)` per D-10. | Do NOT mutate fields other than `multiplicity` on duplicate hit (otherwise dedup is order-dependent); do NOT hash on `file_id` for cross-file aliases (per D-09 cross-file aliases drop `file_id`/`span`). |
| `crates/polint/src/analysis/identity/categorize.rs` | projects unresolved/unknown facts into `IdentityCategory` + `categorize::Reason` per-fact tag | `crates/polint/src/analysis/calls/unresolved.rs` (specifically `reason_for_site` / `reason_for_unsupported` / `status_for_reason` mapping helpers at lines 65–110) | The pure-function `fn categorize_*(input) -> IdentityCategory` shape; exhaustive `match` over the closed source enums (`UnresolvedCallReason`, `CallTargetStatus`) so a new variant becomes a compile error; `pub(crate) fn` with no DB writes. | Do NOT introduce new fact families (D-16: categorization is a tag on existing facts, not a new fact type); do NOT use `_ => …` wildcards on the source enums — match each variant explicitly. |
| `crates/polint/src/analysis/identity/cache_key.rs` | provider parameter digest | `crates/polint/src/analysis/calls/cache_key.rs` | The exact `pub(crate) fn identity_provider_parameter_digest() -> Digest` shape calling `Digest::from_parts(DigestKind::ProviderParameters, "identity_provider_parameters", &[ "identity-facts-1", "identity_records", "go_relstring_v1", "jelly_span_v1", … ])`; the co-located `#[cfg(test)]` exact-equality test that locks the parts list (calls/cache_key.rs lines 25–52). | Do NOT omit the renderer code version strings from the digest input (per D-24 renderer code changes must invalidate identity cache); do NOT depend on volatile data (env vars, host paths, time) — every part must be a static `&str`. |
| `crates/polint/src/analysis/identity/store.rs` | typed storage + dangling-ID validation + indexes | `crates/polint/src/analysis/calls/store.rs` | The `IdentityOutput { records: Vec<IdentityRecord> }` plus `IdentityStore` with `BTreeMap<…, Vec<usize>>` indexes; the `normalized()` method sorting by `(language, package_or_module, container_path, file_id, span, kind)` per the established-pattern bullet in CONTEXT.md; `from_output(...) -> Result<Self, AnalysisError>` returning `AnalysisError::InvalidFact { provider: "polint.identity", reason }` for dangling references to `CallSiteId`/`CallTargetId` (calls/store.rs lines 65–99); ref-getter helpers (`site_refs`, `target_refs`) following the same `indexes.map_or_else(Vec::new, ...)` shape. | Do NOT rebuild indexes the kernel doesn't need (calls/store.rs has six index maps for a reason — only build the ones identity consumers will read; over-indexing wastes memory and adds digest-relevant code paths). |
| `crates/polint/src/analysis/identity/validate.rs` | dangling reference / span / digest checks producing diagnostics | `crates/polint/src/analysis/calls/validate.rs` | The `pub(crate) fn validate_identity(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>)` signature; the `BTreeSet` of valid IDs gathered from `db.*()`; `check_ref(...)` and `check_duplicate_stable_keys(...)` helpers; the `if site.span.start_byte > site.span.end_byte { push_*_diagnostic(...) }` span-sanity check (calls/validate.rs lines 140–148). | Do NOT validate by panicking — every error is a `Diagnostic` pushed into the borrowed `&mut Vec<Diagnostic>`; do NOT mix categorization with validation. |
| `crates/polint/src/analysis/identity/render/mod.rs` | renderer re-export | `crates/polint/src/analysis/calls/mod.rs` | Single-line `pub(crate) mod go_relstring; pub(crate) mod jelly_span;` — nothing else. | No `pub use` that lifts renderer functions to `analysis::identity::*` without the `render::` prefix (keeps call sites explicit about which renderer is being invoked). |
| `crates/polint/src/analysis/identity/render/go_relstring.rs` | pure renderer projecting `IdentityRecord` → Go `RelString` | `crates/polint/src/analysis/calls/provider.rs` `callee_part(callee: &CallCallee) -> String` (lines 271–292) — same role as a pure deterministic string projection with exhaustive `match`. | The `pub(crate) fn render(identity: &IdentityRecord) -> String` signature; exhaustive `match` on `IdentityKind` and on the receiver-ness / generic-ness of the container path; deterministic anonymous-fn ordinal logic mirroring `same_span_ordinal` from `analysis::calls::extract` (lines 110–119) so `parent$N` numbering is byte-stable across runs. | Do NOT take `&AnalysisDb` or `&InputSnapshot` — D-06 mandates the renderer is a pure function of `&IdentityRecord`; do NOT format paths with `std::path::PathBuf::display()` (locale/platform-dependent on Windows separators) — manually slash-join. |
| `crates/polint/src/analysis/identity/render/jelly_span.rs` | pure renderer projecting `IdentityRecord` + `SourceFile` → `file:start_line:start_col:end_line:end_col` | `crates/polint/src/eval/external/jelly_callgraph.rs::jelly_span_identity` (lines 324–339) — current cfg(test)-only Jelly span formatter — and `canonical_location` (lines 283–310) for the exact `:`-separated shape Jelly expects. | The `pub(crate) fn render(identity: &IdentityRecord, source: &SourceFile) -> String` signature; the `replace('\\', "/")` workspace-relative path normalization; the `format!("{}:{}:{}:{}:{}", relative_path, start_line, start_col, end_line, end_col)` exact shape; **1-based line, 1-based column, half-open end** semantics per D-08 (matches Jelly's `canonical_location` parser at jelly_callgraph.rs line 307). The CRLF→LF normalization step (D-12) happens here, immediately before line/column computation — walk `source.text` collapsing `\r\n` → `\n` into a scratch byte-offset map so the byte span is translated into the post-normalization line/column. | Do NOT keep the renderer behind `#[cfg(test)]` — Phase 42 promotes it to production `pub(crate)`; do NOT mutate `SourceFile` (read-only borrow); do NOT normalize CRLF at file-load time (D-12 says renderer time only). |
| `crates/polint/src/eval/report.rs` (modification) | extend `MetricSections` with `categorized_failures` + `jelly_oracle_coverage` | self (`crates/polint/src/eval/report.rs` lines 95–165) — copy the existing `#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)] #[serde(rename_all = "snake_case", deny_unknown_fields)] pub(crate) struct *MetricSection` pattern verbatim. | Add `pub(crate) categorized_failures: CategorizedFailureSection` and `pub(crate) jelly_oracle_coverage: JellyOracleCoverageSection` fields with `#[serde(default)]` so older JSON consumers don't break; define both new sections with `deny_unknown_fields` and `snake_case` discriminants; project `IdentityCategory` via `#[serde(rename_all = "snake_case")]` so `WrongIdentity` → `wrong_identity` per D-15. | Do NOT change the `MetricSummary` shape itself (downstream gates lock the existing field set — extend via `MetricSections` only); do NOT add `Other`/catch-all category — the closed enum maps 1:1 to a counter map with exactly five fields per D-14. |
| `crates/polint/src/eval/external/jelly_callgraph.rs` (modification) | call into `analysis::identity::render::jelly_span` from `normalize_kernel_output` / `normalize_observed` | self (lines 111–167) — the existing `#[cfg(test)] fn normalize_kernel_output` already builds Jelly-shaped strings inline; the change replaces the inline `format!("{}:{}:{}:{}:{}", …)` with a `analysis::identity::render::jelly_span::render(&identity, source)` call. | The `BTreeMap<FileId, &SourceFile>` lookup; per-edge `Option`-short-circuit (`let Some(…) = …; else { continue; };`); `ObservedItem::GraphEdge(ObservedGraphEdge { … })` construction; collecting unknown facts via `crate::eval::observed::call_graph_unknown_facts_from_kernel_output`. | Do NOT keep the inline span formatter (per D-05 the renderer is the single source of truth); do NOT promote `normalize_kernel_output` out of `#[cfg(test)]` unless callers also move (separate-concern change). |
| `crates/polint/src/eval/external/go_rta.rs` (new, or per-existing-slot in `external/mod.rs` registry) | Go RTA adapter consuming `go_relstring::render` | `crates/polint/src/eval/external/jelly_callgraph.rs` (full file, lines 1–225) — same `impl BenchmarkAdapter` shape, same `enumerate_*_cases`/`parse_*_file`/`canonical_location` decomposition, same `pub(crate) struct …Adapter;` zero-sized type, same `adapter_id`/`language_support`/`enumerate_cases`/`prepare_case`/`normalize_observed` method set. | The `BenchmarkAdapter` trait impl skeleton; the `#[cfg(test)] fn normalize_kernel_output` shape; the file enumeration + JSON oracle parsing pattern (Go RTA produces JSON callgraphs the same way Jelly does); registering the adapter in `crates/polint/src/eval/external/mod.rs` `pub(crate) mod go_rta;` plus the test-suite registration block at `external/mod.rs` lines 11–14. | Do NOT render Go function names inline in the adapter (per D-05 — call `analysis::identity::render::go_relstring::render` instead, mirroring how the Jelly adapter call into `jelly_span::render` after this phase); if a `go_x_tools_callgraph.rs` already covers the slot, extend it rather than creating a parallel `go_rta.rs`. |
| `crates/polint/src/analysis_kernel/provider.rs` (modification) | register `polint.identity` manifest entry | self (lines 230–445) — the `PROVIDER_MANIFESTS: &[ProviderManifest] = &[ … ]` array; specifically the `polint.calls` entry at lines 399–422 is the structural template. | Add a new `ProviderManifest` literal immediately *after* `polint.calls` (per D-23 ordering); `id: "polint.identity"`, `kind: ProviderKind::WholeRepoDerived`, `inputs: &["source_files", "functions", "call_sites", "call_targets", "unresolved_calls"]`, `outputs: &["identity_records"]`, `language_scope: LanguageScope::MultiLanguage`, `cache_policy: CachePolicy::InMemoryDerived`, `schema_versions: IDENTITY_SCHEMA` (new const above the array, mirroring `CALLS_SCHEMA` at lines 180–183), `precision_ceiling: PrecisionCeiling::SetupAware`. Update the `provider_order_matches_behavior_preserving_kernel_sequence` test (line 713) to insert `"polint.identity"` between `"polint.calls"` and `"polint.abstract_domains"` in **every** ordering assertion in the file (lines 716, 743, 797, 1013, and any others). | Do NOT set `cache_policy: CachePolicy::ExistingFileFactCache` — identity is whole-repo-derived, not per-file. |

### Eval Reporting / Test Fixtures

| New file | Role | Closest analog | What to copy | What NOT to copy |
|---|---|---|---|---|
| `tests/eval-fixtures/identity/crlf_normalization/expected.polint-eval.toml` | CRLF↔LF snapshot fixture | `tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml` | The full TOML layout: `schema_version = "polint-eval-fixture-1"`, `case_id`, `area`, `[repo] path = "repo"`, `[budget] max_runtime_ms = 120000`, repeated `[[expected]] fact = { family = "...", stable_key = "...", mode = "partial", producer_id = "polint.identity", precision = "...", status = "..." }`. | Do NOT use `mode = "exact"` if the v1.2 fixtures use `"partial"` — keep consistency; do NOT hard-code line numbers that shift under CRLF — the whole point of D-13 is that the rendered span string is byte-identical across `\n` and `\r\n` checkouts, so the fixture asserts equality of two renderer outputs (the two `repo/` subdirs hold the same source, once `\n`-encoded and once `\r\n`-encoded). |
| `tests/eval-fixtures/identity/dedup/expected.polint-eval.toml` + `repo/` | dedup snapshot | `tests/eval-fixtures/direct-calls/core/` (whole dir) | The `repo/.polint.toml` config-stub layout; per-case `[[expected]]` rows asserting that two semantically-identical callsites collapse to one identity record with `multiplicity = 2`. | Do NOT include order-dependent assertions — the determinism gate (Phase 43) inherits this fixture; per D-11 fixtures must be stable across run order, file order, and provider order. |
| `tests/eval-fixtures/identity/jelly_oracle_coverage/expected.polint-eval.toml` | coverage fixture | `tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml` (TOML shape) + the Jelly micro suite enumeration in `crates/polint/src/eval/external/jelly_callgraph.rs::enumerate_jelly_callgraph_cases` (lines 170–201). | The `[[expected]]` per-fixture coverage rows; per D-20 the coverage assertion is `matched/total >= 0.99` on the Jelly micro suite the existing adapter already enumerates. | Do NOT roll a new fixture-enumeration path — reuse the existing Jelly adapter's case set; do NOT cross-average platforms (per D-22 Linux and macOS each pass independently). |
| `tests/public_surface_leak.rs` | workspace integration test (trybuild) | `crates/polint/tests/cli.rs` (zero-arg workspace integration test layout) + `crates/polint/src/sdk/mod.rs` lines 27–54 (the `prelude::*` allow-list source-of-truth). | The `#[test] fn public_surface_leak_allow_list_is_locked()` shape; a constant slice `const ALLOWED_PRELUDE: &[&str] = &[ "BranchId", "BranchObligation", … ]` matching the v1.0–v1.2 exports at `sdk/mod.rs:28–53`; `trybuild::TestCases::new().pass("tests/fixtures/public-surface-leak-probe/")`; per D-19 this allow-list is THE source of truth — adding to it requires a milestone-close review. | Do NOT pull `polint::analysis::*` items from inside the test — the whole point of the gate is the probe crate fails to compile if any leak; do NOT skip the test on any supported platform (D-18 Linux + macOS). |
| `tests/fixtures/public-surface-leak-probe/Cargo.toml` + `src/lib.rs` | trybuild probe crate | `crates/polint/src/sdk/mod.rs` `#[cfg(test)] mod tests` (lines 131–252) — the `use polint::sdk::prelude::*;` + `assert_exported::<T>()` pattern shows exactly which prelude items are reachable. | A minimal `Cargo.toml` with `[dependencies] polint = { path = "../../../crates/polint" }`; `src/lib.rs` containing `use polint::sdk::prelude::*;` (glob — per the "Specific Ideas" section in CONTEXT.md the glob maximises catch-rate); a `fn _assert<T>() {}` followed by `_assert::<RuleCtx>()`, `_assert::<FunctionFact>()` etc. for every allow-listed v1.0–v1.2 type. | Do NOT vendor a Cargo workspace `[workspace]` block in the probe crate (trybuild compiles it in isolation); do NOT reference any `polint::analysis::*`, `polint::analysis_kernel::*`, or `polint::core::AnalysisDb` from outside `polint::sdk::__private` — those are private and the gate exists exactly to catch their leakage. |

## Pattern Excerpts

### Pattern A: Fact-family struct shape with serde + dense ID newtype reference

**From:** `crates/polint/src/analysis/calls/facts.rs` lines 6–24

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CallSiteFact {
    pub(crate) id: CallSiteId,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) caller: FunctionId,
    pub(crate) owner_symbol: Option<SymbolId>,
    pub(crate) body: MirBodyId,
    pub(crate) operation: MirOpId,
    pub(crate) span: Span,
    pub(crate) kind: CallSyntaxKind,
    pub(crate) callee: CallCallee,
    pub(crate) status: CallTargetStatus,
    pub(crate) precision: CallPrecision,
    pub(crate) stable_key: String,
}
```

**Apply to:** `crates/polint/src/analysis/identity/facts.rs` — replicate visibility (`pub(crate)`), derives (`Debug, Clone, PartialEq, Eq, Serialize, Deserialize`), the dense `id: IdentityRecordId` newtype, the `language: LanguageTag`, `file_id: FileId`, `span: Span`, and a `stable_key: String` for fact-store dedup. Add `signature_digest: SignatureDigest` and `multiplicity: u32` per D-02/D-10.

### Pattern B: Closed enum vocabulary with `Copy + Ord + Hash`

**From:** `crates/polint/src/analysis/calls/facts.rs` lines 137–147

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallTargetStatus {
    Resolved,
    Ambiguous,
    Unresolved,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
    Rejected,
}
```

**Apply to:** `IdentityCategory` and `IdentityKind` and `LanguageTag` in `crates/polint/src/analysis/identity/facts.rs`. Exact derive set, exact `pub(crate)` visibility. No `Other` / catch-all variant on `IdentityCategory` per D-14.

### Pattern C: Dense ID newtype

**From:** `crates/polint/src/analysis/ids.rs` lines 27–31

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallSiteId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallTargetId(pub(crate) u64);
```

**Apply to:** `IdentityRecordId(pub(crate) u64)` in `crates/polint/src/analysis/identity/facts.rs` (D-01 forbids extending `analysis::ids` directly — keep new IDs co-located with the identity facts).

### Pattern D: Provider parameter digest with locked test

**From:** `crates/polint/src/analysis/calls/cache_key.rs` lines 1–22

```rust
use crate::analysis_kernel::incremental::{Digest, DigestKind};

pub(crate) fn calls_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "calls_provider_parameters",
        &[
            "calls-facts-1",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "direct_binding",
            // ...
        ],
    )
}
```

**Apply to:** `crates/polint/src/analysis/identity/cache_key.rs`. Use parts: `["identity-facts-1", "identity_records", "go_relstring_v1", "jelly_span_v1", "dedup_v1", "categorize_v1"]`. The renderer version strings (`go_relstring_v1`, `jelly_span_v1`) satisfy D-24: bumping these invalidates the identity cache when renderers change. Co-locate the exact-equality `#[cfg(test)]` test from calls/cache_key.rs lines 25–52.

### Pattern E: Provider `derive_*_with_cache_stats` entry point with output digest

**From:** `crates/polint/src/analysis/calls/provider.rs` lines 24–84

```rust
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_calls_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    /* ...more upstream digests... */
) -> CallsProviderOutput {
    let mut sites = extract_call_sites(db);
    let targets = resolve_direct_call_targets(db, &sites);
    /* ...filter, derive unresolved... */
    let output = CallOutput { sites, targets, unresolved }.normalized();
    let output_digest = calls_output_digest(db, manifest, input_snapshot, /* upstream digests */, &output);
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();
    /* match db.replace_call_facts(output) { Ok => ..., Err => Diagnostic } */
}
```

**Apply to:** `analysis::identity::provider::derive_identity_with_cache_stats`. Five-phase pipeline becomes: (1) extract identity records by projecting over `db.call_sites()`/`db.call_targets()`/`db.unresolved_calls()` plus `db.functions()`; (2) dedup via `analysis::identity::dedup`; (3) categorize via `analysis::identity::categorize`; (4) `IdentityOutput { records }.normalized()`; (5) `identity_output_digest(...)` mirroring `calls_output_digest`. The upstream digest argument list MUST include the v1.2 calls provider output digest (D-04, D-24).

### Pattern F: Output digest that locks stable keys, not dense IDs

**From:** `crates/polint/src/analysis/calls/provider.rs` lines 86–178

```rust
fn calls_output_digest(/* ... */, output: &CallOutput) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", calls_provider_parameter_digest()),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
        // ... more upstream digests ...
    ];
    parts.extend(output.sites.iter().map(|site| {
        format!("call_site={} language={:?} span={} kind={:?} ...", site.stable_key, ...)
    }));
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "calls_output", &refs)
}
```

**Apply to:** `analysis::identity::provider::identity_output_digest`. Per-record `parts.push(format!("identity_record={} language={:?} package_or_module={} container={} digest={} multiplicity={} ...", record.stable_key, ...))`. **The `signature_digest` must be hex-formatted, not `{:?}` (Debug on `[u8; 16]` differs by Rust version)** — use `hex::encode(record.signature_digest.0)`. Sort parts before passing to `Digest::from_parts` so order-independence is preserved.

### Pattern G: BTreeMap-keyed dedup with `.entry(...).or_insert(...)`

**From:** `crates/polint/src/analysis/calls/unresolved.rs` lines 44–63

```rust
fn insert_unresolved(
    rows: &mut BTreeMap<String, UnresolvedCallFact>,
    site: &CallSiteFact,
    reason: UnresolvedCallReason,
    evidence: &str,
) {
    let status = status_for_reason(reason);
    let stable_key = unresolved_stable_key(site, reason, status, evidence);
    rows.entry(stable_key.clone())
        .or_insert(UnresolvedCallFact { /* ... */ stable_key });
}
```

**Apply to:** `analysis::identity::dedup`. Use `BTreeMap<DedupKey, IdentityRecord>` with `DedupKey = (LanguageTag, Arc<str>, Arc<str>, SignatureDigest, Option<Span>)` (per D-09: `Span` is `Some` for in-file uniqueness, `None` for cross-file aliases). On the duplicate-hit branch (when `entry(...)` returns `Occupied`), increment `multiplicity` rather than overwriting — `.and_modify(|record| record.multiplicity += 1).or_insert_with(|| IdentityRecord { multiplicity: 1, ... })`.

### Pattern H: Closed enum exhaustive `match` projection (categorize)

**From:** `crates/polint/src/analysis/calls/unresolved.rs` lines 65–110 (`reason_for_site`)

```rust
fn reason_for_site(site: &CallSiteFact) -> Option<UnresolvedCallReason> {
    match &site.callee {
        CallCallee::FunctionValue { .. } => Some(UnresolvedCallReason::FunctionValue),
        CallCallee::Unknown { reason } => Some(normalize_unknown_reason(*reason)),
        CallCallee::Identifier { reference: None, .. }
        | CallCallee::Constructor { reference: None, .. } => Some(UnresolvedCallReason::MissingSemanticReference),
        CallCallee::Index { .. } => Some(UnresolvedCallReason::DynamicProperty),
        // ...
        _ => None,
    }
}
```

**Apply to:** `analysis::identity::categorize::category_for_unresolved`. **Replace the `_ => None` fallback with explicit per-variant arms** — D-14 says `IdentityCategory` has no `Other`, so every `UnresolvedCallReason` variant must map to exactly one `IdentityCategory` and a future new variant should produce a compile error.

### Pattern I: Store with `from_output` + dangling-reference validation

**From:** `crates/polint/src/analysis/calls/store.rs` lines 65–99

```rust
pub(crate) fn from_output(output: CallOutput) -> Result<Self, AnalysisError> {
    let output = output.normalized();
    let mut site_ids = BTreeSet::new();
    for site in &output.sites { site_ids.insert(site.id); }
    for target in &output.targets {
        if !site_ids.contains(&target.site) {
            return Err(AnalysisError::InvalidFact {
                provider: "polint.calls",
                reason: format!("dangling call site {:?} for target `{}`", target.site, target.stable_key),
            });
        }
    }
    /* ... build indexes ... */
}
```

**Apply to:** `analysis::identity::store::IdentityStore::from_output`. Validate that every `IdentityRecord`'s `originating_call_site_id` exists in `db.call_sites()` and `originating_call_target_id` (if present) exists in `db.call_targets()`. Use `provider: "polint.identity"` in the error.

### Pattern J: Validate pure function pushing diagnostics

**From:** `crates/polint/src/analysis/calls/validate.rs` lines 8–60

```rust
pub(crate) fn validate_calls(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    let files = db.files().iter().map(|row| row.id).collect::<BTreeSet<_>>();
    // ...
    check_duplicate_stable_keys(diagnostics, "CallSite", db.call_sites().iter().map(|row| row.stable_key.as_str()));
    for site in db.call_sites() {
        check_ref(diagnostics, &files, site.file, "CallSite", &site.stable_key, "file", "dangling call file reference");
        // ...
        if site.span.start_byte > site.span.end_byte {
            push_call_diagnostic(diagnostics, "CallSite", &site.stable_key, "span", "invalid span byte range");
        }
    }
}
```

**Apply to:** `analysis::identity::validate::validate_identity`. Check (a) duplicate `stable_key` across `IdentityRecord` rows, (b) dangling `file_id` against `db.files()`, (c) dangling `originating_call_site_id` against `db.call_sites()`, (d) span sanity (`start_byte > end_byte`, `start_line > end_line`), (e) `signature_digest` non-zero invariant.

### Pattern K: Jelly span format — current source of truth for the renderer

**From:** `crates/polint/src/eval/external/jelly_callgraph.rs` lines 324–339 + 283–310

```rust
// jelly_span_identity (cfg(test)) — to be replaced by the new renderer:
fn jelly_span_identity(files: &BTreeMap<FileId, &SourceFile>, case_dir: &Path, span: &Span) -> Option<String> {
    let file = files.get(&span.file)?;
    let relative_path = Path::new(&file.relative_path)
        .strip_prefix(case_dir)
        .unwrap_or_else(|_| Path::new(&file.relative_path))
        .to_string_lossy()
        .replace('\\', "/");
    Some(format!("{}:{}:{}:{}:{}", relative_path,
        span.start_line, span.start_col, span.end_line, span.end_col))
}
```

**Apply to:** `analysis::identity::render::jelly_span::render` — same exact `{}:{}:{}:{}:{}` format, same forward-slash path normalization, same workspace-relative path stripping (the new renderer takes the workspace root from the `SourceFile` borrowed reference instead of a `case_dir: &Path` param). **Add CRLF normalization** by walking `source.text` and re-deriving `start_line`/`start_col`/`end_line`/`end_col` from the byte span after `\r\n` → `\n` collapse — D-12 mandates renderer-time normalization.

### Pattern L: `KernelRunReport` extension shape

**From:** `crates/polint/src/analysis_kernel/incremental/run_report.rs` lines 7–34

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KernelRunReport {
    pub(crate) input_snapshot: InputSnapshot,
    pub(crate) provider_outputs: Vec<ProviderOutputMeta>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) demand_query_trace: DemandQueryTrace,
    #[cfg(test)]
    pub(crate) scc_closure_debug: Option<SccClosureDebugSnapshot>,
}
```

**Note:** This is the *kernel* run report. The *eval* report (`crates/polint/src/eval/report.rs::EvaluationRun` + `MetricSections`) is where the Phase 42 `categorized_failures` and `jelly_oracle_coverage` counters actually live (CONTEXT.md "Existing Code Insights" / "Integration Points" confirms this). Use the `MetricSections`-extension pattern from `crates/polint/src/eval/report.rs` lines 95–165 (Pattern M below) for the actual new sections.

### Pattern M: `MetricSections` sub-section with `deny_unknown_fields` and snake_case discriminants

**From:** `crates/polint/src/eval/report.rs` lines 140–155

```rust
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct UnknownMetricSection {
    pub(crate) total: u64,
    #[serde(default)]
    pub(crate) by_status: BTreeMap<String, u64>,
}
```

**Apply to:** add to `crates/polint/src/eval/report.rs`:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct CategorizedFailureSection {
    pub(crate) wrong_identity: u32,
    pub(crate) unsupported_edge: u32,
    pub(crate) unresolved_edge: u32,
    pub(crate) package_load_limitation: u32,
    pub(crate) model_missing: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct JellyOracleCoverageSection {
    pub(crate) matched: u32,
    pub(crate) total: u32,
    pub(crate) ratio: f64,
    pub(crate) unmatched: Vec<JellyUnmatchedSpan>,
}
```

Add both as `#[serde(default)]` fields on `MetricSections` (lines 95–107) — older consumers continue to deserialize without breakage.

### Pattern N: Provider manifest registration slot

**From:** `crates/polint/src/analysis_kernel/provider.rs` lines 399–422 (`polint.calls` entry) and lines 180–183 (`CALLS_SCHEMA` const).

```rust
const CALLS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "calls-facts-1",
    version: 1,
}];

// inside PROVIDER_MANIFESTS:
ProviderManifest {
    id: "polint.calls",
    kind: ProviderKind::WholeRepoDerived,
    inputs: &["source_files", "functions", /* ... */, "cfg_edges"],
    outputs: &["call_sites", "call_targets", "unresolved_calls"],
    language_scope: LanguageScope::MultiLanguage,
    cache_policy: CachePolicy::InMemoryDerived,
    schema_versions: CALLS_SCHEMA,
    precision_ceiling: PrecisionCeiling::SetupAware,
},
```

**Apply to:** insert a new `IDENTITY_SCHEMA` const + a new `ProviderManifest` literal *immediately after* the `polint.calls` entry, with `id: "polint.identity"`, `inputs: &["source_files", "functions", "call_sites", "call_targets", "unresolved_calls"]`, `outputs: &["identity_records"]`. Update **every** `provider_order_*` test assertion in the file (lines 716, 743, 797, 1013, and any matching pattern) to insert `"polint.identity"` between `"polint.calls"` and `"polint.abstract_domains"`.

### Pattern O: SDK public-surface allow-list anchor (for the leak gate)

**From:** `crates/polint/src/sdk/mod.rs` lines 27–54

```rust
pub mod prelude {
    pub use crate::core::{
        BranchId, BranchObligation, CapabilitySupport, CapabilitySupportStatus,
        CapabilitySupportView, ComplexityMetricFact, CoverageFact, DefinitionFact, DefinitionId,
        /* ...many more... */
        UnresolvedReason,
    };
    pub use crate::diagnostics::{ /* ... */ };
    pub use crate::rule_error::{RuleError, RuleResult};
    pub use crate::sdk::collect_go_tests;
    pub use crate::sdk::facts::{ /* ... */ };
    pub use crate::sdk::scope::{file_in_scope, file_matches_globs, glob_matches};
}
```

**Apply to:** `tests/public_surface_leak.rs` allow-list constant — copy the **exact** set of identifiers from this `prelude` block as the locked source-of-truth list (D-19). The trybuild probe at `tests/fixtures/public-surface-leak-probe/src/lib.rs` does `use polint::sdk::prelude::*;` (glob) and emits `let _ = <T>::default;` or equivalent witness for each allow-listed identifier; any v1.3-private type leaking into the glob fails the probe compile, which fails the workspace test.

## Anti-Patterns to Avoid

- **Do NOT extend `crates/polint/src/analysis/ids.rs`** — D-01 says new IDs (`IdentityRecordId`) live inside `analysis::identity::facts.rs`, alongside the records they identify. The `ids.rs` file stays focused on legacy raw integer ID newtypes.
- **Do NOT promote anything to `polint::sdk::prelude::*`** — the v1.3 milestone (ROADMAP, REQUIREMENTS, CONTEXT D-19, REQUIREMENTS BENCH-01) forbids new public SDK promotion. The leak gate this phase introduces explicitly catches violations.
- **Do NOT duplicate the renderer logic in `crates/polint/src/eval/external/`** — D-05 mandates the renderer in `analysis::identity::render::*` is the single source of truth. Both Jelly and Go RTA adapters call into it. The `#[cfg(test)] fn jelly_span_identity` currently in `jelly_callgraph.rs` lines 324–339 must be deleted and replaced by the new renderer call.
- **Do NOT normalize CRLF at file-load time** — D-12 mandates normalization happens *inside* the Jelly renderer, computed from the raw `SourceFile` text. On-disk byte spans stay byte-true so v1.2 facts remain unchanged.
- **Do NOT use `_ => …` wildcards on closed source enums** when categorizing — D-14's closed `IdentityCategory` only stays exhaustive if every source-enum variant maps explicitly. A wildcard hides the day a new `UnresolvedCallReason` is added and silently falls through.
- **Do NOT include dense IDs in the output digest** — every v1.2 provider digests by `stable_key`, not by `*Id`. The test `calls_output_digest_uses_stable_payloads_not_dense_ids` at `provider.rs:352–379` is the contract; add an equivalent `identity_output_digest_uses_stable_payloads_not_dense_ids` test.
- **Do NOT format `[u8; 16]` with `{:?}`** — Rust array Debug formatting is stable but verbose and platform-independent in practice; nevertheless, the digest should explicitly use `hex::encode(...)` so the byte form in the digest string is human-inspectable and cross-platform identical (D-25 cross-platform byte-identical contract).
- **Do NOT add probabilistic sampling for Jelly oracle coverage** — D-20 mandates a deterministic count over the full fixture set. `matched / total >= 0.99` against the existing `enumerate_jelly_callgraph_cases` output, no shortcuts.
- **Do NOT couple the new `Go RTA` adapter to identity at the field level** — D-04 says identity *references* the v1.2 call IDs by composition. The Go adapter still reads call-graph oracles the same way Jelly does; it just calls `go_relstring::render(&identity)` instead of formatting names inline.
- **Do NOT change `MetricSummary` field layout** — extend via `MetricSections` only. Downstream gates (Phase 43+) and existing v1.2 evaluation JSON consumers lock the current shape.
- **Do NOT include volatile inputs in the provider parameter digest** — only `&'static str` parts, sorted. Anything dynamic (timestamps, host paths, env vars) goes in the *output* digest's `input_snapshot.config.digest` channel, not the parameter digest.

## PATTERN MAPPING COMPLETE
