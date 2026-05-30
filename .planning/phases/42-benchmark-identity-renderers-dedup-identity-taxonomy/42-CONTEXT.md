# Phase 42: Benchmark Identity, Renderers, Dedup & Identity Taxonomy - Context

**Gathered:** 2026-05-28
**Status:** Ready for planning
**Mode:** `/gsd:discuss-phase 42 --auto`

<domain>
## Phase Boundary

Phase 42 delivers the **identity substrate** that the rest of the v1.3 Graph Engine Precision milestone reads from and writes to. Every function and callsite polint analyzes gets a stable identity record `(file, span, language, package/module, container, display, signature digest)`, deduplicated by semantic identity before scoring. Two per-benchmark renderers project that identity into Go `RelString`-style function/method names and Jelly `file:start_line:start_col:end_line:end_col` spans. Evaluation output gains a closed identity-vs-unsupported taxonomy (`wrong_identity`, `unsupported_edge`, `unresolved_edge`, `package_load_limitation`, `model_missing`). A public-surface-leak CI gate ensures no v1.3 solver types ever reach `polint::sdk::prelude::*`.

This phase does **not** add reachability roots (Phase 43), does **not** introduce the shared semantic graph or constraint vocabulary (Phase 44), does **not** add JS/TS inventory/scope/module-graph (Phase 45), does **not** ship the Go semantic frontend sidecar (Phase 46), and does **not** promote any new public SDK view (locked for v1.3). It builds strictly on the v1.2 substrate — `FactMeta`, `analysis::ids`, `analysis::calls` — and produces facts that all later v1.3 phases consume without modification.

</domain>

<decisions>
## Implementation Decisions

### Identity Record Placement and Shape

- **D-01:** Add a new private module `analysis::identity` (`crates/polint/src/analysis/identity/`) that owns the identity record fact family. Do not extend `analysis::ids` — that module stays focused on raw integer ID newtypes (`CallSiteId`, `CallTargetId`, …).
- **D-02:** The identity record is a single fact type `IdentityRecord` with fields: `kind: IdentityKind { Function, Callsite }`, `file_id: FileId`, `span: Span`, `language: LanguageTag`, `package_or_module: Arc<str>`, `container_path: Arc<str>` (dotted/scoped container — `pkg.Type.Method`, `module#Class.method`, `nsA.nsB.fn`), `display_name: Arc<str>`, `signature_digest: SignatureDigest([u8; 16])`.
- **D-03:** `SignatureDigest` is SHA-256 truncated to 16 bytes (32 hex chars when rendered). Truncation keeps fact size small while preserving collision resistance at repo scale. Digest input is a deterministic byte string of `[language tag, package/module, container path, display name, parameter shape if known, return shape if known]` — fields are length-prefixed to prevent boundary ambiguity.
- **D-04:** Identity records reference v1.2 IDs by composition, not replacement. Each `IdentityRecord` carries the originating `CallSiteId` or `CallTargetId` so dedup keys map back into existing `analysis::calls` facts without rewriting them. The existing `analysis::calls` provider stays intact.

### Renderer Organization

- **D-05:** Both renderers live in `analysis::identity::render::{go_relstring, jelly_span}` as `pub(crate)` functions. They are the single source of truth — eval adapters, validation, and any future debug tooling all call these renderers. Do not duplicate rendering logic in `crates/polint/src/eval/external/`.
- **D-06:** Renderer surface is small: `go_relstring::render(identity: &IdentityRecord) -> String` and `jelly_span::render(identity: &IdentityRecord, source: &SourceFile) -> String`. Renderers are pure functions of the identity record plus (for Jelly) the source file content; they do not touch the kernel.
- **D-07:** Go `RelString` follows `golang.org/x/tools/go/callgraph` conventions: `module/path.PkgFunc`, `(*module/path.Receiver).Method`, generic instantiations as `Func[T]` with normalized type-parameter names. Anonymous functions render as `package.parent$N` where N is the deterministic 1-based ordinal within the parent.
- **D-08:** Jelly `file:start_line:start_col:end_line:end_col` uses **1-based line, 1-based column, half-open end** — matching Jelly's micro-suite oracle exactly. The file portion is the workspace-relative path, forward-slash normalized.

