---
phase: 43-reachability-roots-per-suite-scoring-mode
reviewed: 2026-05-29T00:00:00Z
depth: standard
files_reviewed: 24
files_reviewed_list:
  - crates/polint/src/analysis/reachability/mod.rs
  - crates/polint/src/analysis/reachability/facts.rs
  - crates/polint/src/analysis/reachability/discover.rs
  - crates/polint/src/analysis/reachability/traverse.rs
  - crates/polint/src/analysis/reachability/provider.rs
  - crates/polint/src/analysis/reachability/store.rs
  - crates/polint/src/analysis/reachability/validate.rs
  - crates/polint/src/analysis/reachability/cache_key.rs
  - crates/polint/src/analysis/reachability/debug.rs
  - crates/polint/src/analysis/ids.rs
  - crates/polint/src/analysis/mod.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/config/mod.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/eval/suite.rs
  - crates/polint/src/eval/runner.rs
  - crates/polint/src/eval/metrics.rs
  - crates/polint/src/eval/report.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/eval/determinism_gate.rs
  - crates/polint/src/eval/fixtures.rs
  - crates/polint/src/eval/adapter.rs
  - crates/polint/src/eval/mod.rs
findings:
  critical: 1
  warning: 6
  info: 5
  total: 12
status: issues_found
---

# Phase 43: Code Review Report

**Reviewed:** 2026-05-29
**Depth:** standard
**Files Reviewed:** 24
**Status:** issues_found

## Summary

Phase 43 introduces the whole-program `polint.reachability` provider (roots discovery + BFS marking), a per-suite `ScoringMode` gate, and an N=10 determinism gate. The architecture is disciplined: determinism is consistently routed through `BTreeMap`/`BTreeSet` and sort-then-assign-dense-IDs, stable keys are escape-protected, the mode-aware scoring filter is fail-closed for unmarked edges, and visibility is correctly `pub(crate)` throughout (no SDK prelude leak). The closed enums use pinned declaration order, and the `scoring_mode` required-field gate is layered (structural `deny_unknown_fields`/non-`Option` + explicit `validate()`).

The BFS traversal in `traverse.rs` is correct: cycle handling is sound (the `reachable.insert()` guard prevents re-enqueue), frontier ordering is deterministic, and only-resolved-edge semantics are enforced. The mode filter is not backwards.

The one BLOCKER is a real correctness divergence: the cache **output digest is computed over a different fact set than what is stored**, so a cache hit can replay a digest that does not correspond to the persisted facts — and worse, the digest includes run-local dense IDs in direct contradiction of its own D-19 contract, which is the actual nondeterminism/staleness surface. Several WARNINGs concern honesty of `Resolved` status labels, a real-roots filter that silently swallows store-rejection errors, and a duplicate-config-root collision.

## Critical Issues

### CR-01: Output digest is computed over `output` (all roots, with dense IDs) but only `real_roots` are stored — digest/store divergence and self-contradicting D-19 "never dense IDs"

**File:** `crates/polint/src/analysis/reachability/provider.rs:58-104` (digest at `68-77`, `145-156`; store at `87-92`)

**Issue:** Two coupled defects in the provider pipeline:

1. **Digest/store set divergence.** The digest is built from `output` — the `normalized()` set containing **every** discovered root, including configured-unresolvable sentinel roots (`UNRESOLVED_TARGET = FunctionId(u64::MAX)`). But the persisted `storable` set is `real_roots` only (roots whose target is a real function). The output digest therefore keys the cache on a fact set that is strictly larger than what lands in the db. On a cache hit, the kernel will treat a stored (smaller) fact set as valid for a digest derived from the larger set. Two repos that differ only in their *unresolvable* configured roots produce different digests but identical stored facts, and conversely the digest no longer certifies the stored content. The digest must be computed over exactly the facts that are stored.

2. **Dense IDs in the digest payload, contradicting the stated contract.** `reachability_output_digest` runs **after** `for (index, root) in output.roots.iter_mut().enumerate() { root.id = ReachabilityRootId(index as u64); }` (lines 63-65), and `stable_fact_payload` (lines 175-180) does `serde_json::to_string(fact)`. `ReachabilityRootFact.id` has no `#[serde(skip)]` (`facts.rs:18`), so the dense `id` is serialized into every `root=...` digest part. The doc comments at lines 28-29, 66 ("digest over stable payloads (never dense IDs)") and 106 ("never dense IDs (D-19)") are violated by the implementation. Because the configured-unresolvable roots are *included* in `output` but *excluded* from `storable`, the dense-ID assignment over `output` shifts every subsequent root's ID relative to the stored set, so the digest's embedded IDs do not even match the IDs the stored facts will carry after `replace_reachability_facts` re-derives them.

