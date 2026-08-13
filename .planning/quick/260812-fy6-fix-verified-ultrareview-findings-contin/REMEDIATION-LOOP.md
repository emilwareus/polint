# Ultrareview remediation loop

**Branch:** `static-analysis-architecture-review`  
**Started:** 2026-08-12  
**Rule:** append/update this document continuously after every fix, test, review, and decision.  
**Remote actions:** forbidden unless separately authorized; no push, PR, remote CI, main merge, or worktree operations.

## Baseline

- Starting HEAD: `40f7ec123c0796810b2ed0a497d68b84ca2f70c7`
- Source report: `../260812-f4r-perform-a-read-only-ultrareview-of-the-f/ULTRAREVIEW.md`
- Starting verdict: NOT READY — 1 blocker, 8 high, 8 medium.
- Current `main`: `fafd08d8af78d78313fdd61ff616a887652ac0ab`; read-only merge simulation reported 13 conflicts.
- Original 44 untracked artifacts must remain untouched; this GSD directory and the review directory are intentional additions.
- Stabilization checkpoint starts at `442a206e`; branch is one local commit ahead of origin and no push is authorized.
- Baseline package graph contains nine publishable packages (`polint`, `polint-macros`, and seven internal packages) plus unpublished `polint-bench` / `polint-eval`; supported prelude allowlist is 116 names; current debug binary is approximately 101.6 MB.
- Baseline golden cost gate is independently red at committed HEAD for `examples/go-sensitive-writes/json` (448 ms against a 429.6 ms allowance), confirming the local stabilization diff did not introduce that timing regression.

## Finding ledger

| ID | Finding | Status | Fix/evidence |
| --- | --- | --- | --- |
| B-1 | Reconcile current main and rerun final gates | open | — |
| H-1 | Cross-file/language call-site ID collisions | open | — |
| H-2 | Providers publish digest after failed store | open | — |
| H-3 | Neutral replacements omit metadata lifecycle | fixed locally | Facade `AnalysisHost` replacements now refresh stable metadata atomically; scheduled Go+TS deep-stack validation is clean. |
| H-4 | `new-rule` follows symlinked output parents | open | — |
| H-5 | Go embedded sidecar temp cache trusts marker | open | — |
| H-6 | Go offline mode misses cold sidecar build | open | — |
| H-7 | Go module/symbol subprocesses lack timeouts | open | — |
| H-8 | CI cargo-deny arguments unsupported | open | — |
| M-1 | Identity/reachability keys embed `FileId` | open | — |
| M-2 | Reachability digest contains dense IDs | open | — |
| M-3 | Type/value/alias digest contains dense IDs | open | — |
| M-4 | Refined-call digest contains dense IDs | open | — |
| M-5 | TS-specific semantic projection in facade | open | — |
| M-6 | Go RTA algorithm in facade | open | — |
| M-7 | `new-rule` failures leave partial scaffold | open | — |
| M-8 | CI Go cache paths point to old crate | open | — |

## Iteration 1 — correctness and integration

### Plan

- Reconcile/port current-main Phase 65 contracts without weakening the eight-crate split.
- Fix call-site identity, provider outcome atomicity, and metadata lifecycle.
- Run focused tests and independent review.

### Changes

- Started `git merge --no-commit --no-ff main`; retained post-split owners and resolved all 13 tracked conflicts without restoring deleted monolith providers.
- Ported API digest builder variants and API-owned `CacheStats` re-exports; retained private generation/run-manifest, store mirrors, sealed provider outcomes, telemetry, and runtime blocker dispatch in the facade.
- Adapted projection fixtures to public non-exhaustive constructors and `LanguageId` manifest fields.

### Validation

- `cargo fmt --all -- --check` — PASS.
- `cargo check -p polint --lib --locked` — PASS (reduced initial 7 library errors to 0).
- `cargo check --workspace --all-targets --locked` — PASS (reduced initial 52 errors to 0).
- `cargo check --workspace --all-targets --all-features --locked` — PASS.
- Focused tests: sealed outcomes 7 passed; dispatch projection 3 passed after canonical cache-independent identities; store lifecycle/mirror 49 passed; metrics projections 2 passed.
- All conflict markers are resolved and no tracked conflict paths remain.

