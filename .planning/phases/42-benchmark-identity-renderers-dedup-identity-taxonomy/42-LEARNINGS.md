---
phase: 42
phase_name: "benchmark-identity-renderers-dedup-identity-taxonomy"
project: "polint"
generated: "2026-05-29"
counts:
  decisions: 10
  lessons: 9
  patterns: 8
  surprises: 6
missing_artifacts:
  - "42-UAT.md"
---

# Phase 42 Learnings: benchmark-identity-renderers-dedup-identity-taxonomy

## Decisions

### FNV-1a 16-byte SignatureDigest instead of SHA-256 (no new deps)
The plan specified `sha2` + `hex` crates with `#[serde(with = "hex::serde")]`, but neither is a workspace dependency and T-42-SC forbids introducing new third-party deps. The digest was implemented as a deterministic length-prefixed two-pass FNV-1a 16-byte digest with a local hex encode/decode codec.

**Rationale:** Avoids new third-party deps (T-42-SC); stays cross-platform byte-identical (D-25); length-prefixing every field component disambiguates boundaries (T-42-01); matches the existing `cache::stable_hash` FNV-1a convention in the codebase.
**Source:** 42-01-SUMMARY.md

### Arc<str> serde via a field-level adapter
`IdentityRecord` carries `Arc<str>` fields (`package_or_module`, `container_path`, `display_name`), which do not round-trip through plain serde derive without the serde `rc` feature. A field-level serde adapter serializes/deserializes `Arc<str>` as a plain string.

**Rationale:** The serde `rc` feature is not enabled in the workspace and enabling it crate-wide is a larger blast radius than a local field adapter.
**Source:** 42-01-SUMMARY.md

### Order-independent dedup: canonical retained record is the smallest by sort key
On a duplicate dedup-key hit, `dedup_identity_records` retains the smallest record by canonical sort key (incrementing multiplicity) rather than first-insert-wins, so the collapsed record is byte-identical regardless of run/file/provider order.

**Rationale:** First-insert-wins makes the retained record depend on input/iteration order, breaking the D-11 byte-stable contract the Phase 43 determinism gate inherits.
**Source:** 42-01-SUMMARY.md

### Dedup fixture asserts live multiplicity=1; collapse-to-2 proven by unit tests
The Go fixture repo has no true semantic duplicates, so the eval fixture asserts the live deterministic `multiplicity = 1` (with an added `identity.dedup.multiplicity` eval observation so the row is genuinely observed); the `multiplicity = 2` collapse contract is proven by co-located dedup/provider unit tests instead.

**Rationale:** A literal `multiplicity = 2` fixture assertion would pin a value the live provider never emits on the Go repo; in-file callsites keep their span and stay distinct (D-09). Keeping the fixture free of order-dependent assertions preserves the Phase 43 determinism gate.
**Source:** 42-01-SUMMARY.md

### IdentityCategory is a closed five-variant enum with #[repr(u8)] pinned ordinals
`IdentityCategory { WrongIdentity=0, UnsupportedEdge=1, UnresolvedEdge=2, PackageLoadLimitation=3, ModelMissing=4 }` is declared in pinned source order with `#[repr(u8)]` explicit discriminants and snake_case serde; no `Other`/`Unknown`, no `#[non_exhaustive]`. A variant-order lock test casts each variant to `u8` and asserts its pinned ordinal.

**Rationale:** Declaration order defines both serde discriminant order and `Ord` ordering, which propagates into any BTreeMap keyed on the enum (D-25 byte stability). `#[repr(u8)]` makes the byte-stability contract mechanically enforceable; the closed taxonomy forces every classification to be deliberate and turns a new category into a milestone-review change (D-14).
**Source:** 42-03-SUMMARY.md

### Exhaustive no-wildcard categorize projections over upstream fact enums
`category_for_unresolved` (17 `UnresolvedCallReason` variants) and `category_for_unsupported` (7 `CallTargetStatus` variants) are exhaustive `match`es with no `_ =>` wildcard arm, mapping every variant explicitly to one category; `categorize` is a tag on existing facts (`CategorizeReason`) and introduces zero new fact families.

**Rationale:** A wildcard arm would silently absorb a future upstream variant addition; without it, adding a variant upstream becomes a compile error forcing a deliberate categorization decision (Pattern H). Keeping it a per-fact tag (D-16) keeps it composable without inventing new fact families.
**Source:** 42-03-SUMMARY.md