This is the seam the whole phase is supposed to protect (D-06/D-19): the digest must be a function of stable payloads of the stored facts only. As written it folds in run-local IDs and a non-stored superset.

**Fix:** Compute the digest over the stored set, and exclude dense IDs from the payload.

```rust
// Build the storable set FIRST, normalize it, then digest THAT.
let storable = ReachabilityProviderOutput { roots: real_roots, marks }.normalized();

// Either add `#[serde(skip)] id` to ReachabilityRootFact, or strip it before payload:
fn stable_fact_payload<T: Serialize + Debug>(fact: &T) -> String { /* serialize a
    projection that omits `id`, or use a dedicated stable-payload method */ }

let output_digest = reachability_output_digest(/* ... */, &storable);
// assign dense IDs only as a post-store read concern, not before the digest.
db.replace_reachability_facts(storable)?;
```
If the intent is genuinely to fold unresolvable configured roots into the digest (so the cache invalidates when they change), do it via a dedicated `unresolved_configured=<count/keys>` part built from stable keys, not by serializing whole facts with dense IDs into the `root=` parts. Either way the `root=` parts must carry no `id`.

## Warnings

### WR-01: `real_roots` filter discards the store-rejection error path it was meant to preserve; configured-unresolvable roots are silently dropped from the stored facts despite the "never a silent drop" contract

**File:** `crates/polint/src/analysis/reachability/provider.rs:50-92`

**Issue:** The phase contract (D-13, echoed in `discover.rs:13-16` and the provider comment at `82-86`) says unresolvable configured roots must be "honest `RootStatus::Unresolved` rows ... never a silent drop." The provider discovers them, but then `storable` is built from `real_roots` only — the unresolvable rows never reach `db.reachability_roots()`. The justification ("the referential store rejects the sentinel target") is real, but the consequence is that the only place these honest `Unresolved` rows are reportable is the now-divergent digest (see CR-01) and the discovery return value, which the provider drops. Downstream consumers reading `db.reachability_roots()` see no evidence the configured root was unresolvable. This is a silent drop at the db boundary, contradicting the stated invariant. There is no diagnostic emitted for an unresolvable configured root either.

**Fix:** Emit a diagnostic per unresolvable configured root (so it is reported, mirroring `validate_reachability`'s diagnostic discipline), or carry the unresolved rows in a separate non-referentially-validated field on the store. At minimum, document precisely where an operator can see that their configured root failed to resolve, and add a test asserting it is observable.

### WR-02: Configured roots labeled `RootStatus::Resolved` / `RootPrecision::SetupAware` with no honest-precision basis — resolution is a bare trailing-identifier name match

**File:** `crates/polint/src/analysis/reachability/discover.rs:161-216, 285-303`

**Issue:** `resolve_configured_function` resolves `"pkg/path.Func"`, `"src/x.ts#handler"`, or a bare name by taking only the **trailing identifier** (`configured_function_name`) and matching it against `FunctionFact.name` with `db.functions().iter().find(...)` — first match wins. The path/package prefix (`pkg/path`, `src/x.ts`) is entirely discarded. Consequences:

- A config entry `"pkg/a.Handle"` resolves to *any* function named `Handle` in the repo, even one in `pkg/z`. The matched function may be the wrong one, yet the root is stamped `RootStatus::Resolved` and `RootProvenance::Configured` with full confidence. This is exactly the "no fabricated Resolved roots" footgun the phase calls out: a name-only match across the whole repo is not a resolved-static fact.
- If multiple functions share the name, `find` returns the first in `db.functions()` order. That order is presumably deterministic, but the *choice* is arbitrary and unverifiable against the user's intent.

**Fix:** Either honor the path/package prefix when matching (resolve `pkg/path` against the function's file/package), or downgrade the status to `Partial`/precision to `Heuristic` when the match is name-only and ambiguous (more than one candidate). Add a test with two same-named functions in different packages proving the configured root resolves to the intended one (or is flagged ambiguous), not the first by insertion order.

### WR-03: Go `init`/`main` exported-root dedup is asymmetric — a Go exported `init` in a non-`main` package still emits both an Init root and is excluded from Exported, but a Go `main` in a non-`main` package emits an Exported root

**File:** `crates/polint/src/analysis/reachability/discover.rs:59-118`

**Issue:** `go_main_init_roots` emits an `Init` root for **any** `init` function (line 76, no package guard) and a `Main` root only for `main` in package `main`. `exported_root` excludes `init` (any package) and `main`-in-`main` from the Exported kind (lines 100-106). The asymmetry: a Go function named `main` in a *non-main* package that is exported (capitalized — though `main` is lowercase so realistically not exported) would fall through to Exported; more importantly, an exported `init` is suppressed from Exported but `init` is never exported in Go (lowercase), so the guard at 100-106 for `init` is dead for real Go. The functional risk is small, but the dedup logic encodes assumptions (`is_exported` for `init`/`main`) that cannot hold for real Go identifiers, making the guard misleading and the dedup untested for the case it claims to handle. If `FunctionFact.is_exported` is ever set heuristically (e.g. for a non-Go file misclassified as Go), the guard behavior is unspecified.

**Fix:** Tighten or document the guard: since Go `main`/`init` are lowercase and never exported, the `is_exported` dedup branch for them is unreachable for valid Go — either remove the dead `init` arm from `exported_root` or add an assertion/comment that `is_exported` is always false for these. Add a test for an exported-capitalized function that merely *contains* `init`/`main` as a substring to confirm no false dedup.

### WR-04: `mark_call_reachability` is `pub(crate)` and run on every kernel invocation but lives behind no `#[cfg(test)]`, while its only consumers (`reachable_graph_lookup`, `scored_call_graph_edges_for_db`) are `#[cfg(test)]` — marks are computed and stored in production with no production reader