### Review results and follow-up fixes

- H-1 implementation is limited to the split-crate owners: `crates/polint-analysis/src/mir_body_compose.rs`, `crates/polint-analysis/src/calls/store.rs`, and real provider-path regressions in `crates/polint/src/analysis/provider.rs`.
- Composition now collects call operations across all language outputs, orders them by stable body/operation/span inputs, assigns dense global `CallSiteId`s, and remaps operation calls, terminator calls, nested `MirValue::CallReturn`s, and call-return place roots/projections. Body-context lookup is primary; a unique language/file/site fallback handles values whose owner is represented only by a file. Ambiguous file-local duplicate coordinates remain unresolved rather than being attached to an arbitrary call.
- `CallStore::from_output` now rejects duplicate site IDs before constructing owner indexes; the regression uses distinct stable keys (`site-first` / `site-second`) with the same ID.
- Real `AnalysisKernel` provider-path tests cover two Go files, two TS files, and a polyglot pair whose legacy Go start-byte ID equals the legacy TS `(start << 32) | end` ID. The Go regression also reverses source discovery input and asserts stable path-to-ID mapping.
- Focused validation: `cargo fmt --all` — PASS; `cargo test -p polint-analysis mir_body_compose --locked` — PASS (1 test); `cargo test -p polint-analysis calls::store --locked` — PASS (7 tests); `cargo test -p polint analysis::provider::semantic_mir_provider::real_ --locked` — PASS (3 tests); `cargo test -p polint-analysis --locked` — PASS (790 passed, 1 ignored).
- Final focused validation after the deterministic-ordering test edit: `cargo fmt --all -- --check` — PASS; `cargo clippy -p polint-analysis -p polint --all-targets --locked -- -D warnings` — PASS; `cargo test -p polint-analysis mir_body_compose --locked` — PASS (1 test); `cargo test -p polint-analysis calls::store --locked` — PASS (7 tests); `cargo test -p polint analysis::provider::semantic_mir_provider::real_ --locked` — PASS (3 tests); nearest determinism gate `cargo test -p polint determinism --locked` — PASS (29 + 2 tests across matching targets).

## Iteration 2 — security, stable payloads, and ownership

_Not started._

## Iteration 3 — exhaustive review and final gates

_Not started._

## Final verdict

_In progress._


### Iteration 1 H-2 typed provider execution follow-up

- Added sealed `ProviderExecution` with setup/execution/validation failure reasons to every provider result contract and audited producers, including Go semantic setup/lifecycle/client/store paths.
- Kernel now gates upstream digests and Phase65 identity fallback on explicit typed success, records typed failures immediately, and blocks hard dependents in the same scheduling loop.
- Validation store rejection returns no digest and typed validation failure; failed outcomes carry no provider identity.
- Focused evidence: `cargo fmt --all`; `cargo check --workspace`; `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; `cargo test -p polint --lib analysis_kernel::outcome -- --nocapture` (8 passed); `cargo test -p polint-analysis --lib data_flow::store::tests::store_rejects -- --nocapture` (6 passed); `cargo test -p polint-analysis --lib -- --nocapture` (791 passed, 1 ignored).
- H-3 was completed in the stabilization tranche: `AnalysisDb` now owns metadata refresh after scheduled neutral replacements, including identity, reachability, solver, semantic graph, domains, and the existing moved families. Reachability metadata caps resolved roots at the provider's `setup_aware` ceiling, and semantic references use stable fact keys rather than dense IDs.
- A real `dataflow` capability plan over representative Go and TypeScript sources now schedules the deep provider closure and asserts both zero missing metadata and a clean full metadata-validation report.
- The semantic-store future-schema parity assertion now derives `found` from the installed future fixture rather than preserving the stale literal `2`.
- Stabilization evidence: formatting, workspace all-target/all-feature check and clippy, full capability matrix, semantic-store parity, `polint-analysis` all targets (790 passed, 1 ignored), `POLINT_VALIDATE_FACTS=1` Go/TS capability paths, public-surface leak, polyglot canary, and determinism gate all pass. The golden diagnostic outputs remain byte-stable, but the cost gate is currently 2–74 ms above the pre-existing `go-sensitive-writes/json` wall-clock budget across three local runs and remains open for performance triage.
