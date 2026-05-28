# Phase 42: Benchmark Identity, Renderers, Dedup & Identity Taxonomy - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-28
**Phase:** 42-benchmark-identity-renderers-dedup-identity-taxonomy
**Mode:** `--auto` (fully autonomous; Claude picked recommended option for every gray area)
**Areas discussed:** Identity record placement, Identity record shape & signature digest, Renderer organization, Dedup strategy, CRLF/LF normalization, Identity category taxonomy, Public-surface-leak CI gate, Jelly oracle coverage measurement, Visibility & provider wiring

---

## Identity Record Placement

| Option | Description | Selected |
|--------|-------------|----------|
| Extend `analysis::ids` | Add identity record alongside raw integer ID newtypes | |
| New `analysis::identity` module | Dedicated module for the fact family, leaving `ids` for raw IDs | ✓ |
| Inline into `analysis::calls` | Add identity fields directly onto existing call facts | |

**Selection:** New `analysis::identity` module — keeps `analysis::ids` focused on raw IDs and gives identity its own provider, dedup, render, and validate slots without bloating `calls`.
**Auto rationale:** Recommended default; matches v1.2 module-per-concern pattern (`calls`, `entrypoints`, `types`, `values`, `access_paths`, …).

---

## Identity Record Shape & Signature Digest

| Option | Description | Selected |
|--------|-------------|----------|
| Full record + full SHA-256 digest | All fields + 32-byte digest | |
| Full record + truncated 16-byte digest | All fields + 16-byte truncated SHA-256 | ✓ |
| Minimal record (file/span/name) + no digest | Lean record, rely on file/span for uniqueness | |

**Selection:** Full record with kind/file/span/language/package/container/display/16-byte signature digest. Truncated digest balances size with collision resistance at repo scale; length-prefixed digest input prevents boundary ambiguity.
**Auto rationale:** Recommended default; the roadmap success criterion explicitly names every field in the tuple, so the gray area is digest size, not field set.

---

## Renderer Organization

| Option | Description | Selected |
|--------|-------------|----------|
| `analysis::identity::render::{go_relstring, jelly_span}` | Renderers live with identity; eval adapters consume them | ✓ |
| `eval::external::{go_render, jelly_render}` | Renderers live next to eval adapters | |
| Trait on `IdentityRecord` with per-language impls | Polymorphic render method | |

**Selection:** Renderers in `analysis::identity::render` as `pub(crate)` free functions. Single source of truth — eval, validation, and any future debug tooling all call them.
**Auto rationale:** Recommended default; avoids duplication between identity tests and eval adapters and keeps the eval external layer thin.

---

## Dedup Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Per-consumer dedup pass | Each downstream consumer dedupes when reading | |
| Dedup once in `analysis::identity::provider` | Provider emits already-deduplicated records with multiplicity | ✓ |
| Lazy dedup view on top of raw facts | Index built on first query | |

**Selection:** Provider-side dedup with `multiplicity: u32` on the merged record. Downstream consumers read deduplicated facts directly. Dedup key uses semantic identity (language, package/module, container, signature digest, span) — not pure source-text equality.
**Auto rationale:** Recommended default; minimizes work for downstream phases and matches the "deduplicated by semantic identity before scoring" requirement text in IDENT-01.

---

## CRLF/LF Normalization

| Option | Description | Selected |
|--------|-------------|----------|
| Normalize at file-load time | Replace CRLF with LF as files enter the kernel | |
| Normalize at renderer time | Renderers normalize line endings when computing Jelly positions | ✓ |
| Per-fixture conversion in CI | Pre-process fixtures with `dos2unix` before testing | |

**Selection:** Renderer-time normalization. On-disk spans stay byte-true; only the Jelly span string normalizes line endings before computing line/column. Verified by a CRLF↔LF fixture pair that must produce byte-identical renderer output.
**Auto rationale:** Recommended default; load-time normalization would invalidate v1.2 spans, and per-fixture CI conversion would mask real on-disk-CRLF user repos.

---

## Identity Category Taxonomy

| Option | Description | Selected |
|--------|-------------|----------|
| Closed enum (5 named variants) | `IdentityCategory { WrongIdentity, UnsupportedEdge, UnresolvedEdge, PackageLoadLimitation, ModelMissing }` | ✓ |
| Open enum with `Other(String)` | Allow ad-hoc categories | |
| String discriminator | Free-form `category: String` field | |