**File:** `crates/polint/src/analysis/reachability/traverse.rs:109-133`; `crates/polint/src/eval/metrics.rs:423-431`; `crates/polint/src/eval/runner.rs:149-178`

**Issue:** The provider (`provider.rs:55`) computes `marks` unconditionally in production and stores them. But every reader of the marks — `reachable_graph_lookup` and `filter_scored_edges_by_scoring_mode` and `scored_call_graph_edges_for_db` — is `#[cfg(test)]`. In a non-test build, the marking traversal runs over the full call graph (BFS over all `call_targets`), produces facts, validates and stores them, and nothing consumes them. This is dead computation on the production hot path (and the marks participate in the digest, so they also affect cache keys). Not a correctness bug, but it is wasted work plus a maintainability trap: a future reader added in production will silently depend on a fact family that today is only exercised by tests.

**Fix:** If marks are only needed by the (test-gated) eval scoring path in Phase 43, gate the marking computation/storage behind the same condition or document explicitly that production stores them for forward-compat (Phase 47/48). If kept, add a non-test assertion or at least a comment at `provider.rs:55` clarifying the marks are intentionally produced-but-unread in v1.3 production.

### WR-05: `jelly_span_file` uses `rsplitn(5, ':').last()` which silently returns the wrong file segment for Windows-style or colon-containing paths

**File:** `crates/polint/src/eval/metrics.rs:530-532`

**Issue:** `jelly_span_file` extracts the file portion of `file:start_line:start_col:end_line:end_col` via `span.rsplitn(5, ':').last()`. `rsplitn(5, ':')` splits from the right into at most 5 pieces; `.last()` returns the leftmost remaining chunk. If the file path itself contains a `:` (e.g. a Windows drive `C:\...` or a synthetic span with extra colons), the leftmost chunk is truncated/misattributed, so the `unmatched.file` field reported for debugging is wrong. This only affects the diagnostic `file` label in `JellyUnmatchedSpan`, not the matched/total ratio (which keys on the full span string), so it is a quality/observability defect rather than a scoring bug. Still, a wrong file in an "unmatched span" diagnostic actively misleads debugging.