### MetricSections extended additively; MetricSummary shape frozen
Both `jelly_oracle_coverage` (Plan 02) and `categorized_failures` (Plan 03) are added to `MetricSections` as `#[serde(default)]` sibling fields (categorized_failures placed after jelly_oracle_coverage). `MetricSummary`'s 26-field shape is left untouched and locked by a destructure-every-field test with no rest pattern.

**Rationale:** `#[serde(default)]` keeps the JSON contract additive so older v1.2 consumers deserialize without breakage; downstream Phase 43+ gates lock `MetricSummary`, so all Phase 42 reporting extension lives on `MetricSections` only.
**Source:** 42-02-SUMMARY.md, 42-03-SUMMARY.md

### Public-surface-leak gate via no_implicit_prelude probe + direct cargo (Approach B, no trybuild)
The gate is an excluded probe crate with `#![no_implicit_prelude]` + a single `use ::polint::sdk::prelude::*;` glob + 97 witness fns, plus a workspace integration test that shells out `cargo build` on the probe and snapshot-diffs the parsed prelude block against a locked `ALLOWED_PRELUDE` (97 entries). trybuild (Approach A) was deliberately not added.

**Rationale:** Approach B avoids a new third-party dependency, consistent with the Plans 01/02 no-new-deps discipline (T-42-SC), and direct cargo `--message-format` already gives rustc-level granularity. The probe glob maximizes catch-rate; `no_implicit_prelude` ensures only prelude-reachable identifiers are nameable.
**Source:** 42-04-SUMMARY.md

### Go RTA oracle key stays on display_name; package-NAME-only RelString
Plan 05 resolves Go records' `package_or_module` to the `PackageFact` package-clause NAME (`foo.Bar`), but the Go x/tools RTA oracle key intentionally stays derived from `display_name` with an inline Phase 46 deferral note. Full module import-path RelString (`module/path/pkg.Func`) is out of scope.

**Rationale:** The v1.2 substrate's `FunctionFact`/`PackageFact` carry only the package-clause name, not the import path. Routing the package-name-only RelString into the oracle key would regress benchmark matching against the x/tools RTA bare-name `WANT:` oracle; the full import path requires the Phase 46 `go/packages`+`go/ssa` semantic frontend.
**Source:** 42-05-SUMMARY.md, 42-02-SUMMARY.md

### Cache trip-wire bumped go_relstring_v1 -> go_relstring_v2 on Go package_or_module change
When Plan 05 switched the Go `package_or_module` from the file path to the package name, the renderer/provider parameter digest part `go_relstring_v1` was bumped to `go_relstring_v2` in both the digest fn and its locked test, plus a differs-from-pre-bump test.

**Rationale:** Changing the Go `package_or_module` content changes the IdentityRecord bytes, signature digest, and provider output digest; bumping the trip-wire forces deterministic cache invalidation so stale cached identity records are not reused (D-24).
**Source:** 42-05-SUMMARY.md

---

## Lessons

### Synthetic-record tests can pass vacuously while the real provider feeds wrong data
Every `go_relstring.rs` unit test passed because it hand-constructed records with `package_or_module = "module/path/pkg"`, but the real provider fed `db.path_for(file)` (the file path) for all languages, so real Go records rendered `src/main.go.Foo`. The defect was masked because the renderer output was computed and discarded (`let _ = ...`) at every call site. It surfaced only in code review (CR-01), not in the green test suite.

**Context:** Plan 05 closed the gap by adding a provider-level test that runs `derive_identity_with_cache_stats` over a genuine Go `FunctionFact`+`PackageFact` and asserts `foo.Bar` — not a synthetic record.
**Source:** 42-REVIEW.md, 42-05-SUMMARY.md

### Discarded renderer output (`let _ =`) hides a contract from its own tests
Both `go_relstring::render` call sites bound the result to `let _` / `let _rel_string`, so the renderer ran purely for side effect. This decoupled the renderer's correctness from any assertion and let CR-01 stay invisible. Plan 05 replaced the discards with `assert!`/`debug_assert!` non-empty checks so the output is genuinely used.

**Context:** A render call site that discards output is a "vacuously exercised" smell; asserting even a weak invariant (non-empty) reconnects the test to the code path.
**Source:** 42-REVIEW.md, 42-05-SUMMARY.md

### A sort key that excludes identity-bearing fields breaks total order on ties
Dedup canonical selection replaced the record only when `record_sort_key(new) < record_sort_key(existing)`, but `record_sort_key` excluded `originating_call_site_id`/`signature_digest`. Two records sharing a dedup key AND a sort key but differing in `originating_call_site_id` resolved order-dependently (first-inserted wins). Plan 05 extended the comparison into a literal total order (`record_total_order_key`) applied to both collision selection and final sort.