### Dedup Strategy

- **D-09:** Dedup happens **once**, inside `analysis::identity::provider`, before output emission. The dedup key is the full `IdentityRecord` minus `file_id`/`span` for cross-file aliases, with span included for in-file uniqueness. Downstream consumers (eval, calls, future solvers) read deduplicated facts directly — no per-consumer dedup pass.
- **D-10:** Dedup is **semantic, not syntactic**: two callsites that resolve to the same `(language, package_or_module, container_path, signature_digest, span)` collapse to one identity record with a `multiplicity: u32` field recording the merge count. Multiplicity informs scoring without hiding underlying call-site facts.
- **D-11:** Snapshot fixtures verify dedup is stable across run order, file order, and provider order (consistent with the determinism gate that Phase 43 will inherit).

### CRLF/LF Normalization

- **D-12:** Normalization happens at **renderer time**, not at file load. On-disk spans stay byte-true to the source so v1.2 facts remain unchanged; the renderer normalizes line endings before computing Jelly's line/column positions so a `\r\n`-checked-out file and an `\n`-checked-out file produce byte-identical Jelly span strings.
- **D-13:** The CRLF/LF normalization fixture is a single source file recorded twice — once with `\n` endings, once with `\r\n` — and the renderer output for both must match byte-for-byte. The fixture lives under `tests/eval-fixtures/identity/crlf_normalization/`.

### Identity Category Taxonomy

- **D-14:** Categories are a closed enum `IdentityCategory { WrongIdentity, UnsupportedEdge, UnresolvedEdge, PackageLoadLimitation, ModelMissing }` in `analysis::identity::facts`. Closed enum, no `Other` variant — every classification must be explicit. Adding a category in a later phase is a deliberate API change.
- **D-15:** Eval reporting projects `IdentityCategory` into the report JSON as a lowercase snake_case string discriminator. The `KernelRunReport` and downstream eval JSON gain a `categorized_failures: { wrong_identity: u32, unsupported_edge: u32, unresolved_edge: u32, package_load_limitation: u32, model_missing: u32 }` summary counter alongside the existing TP/FP/FN/unknowns rows.
- **D-16:** Categorization is performed by `analysis::identity::categorize` against the existing `unresolved_edge`/`unknown` fact families plus the new identity records. Wrong-identity is detected when an emitted callsite identity does not match any oracle identity but the file/span overlaps an oracle entry — i.e. polint named the right place wrong. Package-load-limitation and model-missing categories are surfaced by attaching a `categorize::Reason` to existing unresolved/budget facts; no new fact families are required beyond the category enum and the per-fact reason tag.

### Public-Surface-Leak CI Gate

- **D-17:** The leak gate is a workspace integration test at `tests/public_surface_leak.rs` that depends on a tiny external rule crate fixture (`tests/fixtures/public-surface-leak-probe/`). The probe crate imports only `polint::sdk::prelude::*` and a curated allow-list of v1.0–v1.2 public types. The test compiles the probe via `trybuild` and snapshot-asserts that **zero** identifiers from the `analysis::` private namespace reach the probe crate's symbol table.
- **D-18:** The leak gate runs in fast CI (every PR), on Linux + macOS, alongside existing public-no-leak proofs. Failure is a hard block — Phase 42 is the phase that institutes this gate, and every subsequent v1.3 phase inherits it.
- **D-19:** The leak gate's allow-list is captured once in `tests/public_surface_leak.rs` as a source-of-truth list. Phases 43–54 may add public types only by extending this list deliberately. v1.3's "no public SDK promotion" rule means the list should not grow during v1.3 except for documented exceptions reviewed at milestone close.

### Jelly Oracle Coverage Measurement

