# Stack Research

**Domain:** Multi-language native static-analysis engine — shared semantic graph and unified call-graph solver (Rust core), Go semantic frontend (sidecar), JS/TS function-token + object/property solver, Go RTA provider, adaptation model layer
**Researched:** 2026-05-27
**Confidence:** HIGH for Rust crate selection and Oxc/tree-sitter continuity. MEDIUM for the Go sidecar protocol choice (architectural; rationale is HIGH but exact wire format is a deliberate decision). HIGH for "what NOT to add."

## Scope

v1.3 is a **subsequent milestone** layered on the validated v1.2 substrate. The existing stack (Rust 2024 workspace, rustc 1.94.0, cargo 1.94.0, clap 4.6.1, serde 1.0.228, serde_json 1.0.149, toml 1.1.2, anyhow 1.0.102, thiserror 2.0.18, rayon 1.12.0, ignore 0.4.25, globset 0.4.18, petgraph 0.8.3, tree-sitter 0.26.8, tree-sitter-go 0.25.0, Oxc 0.129.0, oxc_resolver 11.19.1, insta 1.47.2, assert_cmd 2.2.1, predicates 3.1.4, tempfile 3.27.0, proptest 1.11.0) **stays**. This research focuses **only on deltas** required to ship the v1.3 graph engine precision work.

Reference: `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md`, `research/call-graphs/RECOMMENDED_IMPLEMENTATION.md`, `research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md`, `research/type-alias-points-to/RECOMMENDED_IMPLEMENTATION.md`, the current `Cargo.toml` and `crates/polint/Cargo.toml`.

## Version Verification Pass (Existing Stack)

Verified on 2026-05-27 against crates.io. Already-pinned versions remain current; small bumps are non-breaking.

| Crate | Currently Pinned | Latest Stable | Action |
|---|---|---|---|
| `oxc_*` (parser/ast/semantic/span/allocator) | 0.129.0 | 0.133.0 (2026-05-26) | **Bump to 0.133.0** when starting Phase 1. Required: v1.3 leans hard on `oxc_semantic` for scope/binding/`SymbolTable`/`ScopeTree`. Each weekly Oxc release brings semantic fidelity and bug fixes; staying current minimizes drift. |
| `oxc_resolver` | 11.19.1 | 11.19.2 (2026-05-25) | Patch bump; safe. |
| `tree-sitter` | 0.26.8 | 0.26.9 (2026-05-19) | Patch bump; safe. |
| `tree-sitter-go` | 0.25.0 | 0.25.0 | No change. |
| `petgraph` | 0.8.3 | 0.8.3 | No change — used as the existing graph substrate; v1.3 extends with hand-rolled solver structures rather than replacing it. |
| Everything else | as recorded | within point releases | No required bumps for v1.3. |

`bincode` 3.0 is now stable (Dec 2025) — relevant only if we adopt it for the layer cache or sidecar wire format (decisions below).

## Recommended Stack Additions

### Core Additions (Rust workspace)