**Context:** The fix had to apply the same key to BOTH the canonical-selection compare and the final `sort_by_key` so the retained record and output order always agree (CR-03 / Phase 43 byte-stability dependency).
**Source:** 42-REVIEW.md, 42-05-SUMMARY.md

### Verification can pass overall yet a SUMMARY can overstate what was delivered
42-02-SUMMARY originally claimed "IDENT-02 fully addressed: both renderers exist with the exact Go RelString and Jelly span formats." The Go half was format-correct in unit tests but wrong on real data. Plan 05 reconciled the SUMMARY wording to state the Jelly half is fully delivered end-to-end while the Go half is package-NAME-qualified with the full import path deferred to Phase 46.

**Context:** The gaps_found -> gap-closure -> passed loop caught this; a SUMMARY's "fully addressed" claim is not self-validating and benefits from adversarial code review against real data flow.
**Source:** 42-VERIFICATION.md, 42-05-SUMMARY.md, 42-REVIEW.md

### Native syntactic analysis cannot emit every failure category — probe the live DB before pinning a fixture
The plan assumed four of five `IdentityCategory` counters would fire from a native Go/TS fixture. Empirical probing of the live `AnalysisDb` showed the syntactic frontend only ever emits `Reflection`/`Eval` (UnsupportedEdge) and `DynamicProperty`/`MissingSemanticReference` (UnresolvedEdge); `wrong_identity` needs a benchmark oracle, `package_load_limitation` needs `SetupMissing`, and `model_missing` needs `Rejected` — none of which native source produces.

**Context:** Plan 03 honored the BLOCKER #4 fallback: the fixture asserts the two genuinely-emitted categories from real source, and `eval::metrics` unit tests construct synthetic facts and drive the real `categorized_failures_from_db` projection for the other three, so all five counters are proven non-zero without fabricating analysis output.
**Source:** 42-03-SUMMARY.md

### `#![no_implicit_prelude]` requires a leading `::` on the extern-crate path
With `#![no_implicit_prelude]`, the implicit extern-prelude is disabled, so a bare `use polint::sdk::prelude::*;` failed with `E0433: cannot find module or crate polint` (cascading to ~97 "cannot find type" errors). The fix was `use ::polint::sdk::prelude::*;` (leading `::`).

**Context:** This is exactly the strictness that makes the probe trustworthy — only items reachable through the explicit prelude glob (or `::core`/`::std` absolute paths) are nameable. The redundancy test accepts both `::polint` and bare `polint` forms while still enforcing exactly one prelude-glob import.
**Source:** 42-04-SUMMARY.md

### An excluded probe crate needs its own committed Cargo.lock for --locked CI builds
The workspace-excluded probe is an independent crate with no lock file, so `cargo build --locked` failed (`cannot create the lock file ... because --locked was passed`). A `Cargo.lock` was generated and committed under the probe dir (its `target/` is gitignored).

**Context:** Workspace exclusion keeps the probe out of normal builds/lints/tests, but exclusion also means it does not participate in the workspace lock file, so it needs its own for deterministic `--locked` CI.
**Source:** 42-04-SUMMARY.md

### A workspace-root tests/ dir is not a crate — integration tests must live under crates/<pkg>/tests/
The plan placed the leak gate at `tests/public_surface_leak.rs` (workspace root), but `cargo test --package polint --test public_surface_leak` only resolves an integration-test target under `crates/polint/tests/`. The test was relocated and uses `CARGO_MANIFEST_DIR` to reach the probe and `sdk/mod.rs`.

**Context:** The plan explicitly authorized this relocation if `--test` resolution required it; the lesson is that `--test <name>` is scoped to a crate's `tests/` directory, not the workspace root.
**Source:** 42-04-SUMMARY.md

### The pre-commit lint gate runs cargo fmt --check, which a build+clippy-only verification misses
Plan 01's executor verified with `cargo build` + `cargo clippy` but not `cargo fmt --check`; on resume the pre-commit `make lint` rejected the commit due to import-ordering/line-wrapping differences in the new identity files. Running `cargo fmt -p polint` (cosmetic-only) and re-committing passed the hook. Separately, Plan 05 hit `cargo fmt` reflowing edits mid-task, invalidating an Edit's `old_string` match.

**Context:** Include `cargo fmt --check` in task verification (not just build + clippy) to avoid commit-time surprises; expect the formatter to reflow regions after refactors and re-read before re-editing.
**Source:** 42-01-SUMMARY.md, 42-05-SUMMARY.md