**Selection:** Closed enum. Adding a category is a deliberate API change. Eval projects the enum into report JSON as lowercase snake_case.
**Auto rationale:** Recommended default; the success criterion enumerates exactly these five and a closed enum keeps Phase 43+ honest about adding new failure modes.

---

## Public-Surface-Leak CI Gate

| Option | Description | Selected |
|--------|-------------|----------|
| Static doc audit | Manual review of `polint::sdk::prelude` exports | |
| Workspace integration test compiling an external probe crate | `trybuild`-style test against `polint::sdk::prelude::*` with snapshot allow-list | ✓ |
| `cargo public-api` snapshot in CI | Use `cargo-public-api` to diff the public surface | |

**Selection:** Workspace integration test at `tests/public_surface_leak.rs` with a probe crate under `tests/fixtures/public-surface-leak-probe/`. Runs in fast CI on Linux + macOS. Allow-list captured in the test source.
**Auto rationale:** Recommended default; lives in the codebase, runs every PR, and the snapshot doubles as documentation of the supported v1.0–v1.2 public surface that v1.3 must not regress.

---

## Jelly Oracle Coverage Measurement

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministic fixture-based count | Match against fixed Jelly micro-fixture set | ✓ |
| Probabilistic sample | Random subset per CI run | |
| Manual spot-check | Maintainer review without automated count | |

**Selection:** Deterministic count over the Jelly micro-fixture set already wired through `crates/polint/src/eval/external/jelly_callgraph.rs`. Threshold `oracle_spans_matched / oracle_spans_total >= 0.99` on Linux + macOS independently. Per-fixture matches and unmatched spans surfaced individually.
**Auto rationale:** Recommended default; only deterministic measurement is acceptable for a CI gate. Probabilistic sampling would cause flakes.

---

## Visibility & Provider Wiring

| Option | Description | Selected |
|--------|-------------|----------|
| `pub(crate)` everywhere; new manifest entry `polint.identity` | Slot identity after `polint.calls` in the manifest | ✓ |
| `pub` for renderer, `pub(crate)` for facts | Promote rendering helpers to public | |
| Fold identity into the existing `polint.calls` provider | No new manifest entry | |

**Selection:** Full `pub(crate)` visibility; new manifest entry `polint.identity` ordered after `polint.calls`. Provider digests inputs the same way v1.2 providers do.
**Auto rationale:** Recommended default; v1.3 milestone forbids new public SDK promotion, and a dedicated provider gives clean cache invalidation boundaries.

---

## Claude's Discretion

The following sub-decisions were deferred to the planner under `--auto` mode:

- Exact internal layout under `analysis::identity/` (`facts.rs`, `provider.rs`, `render/{go_relstring.rs,jelly_span.rs}`, `categorize.rs`, `dedup.rs`, `cache_key.rs`, `validate.rs`, `store.rs`).
- Whether `multiplicity` is stored on the dedup output record or computed by a downstream view.
- Exact `LanguageTag` representation (newtype over `&'static str` vs enum).
- Whether the public-surface-leak probe crate vendors one `Cargo.toml` per scenario or uses one feature-gated crate.
- Phase 42 slice boundaries for the planner (identity facts + provider + dedup; renderers + CRLF fixture; identity taxonomy + eval projection; leak gate + probe).

## Deferred Ideas

Items mentioned during analysis but belonging to later v1.3 phases:

- Per-suite scoring mode (`oracle-rta` / `oracle-jelly` / `whole-repo`) → Phase 43 (REACH-02).
- Determinism gate fixture (10-shuffle byte-identical observed JSON) → Phase 43 (REACH-03).
- Shared `analysis::semantic_graph` skeleton + `NodeKind`/`EdgeKind`/constraint enum → Phase 44 (GRAPH-01/02).
- JS/TS inventory + scope + module graph + direct-call constraints → Phase 45 (JS-01/02/03).
- Go semantic frontend + sidecar (`polint-go-frontend` binary + NDJSON protocol) → Phase 46 (GO-01..04).
- Unified solver core + `DerivedEdgeProvenance` → Phase 47.
- Reachability roots from v1.2 entrypoints → Phase 43 (REACH-01).
- Public SDK promotion of any v1.3 type → explicitly out of v1.3 per ROADMAP.md; revisit at milestone close.
