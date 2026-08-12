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

## Finding ledger

| ID | Finding | Status | Fix/evidence |
| --- | --- | --- | --- |
| B-1 | Reconcile current main and rerun final gates | open | — |
| H-1 | Cross-file/language call-site ID collisions | open | — |
| H-2 | Providers publish digest after failed store | open | — |
| H-3 | Neutral replacements omit metadata lifecycle | open | — |
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

_Not started._

## Iteration 2 — security, stable payloads, and ownership

_Not started._

## Iteration 3 — exhaustive review and final gates

_Not started._

## Final verdict

_In progress._