---

## Patterns

### Length-prefixed two-pass FNV-1a digest with local hex codec
Length-prefix every field component (`(len as u32).to_le_bytes()` then bytes) before hashing through two seeded FNV-1a passes into `[u8; 16]`, serialized as 32-char lowercase hex via a local codec.

**When to use:** Whenever a fact family needs a deterministic, cross-platform byte-identical signature/identity digest at repo scale without a cryptographic-hash dependency, and field boundaries must be unambiguous. (Note WR-05: the `u32` length prefix theoretically wraps above 4 GiB; use `u64` if fields can be that large.)
**Source:** 42-01-SUMMARY.md, 42-REVIEW.md

### Dense IDs assigned only after sort + dedup; output digest keyed on stable_key never dense IDs
The provider pipeline runs extract -> dedup -> sort -> assign dense `IdentityRecordId` -> compute output digest, and the digest's per-record parts use `stable_key` (and stable payload fields), sorted, never the dense `IdentityRecordId.0`. A locked test renumbers IDs while preserving stable keys and asserts the digest is identical (Pattern F).

**When to use:** Any deterministic provider where dense run-local IDs are convenient handles but must not leak into a cache/output digest, so that ID renumbering across runs does not falsely invalidate the cache.
**Source:** 42-01-SUMMARY.md

### Cache trip-wire version strings in the provider parameter digest
The identity provider parameter digest embeds renderer/logic version tokens (`go_relstring_v2`, `jelly_span_v1`, `dedup_v1`, `categorize_v1`) in its parts list, with a locked exact-equality test mirroring the list. Changing renderer logic requires bumping the token, which trips the locked test until done.