| Crate | Version | Purpose | Why Recommended for v1.3 |
|---|---|---|---|
| `fixedbitset` | 0.5.7 | Compact, fast bitsets for reachability, points-to membership, function-token sets, RTA-reached function sets, address-taken-function sets, scope/CFG worklists. | Already a transitive dep through `petgraph`; using it directly is the canonical Rust choice for the bit-set operations central to RTA, Andersen, and function-token propagation. Deterministic, no `unsafe` exposure to consumers, zero new toolchain risk. |
| `roaring` | 0.11.4 | Compressed bitmaps for **sparse** large token/object sets (JS/TS function-token propagation, allocation-site abstraction with many objects, summary projection sets) where density is low and union/intersection cost dominates. | When token sets are sparse, RoaringBitmap dramatically beats `fixedbitset` on memory and on union/intersection in the worklist. Used by major analyzers and search systems. Should be opt-in per fact family — both crates can coexist. |
| `hashbrown` | 0.17.1 | Direct access to SwissTable (with `raw_entry`/`entry_ref` and custom hashers) for hot solver maps: place→token-set, callsite→target-set, alias indexes. | `std::collections::HashMap` already uses hashbrown internally but does not expose the raw entry API the solver needs (`entry_ref` to avoid String clones on lookups, `raw_entry_mut` for incremental token-set updates). Explicit dep gives us the hot-path control we need. |
| `rustc-hash` | 2.1.2 | `FxHasher` for all internal solver hash maps/sets that key on `u32`/`u64` IDs (PlaceId, CallSiteId, TokenId, SymbolId). | Deterministic, fast on integer keys, no DOS-resistance needed for in-process internal state. Replaces ad-hoc `BuildHasherDefault<FxHasher>` patterns. Used throughout rustc for exactly this scenario. |
| `smallvec` | 1.15.1 | Inline-store small vectors for `points_to_set`, `call_targets`, `flow_edges`, `summary_inputs` where the common case is 0–4 elements. | Dramatically reduces heap pressure for the millions of small collections an Andersen/RTA solver produces. Already in industry use (rustc, swc, Oxc). Drop-in for `Vec` in fact bodies. |
| `indexmap` | 2.14.0 | Deterministic insertion-ordered maps for fact-store outputs that must be hashed/snapshotted. | Critical for our layer-cache invariants — `HashMap` iteration order changes per run, breaking deterministic digests and snapshot tests. We already need this in several spots; v1.3 makes it mandatory for solver outputs. |
| `string-interner` | 0.20.0 | Stable, deterministic interning of property names, package paths, qualified Go names (`pkg.Type.Method`), Jelly span keys, model match strings. | Property keys appear millions of times in the JS/TS object/property model and Go method-set lookups. Avoids `String` proliferation and gives us a small `Symbol` (u32) we can put inside `PlaceKind::Property`/`FieldKey`. `lasso` 0.7.3 is the alternative — picking `string-interner` for its simpler API and 2026 maintenance cadence (0.20.0 released April 2026 vs. lasso's Aug 2024). |
| `blake3` | 1.8.5 | Fast cryptographic-strength digests for new cache key families: solver budget digest, Go sidecar input digest (package patterns + lockfile + go.work + build tags + Go version + file digests), model digest, adaptation accept/reject manifest digest. | We already need stable cache keys for the new fact families per `research/incremental-query-engine/`. blake3 is ~10x faster than sha256 on real workloads, parallelizable, deterministic, and battle-tested. The existing v1.2 cache used ad-hoc hashing — v1.3's expanded cache must standardize. |

### Go Semantic Frontend (Sidecar) — Architectural Choice

**Recommendation: Out-of-process Go helper sidecar + serialized facts (JSON-line / NDJSON over stdio).**

This is a primary architectural decision. The three candidates and the rationale:

| Option | Verdict | Why |
|---|---|---|
| **In-process FFI via `cgo` from a Rust binding to `go/packages`+`go/ssa`** | **Rejected** | (1) `go/packages` and `go/ssa` are pure Go; calling them via cgo requires hosting the Go runtime inside our Rust process. (2) Pulls Go GC, threading, signal handlers into polint's process — breaks rayon determinism and Rust panic isolation. (3) No supported Rust binding exists. (4) Cross-compilation becomes painful. (5) Sidebar v1.2 explicitly preserved this as a future trait boundary; we are honoring that. |
| **Out-of-process Go sidecar + serialized facts (recommended)** | **Accepted** | (1) Process isolation matches v1.2's existing extension-provider quarantine model. (2) Go binary can be cross-compiled, distributed, pinned by version. (3) Subprocess can be cached at the polint layer-cache boundary (input digest in → facts out). (4) Determinism preserved — Go side runs single-threaded over a fixed package set; Rust side schedules with rayon as before. (5) Crash containment: a Go-side panic does not kill polint. (6) Same model gopls/`golangci-lint`/staticcheck-as-CLI use successfully. |
| **Embed a Rust reimplementation of `go/types`+SSA** | **Rejected** | Multi-engineer-year effort, would diverge from the Go toolchain's authoritative type checker. The product principle "use official language tooling where it is the compatibility authority" (from `research/type-alias-points-to/`) explicitly endorses Go toolchain reuse. |

**Wire format choice: NDJSON (newline-delimited JSON) over stdio.**

| Format | Verdict | Rationale |
|---|---|---|
| **NDJSON (recommended)** | **Accepted** | (1) `serde_json` 1.0.149 already in stack — zero new deps on the Rust side. (2) On the Go side, `encoding/json` is the standard library — zero new deps. (3) Human-debuggable: developers can `cat` a captured stream to debug RTA edge issues. (4) Snapshot-friendly: easy to insta-snapshot trimmed sidecar output for fixtures. (5) Streaming: Rust can begin consuming functions/SSA blocks before Go finishes the whole program. (6) Determinism: line-oriented total order is easy to enforce on the Go side. (7) Performance: x/tools-RTA fixtures are ≤100 packages — JSON parsing is not the bottleneck; package loading and type-checking are. |
| **Protobuf (`prost` 0.14.3)** | **Rejected for v1.3** | Adds two heavy deps (`prost` + `prost-build` with `protoc` toolchain dependency on contributors' machines), schema versioning ceremony, and gains negligible perf for our 100-package upper bound. Reconsider only if benchmark shows JSON parsing > 5% of total RTA runtime on real repos. |
| **MessagePack (`rmp-serde` 1.3.1)** | **Rejected** | More compact than JSON but not human-debuggable, no Go stdlib support, no clear win for our workload size. |
| **gob (Go-native)** | **Rejected** | Go-only — would require a Rust gob decoder we don't have. |
| **`bincode` 3.0** | **Rejected** | Rust-only format; cannot be produced by a Go process. |
| **Capnp / FlatBuffers** | **Rejected** | Zero-copy benefits are wasted on the data sizes we expect; adds schema toolchain. |

**Sidecar dependency footprint (Rust side):**

| Crate | Version | Purpose |
|---|---|---|
| (no new crate) | — | `std::process::Command` and `tokio` are not needed — we use a synchronous `Stdio::piped()` model. |
| `which` | 8.0.2 | Locate `polint-go-frontend` binary on PATH or by configured path; emit a structured `SetupMissing` fact if not present, matching the existing unsupported/unknown taxonomy. |

**Sidecar binary (Go side, not a Rust dep, but documented for tracking):**

| Module | Pinned version | Purpose |
|---|---|---|
| `golang.org/x/tools` | v0.45.0 (2026-05-08) | Provides `go/packages` (`Load`, `LoadMode`) and `go/ssa` (`ssautil.AllPackages`, `prog.Build()`). |
| `golang.org/x/tools/go/callgraph/rta` | (same module) | Reference RTA implementation; v1.3 may either consume `rta.Analyze` output directly or reimplement RTA in Rust over emitted SSA-like facts. **Decision to be resolved in Phase 6 (Go RTA provider) of the v1.3 roadmap**, not in this stack research. |

**Sidecar build distribution:** Ship `polint-go-frontend` as a separately built Go binary published alongside polint releases. Users supply Go toolchain themselves only if they want to rebuild from source. (This mirrors how `gopls` is distributed.)

### Supporting Libraries (Conditional)

| Crate | Version | Purpose | When to Use |
|---|---|---|---|
| `bincode` | 3.0 | Compact binary serialization for the **layer cache** payloads (places, points-to constraints, MIR, summaries). | Add **only** if Phase X profiling shows `serde_json` cache I/O > 10% of total runtime on a real repo. v1.2's layer cache used `serde_norway` (YAML) for debug-friendliness; consider `bincode` only when reading/writing solved-graph blobs at scale. Until then, keep `serde_norway` for human-debuggable cache. |
| `dashmap` | 6.2.1 | Concurrent hash map for solver structures that need **inter-thread** sharing during the parallel fixed-point loop. | Add **only** if profiling shows our solver's parallel phase contended on a `Mutex<FxHashMap>`. v1.3's solver should default to **single-threaded determinism** with rayon parallelism around it; introduce dashmap only when a clear hotspot exists. Easy to misuse — order-dependent iteration in dashmap will break determinism. |
| `ahash` | 0.8.12 | Faster hasher for `String`/byte-key maps. | Prefer `rustc-hash` (`FxHasher`) for integer keys. Use `ahash` **only** if benchmarks show string-keyed hashing is hot (unlikely once we intern everything via `string-interner`). |
| `memmap2` | 0.9.10 | Memory-mapped reads for large cached blobs. | Defer to v1.4+ unless cache blob I/O measurably dominates. |
| `zerocopy` | 0.8.48 | Zero-copy parsing of cache blobs into typed views. | Defer to v1.4+. Adds non-trivial trait-derive complexity. |
| `rkyv` | 0.8.16 | Zero-copy archived serialization. | Defer indefinitely. Would compete with `bincode` and introduce a different mental model. |

### Development Tools

| Tool | Purpose | Notes |
|---|---|---|
| Existing `insta` 1.47.2 | Continued snapshot coverage for new MIR/CFG/solver/RTA/token-flow facts. | New snapshot families: `mir.snap`, `places.snap`, `calls.refined.snap`, `tokens.snap`, `props.snap`, `go-rta.snap`, `adaptation.snap`, `go-sidecar.ndjson.snap`. |
| Existing `proptest` 1.11.0 | Property tests for solver monotonicity, lattice laws, RTA fixed-point convergence, token-set union associativity. | Already in v1.2 for domain-law tests; extend to refined-call and points-to laws. |
| Existing `assert_cmd` 2.2.1 + `predicates` 3.1.4 | CLI integration tests including sidecar-missing diagnostics. | Use `tempfile` 3.27.0 + a fake `polint-go-frontend` stub binary on PATH for sidecar tests where Go toolchain is not available. |
| `cargo-deny` | Optional: license/vulnerability audit before promoting new deps to release. | Not a code dep; recommended in CI before each v1.3 PR that adds a crate. |

## Installation

Workspace `Cargo.toml` additions (only the new entries — keep all existing pins):

```toml
[workspace.dependencies]
# ... existing ...
fixedbitset    = "0.5.7"
roaring        = "0.11.4"
hashbrown      = "0.17.1"
rustc-hash     = "2.1.2"
smallvec       = "1.15.1"
indexmap       = "2.14.0"
string-interner = "0.20.0"
blake3         = "1.8.5"
which          = "8.0.2"

# Bump (non-breaking patch/minor)
oxc_allocator  = "0.133.0"
oxc_ast        = "0.133.0"
oxc_parser     = "0.133.0"
oxc_resolver   = "11.19.2"
oxc_semantic   = "0.133.0"
oxc_span       = "0.133.0"
tree-sitter    = "0.26.9"
```

Go sidecar (separate `go-frontend/` directory at the repo root, not part of the Rust workspace):

```bash
# go-frontend/go.mod
go 1.24
require golang.org/x/tools v0.45.0
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|---|---|---|
| Out-of-process Go sidecar via NDJSON | In-process `cgo`-hosted Go runtime | Never for v1.3. Reconsider only if (a) a maintained Rust binding to `go/packages` appears (none exists as of 2026-05-27), and (b) cross-compilation is no longer a release concern. |
| `string-interner` 0.20.0 | `lasso` 0.7.3 | If we need a thread-safe interner shared across rayon workers and `string-interner`'s `StringBackend` arena allocation becomes a hotspot. lasso ships `ThreadedRodeo` for that case. |
| `fixedbitset` 0.5.7 | `bitvec` 1.x | bitvec gives bit-fiddly views over arbitrary memory; we don't need that. fixedbitset is simpler and what petgraph itself uses. |
| `roaring` 0.11.4 | `croaring` (FFI binding) | croaring is the C implementation with potentially faster constants but adds `unsafe`/FFI surface that breaks our `unsafe_code = "forbid"` lint. Stay pure-Rust. |
| Hand-rolled worklist solver | `differential-dataflow` / `datafrog` / `crepe` (Datalog crates) | Datalog frameworks were evaluated in `research/incremental-query-engine/` and explicitly deferred. They are appealing for declarative analysis but introduce a foreign mental model, opaque scheduling, and tricky cache integration. Hand-rolled worklist with `fixedbitset`/`roaring` keeps determinism and debuggability. Revisit in v1.4+ if specific provider families become unmaintainable. |
| Hand-rolled RTA in Rust over emitted SSA facts | Consume `golang.org/x/tools/go/callgraph/rta.Analyze` output directly from the sidecar | The latter is faster to ship and is the benchmark oracle (so matches x/tools-RTA expectations exactly). The former gives polint stable internal identities even across Go toolchain version changes. **Phase decision, not stack decision.** Both are compatible with this stack. |
| NDJSON wire format | protobuf via `prost` | Only if a real-world benchmark proves JSON parsing dominates sidecar latency. Until then, NDJSON wins on debuggability and zero new deps. |

## What NOT to Use

| Avoid | Why | Use Instead |
|---|---|---|
| **CodeQL / Soot / WALA / Doop / Jelly / PyCG as runtime engines** | Explicitly out of scope per `research/call-graphs/RECOMMENDED_IMPLEMENTATION.md`. Their licenses, toolchain footprints, and "black-box engine" philosophies are incompatible with polint's product model. Jelly is the **benchmark oracle for JS/TS** but **never** a runtime dependency. | Native Rust implementations in `crates/polint/src/analysis/`. |
| **`cgo`-hosted Go runtime inside polint** | Breaks rayon determinism, panic isolation, cross-compilation, and our `unsafe_code = "forbid"` workspace lint. | Out-of-process sidecar (above). |
| **`differential-dataflow`, `datafrog`, `crepe` as required solver foundation** | Excellent for incremental relational analysis but their scheduling models conflict with polint's deterministic layer-cache and provenance/precision sidecar metadata. Adoption would require rewriting `analysis::*` cache and validation contracts. | Hand-rolled worklist with `fixedbitset`/`roaring`/`smallvec`. |
| **`tokio` / `async-std` / `async` traits in the solver core** | The solver must be deterministic and reproducible. Async introduces scheduler-dependent ordering. Sidecar I/O is synchronous `Stdio::piped()`. | Synchronous code + `rayon` for parallel passes around the solver's deterministic inner loop. |
| **Capnp / FlatBuffers / rkyv for the sidecar wire format** | Zero-copy benefits negligible at our data sizes; adds significant schema toolchain burden on Go side. | NDJSON + `serde_json` (Rust) + `encoding/json` (Go). |
| **`once_cell::sync::Lazy` for solver caches** | Global mutable state breaks the layer-cache's input-snapshot invariant: any global stash becomes a hidden cache-key input. | Per-`AnalysisKernel` borrowed state, passed explicitly. |
| **Custom hash functions inside `Hash` impls of fact IDs** | Fact IDs must serialize/hash to the same bytes across runs. Custom hashers attached to types make digest changes invisible. | Use plain `derive(Hash)`, then pick the hasher at map-construction time (`FxHasher` for solver, default for serde). |
| **`tree-sitter-typescript` / re-introducing tree-sitter for TS/JS** | Oxc's `oxc_semantic` is the v1.3 binding layer; switching parsers would invalidate every fact family. | Keep Oxc; add an `oxc_semantic`-based scope/binding consumer (no new crate). |
| **A second graph crate alongside `petgraph`** | `petgraph` is already wired for module/symbol/call indexes. Adding a parallel graph library (e.g. `graphlib`, `gryf`) fragments code paths. | Extend petgraph with custom node/edge weights for new graphs; use hand-rolled `Vec`/`SmallVec<[..; N]>`-based CSR storage where petgraph overhead is measurable (points-to constraint graph, function-token flow graph). |
| **`hex` / `base64` re-encoding of digest bytes throughout the codebase** | Mixed encodings of digests become cache-key footguns. | Pick one canonical encoding for serialized digests (`blake3::Hash::to_hex()`) and use it everywhere; never `.to_string()` an arbitrary byte vec. |
| **Heap-allocated `Box<dyn Iterator>` in solver hot loops** | Vtable dispatch in inner worklist loops is a measurable hot-path regression. | Generic iterators or `for`-loops with concrete types. |

## Stack Patterns by Variant

**If the Go RTA Phase consumes `rta.Analyze` output directly:**
- The sidecar's responsibility expands: build SSA, run `rta.Analyze`, then serialize the callgraph plus reachable set as NDJSON edges.
- Rust side: ingest as `CallTargetFact`/`Reachability` directly, attaching polint identities via the symbol/file map the sidecar emits.
- Pro: minimal Rust solver code for Go.
- Con: ties polint's Go recall to whatever RTA quirks `x/tools` exhibits.

**If the Go RTA Phase reimplements RTA in Rust over emitted SSA-like facts:**
- Sidecar emits: package set, type set, method-set table, address-taken function set, SSA function signatures, `MakeInterface`/`Invoke`/`Call` instruction stream as NDJSON.
- Rust side: pure `analysis::go::rta` provider using `fixedbitset` for reached/address-taken/concrete-type sets, deterministic worklist.
- Pro: polint owns the algorithm, identities, provenance, and budgets.
- Con: more Rust code; must keep the algorithm's behavior aligned with x/tools.
- **Recommended starting point.** The hand-rolled provider is the architectural keystone (per `BOOTSTRAP-INTEGRATION.md`'s "OPAL separates type information providers from call graph clients").

**If the JS/TS function-token solver outgrows `fixedbitset`:**
- Switch the token-set storage **per fact family** to `roaring::RoaringBitmap`.
- Both crates can coexist; the decision is per fact, not project-wide.

**If the adaptation model layer needs schema validation:**
- Use `serde` + `serde_json` (already present) for model file parsing.
- Use the v1.2 extension-provider validation infrastructure for accept/reject — **do not introduce a new schema library** (`jsonschema`, `valico`). Validation is structural and lives in Rust types.

## Version Compatibility

| Package A | Compatible With | Notes |
|---|---|---|
| `oxc_*` @ 0.133.0 | `oxc_resolver` @ 11.19.2 | Oxc crates version-lock together; bump as a unit. |
| `fixedbitset` 0.5.7 | `petgraph` 0.8.3 | petgraph 0.8 already depends on fixedbitset 0.5.x — direct use shares the same dep. |
| `hashbrown` 0.17.1 | `indexmap` 2.14.0 | indexmap 2.14 uses hashbrown 0.17 internally; safe to share. |
| `string-interner` 0.20.0 | `rustc-hash` 2.1.2 | Default backend is fine; configure with `BuildHasherDefault<FxHasher>` for symbol-table interner. |
| Rust 2024 edition (rustc ≥1.95 per workspace) | All listed crates | All listed crates declare MSRV ≤1.85; we're well above. |
| Go sidecar `golang.org/x/tools` v0.45.0 | Go toolchain ≥1.24 | x/tools v0.45.0 dropped support for Go <1.24. Document the requirement in the sidecar README; polint Rust core does not depend on Go toolchain version. |
| `blake3` 1.8.5 | All other crates | `blake3` has zero impactful transitive deps for our use; do not enable the `rayon` feature (would conflict with our own rayon usage scheduling). |
| `bincode` 3.0 (if adopted) | `serde` 1.0.228 | bincode 3.0 changed the derive model from 1.x. If adopted, scope strictly to layer-cache modules; do not let it leak into public SDK types. |

## Determinism and Layer-Cache Invariants Checklist

Every new dep was screened against the v1.2 layer-cache invariants. Risks and mitigations:

| Risk | Mitigation |
|---|---|
| `hashbrown` / `dashmap` iteration order is undefined | Never serialize a `HashMap`/`DashMap` to a cache blob. All persisted outputs go through `IndexMap` or sorted `Vec`. |
| `roaring::RoaringBitmap` serialization version drift | Pin `roaring` exact version; include `roaring` crate version in the relevant layer-cache digest input. |
| `string-interner` symbol IDs are insertion-order dependent | Symbol IDs must be **internal-only handles**; never persist them. Persist the interned string (or its blake3 digest) instead. |
| `blake3` SIMD path vs. portable path could differ | blake3 outputs are bit-identical regardless of code path; safe to use. |
| Sidecar subprocess ordering | Sidecar must emit facts in a deterministic order (sort by package path, then by SSA function declaration position). Document in sidecar protocol spec. |
| `oxc_semantic` scope IDs change across Oxc versions | Include `oxc_semantic` version in the syntax-layer cache digest. v1.2 already digests rule/model versions; extend to library versions for new fact families. |
| `which` resolution depends on `$PATH` at runtime | `polint-go-frontend` location must be in the sidecar input digest (resolved path + binary file digest). Already covered by the "lifecycle/toolchain digest" plan in `research/incremental-query-engine/`. |

## Integration Points Against Existing Stack

| Existing Component | Integration with v1.3 Additions |
|---|---|
| `oxc_semantic` 0.133 | Drives JS/TS scope/binding/`SymbolTable`/`ScopeTree` consumption for the function-token solver. **No new parser dep.** v1.3 deepens what we read from Oxc, not what produces it. |
| `oxc_resolver` 11.19.2 | Already handles ESM/CJS/tsconfig path resolution. v1.3 module-graph extension uses its `tsconfig.json` `paths` and package-entry resolution to build the JS/TS module graph used by token propagation. **No replacement needed.** |
| `tree-sitter-go` 0.25.0 | **Stays as-is** for fast-tier Go facts. Semantic Go (RTA, method sets, receiver types) comes from the sidecar; tree-sitter remains the fall-back when the sidecar reports `SetupMissing`. |
| `petgraph` 0.8.3 | Continues to host the module graph, symbol graph, refined-call graph views. v1.3 adds new graphs (token-flow graph, points-to constraint graph) as hand-rolled CSR structures next to petgraph — **petgraph is not replaced, it is supplemented** where the solver's hot loop requires direct access to neighbor slices. |
| `serde_json` 1.0.149 | Reused for sidecar NDJSON wire and existing debug/eval JSON. **No new serializer adopted.** |
| `serde_norway` 0.9.42 (YAML) | Existing layer cache format. **Keep for v1.3** (cache debuggability matters during a fast-moving milestone). Reconsider `bincode` only after profiling. |
| `rayon` 1.12.0 | Continues to schedule per-file parallelism. The solver's fixed-point loop stays **single-threaded** for determinism; rayon parallelizes around it (per-function pre-pass, per-package post-pass). |
| `polint-macros` 0.1.7 | Rule manifest macros stay as-is; no v1.3 changes here. |

## Sources

- Workspace `Cargo.toml` and `crates/polint/Cargo.toml` (read 2026-05-27) — current pinned versions.
- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — primary v1.3 design source; sets sidecar/JS-token/object-model expectations.
- `research/call-graphs/RECOMMENDED_IMPLEMENTATION.md` and `research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md` — native-engine principle, refused dependencies list, fact-model architecture.
- `research/type-alias-points-to/RECOMMENDED_IMPLEMENTATION.md` — bounded Andersen solver shape (bitsets, SCC collapse, delta propagation, worklist budgets), aliases provider stack.
- `research/ROADMAP.md` — confirmed PR ordering and the rule that "extension code digest participates in cache keys."
- crates.io API (queried 2026-05-27) for current stable versions: `fixedbitset` 0.5.7, `roaring` 0.11.4, `hashbrown` 0.17.1, `rustc-hash` 2.1.2, `smallvec` 1.15.1, `indexmap` 2.14.0, `string-interner` 0.20.0, `blake3` 1.8.5, `which` 8.0.2, `oxc` 0.133.0, `oxc_resolver` 11.19.2, `tree-sitter` 0.26.9, `tree-sitter-go` 0.25.0, `petgraph` 0.8.3, `bincode` 3.0, `dashmap` 6.2.1, `ahash` 0.8.12, `lasso` 0.7.3 (alternative), `prost` 0.14.3 (rejected), `rmp-serde` 1.3.1 (rejected), `rkyv` 0.8.16 (deferred), `memmap2` 0.9.10 (deferred), `zerocopy` 0.8.48 (deferred), `sha2` 0.11.0 (not used; prefer blake3).
- pkg.go.dev `golang.org/x/tools/go/packages` and `golang.org/x/tools/go/ssa` (queried 2026-05-27) — confirmed v0.45.0 (2026-05-08), API stable, sidecar approach is the established pattern (gopls, golangci-lint, staticcheck).

---
*Stack research for: v1.3 Graph Engine Precision (subsequent milestone)*
*Researched: 2026-05-27*