- **D-20:** Oracle-span coverage is a deterministic, fixture-based count: `oracle_spans_matched / oracle_spans_total >= 0.99` over the Jelly micro-fixture set already wired through `crates/polint/src/eval/external/jelly_callgraph.rs`. No probabilistic sampling.
- **D-21:** The coverage test reports per-fixture matches and surfaces unmatched oracle spans individually so regressions are debuggable. Output lives in the eval report JSON under `jelly_oracle_coverage: { matched: u32, total: u32, ratio: f64, unmatched: Vec<{file, span, reason}> }`.
- **D-22:** Coverage runs in fast CI on Linux + macOS. Both platforms must pass the ≥99% threshold independently — no cross-platform averaging.

### Visibility and Provider Wiring

- **D-23:** Every new module, fact, type, enum, and function is `pub(crate)`. Identity records flow through the kernel's existing provider/cache/manifest/digest machinery. The provider manifest entry is `polint.identity`, ordered after `polint.calls` (because identity references existing call IDs) and before any v1.3 solver-introducing provider.
- **D-24:** Identity provider participates in the cache key — its digest inputs are source file digests, language tag, the v1.2 calls provider digest, and renderer code version (so renderer changes invalidate identity cache deterministically).
- **D-25:** Cross-platform byte-identical renderer output is a hard contract: identity record bytes, renderer output bytes, and provider digest must be identical between Linux and macOS for the same input snapshot. The CRLF fixture is one half of the proof; a workspace-path-normalization fixture is the other.

### The Agent's Discretion

- The planner may choose the exact internal layout of `analysis::identity/`: `facts.rs`, `provider.rs`, `render/{go_relstring.rs,jelly_span.rs}`, `categorize.rs`, `dedup.rs`, `cache_key.rs`, `validate.rs`, `store.rs` — provided visibility stays `pub(crate)` and the digest discipline matches `analysis::calls`.
- The planner may decide whether `multiplicity` is stored on the dedup output record or computed by a downstream view, provided the snapshot fixtures remain byte-stable.
- The planner may pick the exact `LanguageTag` representation (newtype over `&'static str` vs enum) provided it round-trips deterministically through serde and is forward-compatible with adding more languages in later milestones.
- The planner may decide whether the public-surface-leak probe crate vendors a minimal `Cargo.toml` per probe scenario or uses a single `[features]`-gated crate, provided the snapshot stays stable on both supported platforms.
- The planner may split Phase 42 into the natural slices: identity records + provider + dedup; renderers + CRLF fixture; identity taxonomy + eval projection; public-surface-leak gate + probe crate.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 42 goal, IDENT-01/02/03 mapping, success criteria, v1.3 milestone framing, no-public-SDK-promotion rule.
- `.planning/REQUIREMENTS.md` — IDENT-01/02/03 requirement text and dependency on v1.2 substrate.
- `.planning/PROJECT.md` — Product boundary, private-analysis-first milestone intent, agent-extensible thesis, and public API discipline carried into v1.3.
- `.planning/STATE.md` — Current v1.3 planning state and v1.2 closeout reference.

### v1.3 Graph Engine Benchmark Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — Two-suite scope (Go x/tools RTA + Jelly micro), baseline numbers, what each oracle expects, and engine-capability gaps that motivate identity + renderers.
- `research/evaluation-harness/FINAL-REPORT.md` — External-benchmark-first strategy, suite ranking, measurement model, tiers.
- `research/evaluation-harness/RECOMMENDED_IMPLEMENTATION.md` — Internal eval architecture, canonical model, matchers/metrics, native fixture adapter, OWASP adapter, default-vs-extension delta.
- `research/evaluation-harness/STANDARD.md` — Suite/case/adapter vocabulary, expected/observed model, result classes, determinism requirements.
- `research/evaluation-harness/decisions/decision-log.md` — Accumulated benchmark architecture decisions inherited from v1.2.
- `research/call-graphs/FINAL-REPORT.md` — Layered call-graph conclusion, unresolved facts, repo-local model provenance — context for the identity-vs-unsupported taxonomy.
- `research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md` — Revised private semantic-bootstrap path, `analysis::calls` placement, public-view deferral.

### Upstream v1.2 Phase Decisions Carried Forward