**When to use:** When downstream code-version changes (e.g. a renderer's logic) must deterministically invalidate a cache; the locked-parts-list test is the intended trip-wire that forces a deliberate version bump.
**Source:** 42-01-SUMMARY.md, 42-05-SUMMARY.md

### Closed taxonomy enum with #[repr(u8)] pinned ordinals + variant-order lock test
Declare a closed enum with explicit `#[repr(u8)]` discriminants in pinned source order, derive `Ord`/serde from declaration order, and add a lock test that casts each variant to `u8` and asserts its pinned ordinal — no `Other`/`Unknown`, no `#[non_exhaustive]`.

**When to use:** When an enum's declaration order is itself a byte-stability boundary (serde discriminant order, `Ord`, BTreeMap key order) and accidental reordering or catch-all-variant additions must fail the build.
**Source:** 42-03-SUMMARY.md

### Exhaustive no-wildcard projection over an upstream closed enum
Map every variant of an upstream fact enum to a target via a `match` with one explicit arm per variant and no `_ =>` wildcard, so a new upstream variant becomes a compile error rather than a silent default; lock the mapping in tests as the audit contract.

**When to use:** When a downstream classification must stay correct as upstream evolves, and you want the type system (not a runtime default) to flag every new upstream case.
**Source:** 42-03-SUMMARY.md

### Additive report extension: #[serde(default)] sibling field + frozen-summary destructure lock
Extend a serialized report struct by appending a `#[serde(default)]` field (preserving prior field iteration order) and prove the locked sibling struct is unchanged with a destructure-every-field test that omits the rest pattern, so adding/removing a field fails to compile.

**When to use:** When extending a JSON report consumed by older readers — `#[serde(default)]` keeps deserialization backward-compatible while the destructure lock prevents accidental schema drift on the frozen part.
**Source:** 42-02-SUMMARY.md, 42-03-SUMMARY.md

### Public-surface-leak probe: excluded crate + no_implicit_prelude + single prelude glob + witnesses
A workspace-excluded probe crate with `#![no_implicit_prelude]` and exactly one `use ::<crate>::sdk::prelude::*;` glob, with one `PhantomData::<Type>`/value-binding witness per allow-listed identifier; a workspace integration test compiles the probe, snapshot-diffs the parsed prelude block against a source-of-truth `ALLOWED_PRELUDE` constant, and includes a parser self-test (synthetic-leak negative control + clean positive control) plus a probe-tamper redundancy check.

**When to use:** To mechanically freeze a public API surface so any unsanctioned `pub use` into the prelude (or any probe tampering) hard-blocks CI. Two-layer enforcement: Rust's E0365 physically forbids re-exporting a `pub(crate)` type into a public prelude; the snapshot diff catches a genuinely-`pub` type added to the prelude.
**Source:** 42-04-SUMMARY.md

### Byte-stable fixture assertions via .nonzero booleans, not exact counts
For determinism-gate-safe fixtures, the observation layer emits both the exact count invariant (rehydrated into the report) AND a `.nonzero` boolean per metric; the fixture asserts the order-stable booleans rather than brittle exact counts that drift with internal analysis details.

**When to use:** When a fixture must prove a capability fires (e.g. a category counter is non-zero) without pinning a value that future analysis refinements would churn, and the fixture is inherited by a downstream byte-stability/determinism gate.
**Source:** 42-03-SUMMARY.md

---

## Surprises

### Disk-full (ENOSPC) interruption blocked the final commit after work was already green
A transient disk-full condition during Plan 01 execution prevented the final Task 2 commit, SUMMARY, and tracking updates — even though all Task 2 code was complete and verified green (cargo build zero warnings, clippy clean, 1583 lib tests incl. 28 identity tests, dedup fixture 4 passed). On resume (~25-28 GiB free), the work was persisted unchanged except for a cosmetic `cargo fmt` deviation; no re-implementation was needed.

**Impact:** Stretched Plan 01 to an 8h 9m duration spanning two sessions; recoverable because the work was complete before the interruption, but the commit/persistence step is a single point of failure independent of code correctness.
**Source:** 42-01-SUMMARY.md

### sha2 is NOT a workspace dependency — the planned digest backend was unavailable
The plan confidently specified `sha2` + `hex` for the signature digest (and the threat model assumed they were existing workspace deps), but neither was actually present, forcing the FNV-1a fallback during execution.

**Impact:** A blocking deviation in Plan 01; resolved cleanly via FNV-1a but illustrates that "already a workspace dep" assumptions in a plan must be verified at execution time. Code review (WR-04) later confirmed sha2 was never added, so the FNV choice keeps T-42-SC honored.
**Source:** 42-01-SUMMARY.md, 42-REVIEW.md

### The prelude exported 97 identifiers, not the planning anchor's "~85" estimate
Programmatic extraction from `sdk/mod.rs:28-53` yielded exactly 97 allow-list entries vs the plan's `<interfaces>` estimate of ~85. The count is now locked at 97 with an explicit `assert_eq!` so any drift fails loudly.

**Impact:** Harmless (the acceptance threshold was `>= 80`), but a reminder that hand-estimated counts in plans drift from the source of truth; the count-lock assertion converts the count into a deliberate-change tripwire for Phases 43-54.
**Source:** 42-04-SUMMARY.md

### The Jelly renderer silently replaced precomputed span line/col with byte-offset re-derivation
The new `jelly_span::render` ignores `identity.span.{start_line,start_col,...}` and recomputes line/column from byte offsets over `source.source`, counting columns per UTF-8 character. Code review (CR-02) flagged that if the analysis's stored line/col used a different convention (tab expansion, 0- vs 1-based, UTF-16 vs codepoint, BOM, half-open vs inclusive), real Jelly spans would silently diverge — and the single micro-fixture coverage test (1 case, ratio >= 0.99) could pass while the broader suite regresses.

**Impact:** A HIGH-severity correctness concern that no in-scope test caught; broad multi-fixture Jelly coverage is deferred to Phase 45, leaving the column-convention risk test-guarded only by one micro case.
**Source:** 42-REVIEW.md, 42-VERIFICATION.md

### Provider output digest mixes a stable label (language) with Debug formatting (kind)
The per-record output-digest part uses the stable `language.as_str()` for one field but `kind={:?}` (Rust `Debug`) for `IdentityKind`. WR-07 flagged that the digest input therefore depends on a `Debug` impl — stable today but not a documented serialization contract; a future derive/rename could silently change the cross-platform digest. (The plan's Pattern F text itself specified `{:?}` here.)

**Impact:** Low-severity but an inconsistency baked into the locked digest contract; coupling a digest to `Debug` is fragile and was carried straight from the plan into the code.
**Source:** 42-REVIEW.md

### Renderer purity (no case_dir param) caused 0/3 oracle matches until the oracle JSON path was realigned
Removing the `case_dir` parameter for renderer purity (D-06) meant the renderer emits workspace-relative paths (`tests/micro/app.js`), but the oracle JSON's `case_dir`-relative `files` array (`app.js`) matched 0/3 spans on the first run. The fix was to author the oracle `files` array with the workspace-relative path so endpoint spans align with renderer output.

**Impact:** A bug-class deviation in Plan 02 surfaced only when the fixture ran; the eval edge-key alignment became the fixture's responsibility rather than the renderer's, a direct (and initially surprising) consequence of the purity decision.
**Source:** 42-02-SUMMARY.md