**Fix:** Document that Jelly spans are normalized to forward-slash repo-relative paths with no colons (the renderer enforces this per `identity_render_invariants`), and add a debug assertion, or split on the first 4 colons from the *left* of the line/col tail only after confirming the path has no colon. Given the renderer already guarantees no `:\` (observed.rs:683-688), a comment tying this function to that invariant suffices.

### WR-06: Provider error path swallows the underlying message and still returns a success digest, so a failed store looks cached-and-valid

**File:** `crates/polint/src/analysis/reachability/provider.rs:92-104, 182-190`

**Issue:** When `db.replace_reachability_facts(storable)` fails (e.g. a dangling reference that escaped discovery), the provider returns `output_digest: Some(output_digest)` — the same digest as the success path — alongside a generic diagnostic. `provider_error_diagnostic` binds the real message to `_message` and discards it (line 183), emitting only "run internal debug output for details." Two problems: (1) the real failure reason is dropped, making the failure undebuggable, contradicting the verbose error discipline used in `validate.rs`; (2) returning a populated `output_digest` on a store failure means a caching layer keyed on `output_digest` could record a hit for a state where the facts were never actually stored — the db retains whatever it had before. Combined with CR-01, the digest is doubly untrustworthy on the error path.

**Fix:** Propagate the error message into the diagnostic evidence (mirror `validate.rs::push_diagnostic`'s `.with_evidence(...)`). On store failure, return `output_digest: None` (or a distinct sentinel) so the cache cannot treat a failed run as a valid cached output.

## Info

### IN-01: `discover_reachability_roots` assigns placeholder dense IDs that are immediately discarded by the provider — dead assignment

**File:** `crates/polint/src/analysis/reachability/discover.rs:48-53`

**Issue:** The loop `for (index, root) in roots.iter_mut().enumerate() { root.id = ReachabilityRootId(index as u64); }` assigns IDs that the provider overwrites after `normalized()` (`provider.rs:63-65`), and the comment even says "Discovery output IDs are never persisted." This is a harmless but pointless mutation that suggests IDs matter at discovery time when they do not.

**Fix:** Drop the placeholder loop; leave `id: ReachabilityRootId(0)` from the constructors (as the bridge/native/configured builders already set) and let the provider assign the only IDs that matter. Removes a misleading "IDs are meaningful here" signal.

### IN-02: `function_identity_for_target` does a linear scan of `db.functions()` per entrypoint bridge root — O(entrypoints × functions)

**File:** `crates/polint/src/analysis/reachability/discover.rs:266-274`

**Issue:** For each entrypoint, `entrypoint_bridge_root` calls `function_identity_for_target`, which scans all functions to find the target. Performance is out of v1 scope, but this is also a readability/consistency note: the rest of discovery already iterates `db.functions()` once. (No action required for correctness.)

**Fix:** If revisited later, build a `BTreeMap<FunctionId, &FunctionFact>` once and reuse it across the entrypoint and configured-root passes.

### IN-03: `mark_sort_key` and `root_sort_key` clone the full `stable_key` String on every comparison key extraction

**File:** `crates/polint/src/analysis/reachability/store.rs:27-42`

**Issue:** `sort_by_key` calls the key function on each element; `root_sort_key`/`mark_sort_key` clone the `stable_key` (and for roots the whole tuple). Correct and deterministic, just allocation-heavy. Out of v1 perf scope; noting for completeness.

**Fix:** Use `sort_by` with borrowed comparisons (`|a, b| (...).cmp(&(...))`) to avoid the clones if this is ever hot.

### IN-04: `unresolved_span()` and the unresolved configured root both hardcode `FileId(0)`, which is a valid file id — span/file of an unresolved root aliases a real file

**File:** `crates/polint/src/analysis/reachability/discover.rs:198-213, 325-327`

**Issue:** Unresolvable configured roots use `file: FileId(0)` and `Span::point(FileId(0), 0, 0)`. `FileId(0)` is a legitimate file id (the first added file). The unresolved root's `file` therefore points at a real file's id, and `validate_reachability`'s file-reference check (`validate.rs:91-99`) would pass for it even though semantically it has no file. Currently masked because these roots are filtered out before storage (WR-01), but if WR-01 is fixed by storing them, the file reference becomes a misleading alias. There is no sentinel `FileId` analogous to `UNRESOLVED_TARGET`.

**Fix:** If unresolved configured roots are ever stored, introduce a sentinel `FileId` (or make `file` `Option<FileId>`), and have the validator skip the file check for unresolved-status roots.

### IN-05: Two identical strings in `[reachability] roots` produce two roots with identical stable keys (duplicate)

**File:** `crates/polint/src/analysis/reachability/discover.rs:161-216`; `crates/polint/src/config/mod.rs:46-49`

**Issue:** `configured_roots_for` maps over the config `Vec<String>` 1:1 with no dedup. If a user lists the same entry twice (or two entries resolve to the same function with the same `(kind, language, identity, file, span)`), two roots with byte-identical `stable_key` are produced. For resolved duplicates, `check_duplicate_stable_keys` (`validate.rs:127-144`) would flag it as a diagnostic (good); for unresolved duplicates the stable key is `configured:{entry}` so identical entries collide identically. This is reported by validation rather than silently merged, so it is Info, but the config layer could reject duplicates earlier with a clearer message.

**Fix:** De-duplicate `reachability.roots` at config load (preserving first occurrence) or document that duplicate configured roots surface as duplicate-stable-key validation diagnostics.

---

_Reviewed: 2026-05-29_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