- `.planning/milestones/v1.2-phases/40-external-benchmark-adapters-and-promotion-gates/40-CONTEXT.md` — Crate-private eval extension, suite manifest shape, deterministic JSON reports, no-vendoring policy, tiered gates.
- `.planning/milestones/v1.2-phases/41-public-sdk-query-views-and-agent-ergonomics/41-CONTEXT.md` — Final v1.2 public-surface decisions; v1.3 must not regress these.
- `.planning/milestones/v1.2-phases/37-refined-call-graph-providers/37-CONTEXT.md` — Refined call edge fact contract preserved into v1.3, provider digest discipline.
- `.planning/milestones/v1.2-phases/30-direct-call-facts/30-CONTEXT.md` — Direct call-site/target/unresolved fact model identity records layer on top of.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/ids.rs` — Raw integer ID newtypes (`CallSiteId`, `CallTargetId`, `FileId`-adjacent IDs); reference for visibility/serde discipline. Identity adds new IDs alongside, does not extend this file.
- `crates/polint/src/analysis/calls/` — Direct call fact family (`facts.rs`, `provider.rs`, `extract.rs`, `unresolved.rs`, `validate.rs`, `cache_key.rs`, `store.rs`) — identity references but does not modify these.
- `crates/polint/src/eval/external/jelly_callgraph.rs` — Existing Jelly adapter with NDJSON parsing, span model, fixture enumeration; identity renderer plugs into this adapter's `normalize_observed` path.
- `crates/polint/src/eval/external/mod.rs` — External adapter registry; Go RTA adapter slot lives or will live here.
- `crates/polint/src/eval/{adapter.rs,model.rs,observed.rs,report.rs,runner.rs,suite.rs,metrics.rs}` — Existing eval canonical model identity categories project into.
- `crates/polint/src/eval/fixtures.rs` — Native fixture manifests; new `tests/eval-fixtures/identity/` fixtures follow this shape.
- `crates/polint/src/sdk/{mod.rs,facts.rs,scope.rs}` — Current public surface (`polint::sdk::prelude::*`); the leak gate's allow-list is anchored here.
- `crates/polint/src/analysis_kernel/{provider.rs,validation.rs,debug.rs}` — Provider manifest/order/schema vocabulary; identity provider slots into the manifest after `polint.calls`.
- `tests/eval-fixtures/` — Native fixture suite and provider-order expectations; identity fixtures extend this tree.
- `tests/fixtures/` — Shared fixture root; the public-surface-leak probe crate lives under `tests/fixtures/public-surface-leak-probe/`.
- `AGENTS.md` and `docs/API-VISIBILITY-PLAN.md` — Public API visibility discipline; the leak gate enforces the rules described here.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::ids::{CallSiteId, CallTargetId, …}` already encode every raw ID identity records reference; identity records compose these rather than duplicate them.
- `analysis::calls::facts` already carries `CallSiteFact`, `CallTargetFact`, `UnresolvedCallFact`, status/algorithm/precision/provenance vocabulary. Identity records reference these by ID; no rewrite needed.
- `analysis::calls::provider` already normalizes output and digests source/config/lifecycle/upstream provider inputs. The identity provider copies this digest discipline rather than inventing a new cache model.
- `eval::external::jelly_callgraph::JellyCallgraphAdapter` already enumerates Jelly micro-fixture cases and parses oracle NDJSON. The Jelly renderer plugs into this adapter's existing observed-output path.
- `eval::{model, observed, report}` already define `ObservedItem`, `ExpectedItem`, `CaseResult`, and `KernelRunReport`; the identity-category counters extend these without breaking JSON consumers.
- `analysis_kernel::provider` provides the provider manifest plumbing identity slots into.

### Established Patterns

- **Provider digest participation:** every v1.2 provider digests source, config, lifecycle, and upstream provider digests; identity provider follows the same recipe so cache invalidation behaves identically.
- **Visibility discipline:** every analysis fact family is `pub(crate)`; the v1.3 milestone explicitly bans new public SDK promotion. Identity follows this absolutely.
- **Determinism:** v1.2 providers sort by stable keys, assign dense IDs only after sorting, and emit deterministic JSON. Identity records inherit this — sort by `(language, package_or_module, container_path, file_id, span, kind)`.
- **Snapshot-fixture verification:** v1.2 phases verify fact shapes through snapshot fixtures under `tests/eval-fixtures/`. Identity adds `tests/eval-fixtures/identity/` rather than inventing a new fixture root.
- **Cross-platform byte-identical proof:** existing CI gates already verify Linux + macOS byte-identical reports. Identity renderers extend this contract — same fixtures, same byte-identical assertion.

### Integration Points

- `analysis_kernel::provider` manifest gains `polint.identity` between `polint.calls` and any v1.3 solver-introducing provider.
- `eval::report::KernelRunReport` gains a `categorized_failures` counter map and a `jelly_oracle_coverage` ratio + unmatched list.
- `eval::external::jelly_callgraph` and the (existing or new) Go RTA adapter both consume `analysis::identity::render::{go_relstring, jelly_span}` rather than rendering identities themselves.
- `tests/public_surface_leak.rs` is a new top-level workspace integration test; its allow-list is the v1.3 public-surface source of truth.
- `polint::sdk::prelude` re-exports stay frozen for v1.3 — the leak gate enforces this.

</code_context>

<specifics>
## Specific Ideas

- Go `RelString` format must match `golang.org/x/tools/go/callgraph/cha` and `…/rta` output verbatim for benchmark-matchable cases: package path slash-joined, function `pkg.Func`, method `(*pkg.T).M` or `(pkg.T).M` for pointer-vs-value receivers, generic instantiation `Func[T0,T1,...]` with normalized type parameter names.
- Jelly span format is `file:start_line:start_col:end_line:end_col` with **1-based line, 1-based column, half-open end column**. The micro suite under Jelly's repository is the oracle — match its conventions exactly.
- Anonymous functions in both renderers use the parent-relative ordinal pattern (`parent$1`, `parent$2`, …) consistent with the established v1.2 fact display conventions and with Jelly's anonymous-function naming.
- The CRLF/LF fixture should exercise an actual multi-line function so line counts genuinely shift between `\n` and `\r\n` encodings — a single-line probe wouldn't catch normalization regressions.
- The public-surface-leak probe crate should import `polint::sdk::prelude::*` with a glob to maximize the chance of catching accidental re-exports; the snapshot then enumerates which prelude items are actually reachable.

</specifics>

<deferred>
## Deferred Ideas

- **Per-suite scoring mode** — `oracle-rta`, `oracle-jelly`, `whole-repo` modes belong in Phase 43 (REACH-02). Identity records expose enough to make scoring-mode selection well-defined; the mode field itself is Phase 43.
- **Determinism gate (10-shuffle byte-identical observed JSON)** — Phase 43 (REACH-03). Phase 42 inherits determinism *implicitly* through provider digest discipline; the explicit gate fixture is Phase 43.
- **Shared `analysis::semantic_graph` skeleton + `EdgeKind`/`NodeKind`/constraint enum** — Phase 44 (GRAPH-01/02). Identity records will become `NodeKind::Function` and `NodeKind::Callsite` inputs in Phase 44 without modification.
- **JS/TS inventory + scope + module graph + direct calls as constraints** — Phase 45 (JS-01/02/03). Identity covers naming; Phase 45 covers structural enumeration through Oxc.
- **Go semantic frontend + sidecar (`polint-go-frontend` binary + NDJSON protocol)** — Phase 46 (GO-01..04). Identity covers Go `RelString` naming; Phase 46 covers the typed-process boundary feeding richer Go semantic facts.
- **Unified solver core + `DerivedEdgeProvenance`** — Phase 47.
- **Reachability roots from v1.2 entrypoints** — Phase 43 (REACH-01).
- **Public SDK promotion of any v1.3 type** — explicitly out of v1.3 per ROADMAP.md; revisit at milestone close.

</deferred>

---

*Phase: 42-Benchmark Identity, Renderers, Dedup & Identity Taxonomy*
*Context gathered: 2026-05-28*
