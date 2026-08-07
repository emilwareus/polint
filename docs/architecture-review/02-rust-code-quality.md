# Rust Code Quality & Idiom Audit — `polint`

**Scope:** the *code*, not the architecture. Judged against the bar set by rust-analyzer, ruff, oxc, cargo, and tokio.
**Commit:** `1263208a` (branch `biarritz`), 2026-07-28.
**Measured:** 267,710 LOC Rust across `crates/`; `crates/polint/src` = 253,559 lines, of which **125,273 are production** and **128,286 are inline `#[cfg(test)]`** (301 test modules), plus 13,206 lines in `crates/polint/tests/`.

**One-paragraph verdict.** The *discipline* here is top-tier: `cargo clippy --workspace --all-targets --all-features -- -D warnings` is clean, `unsafe` is `forbid`den workspace-wide with exactly one audited FFI exception, error types are domain-scoped `thiserror` enums, rules run inside `catch_unwind`, and CI runs 12 jobs including a determinism gate and a public-API-leak gate. That is better hygiene than most Rust infrastructure projects of this size. The *engineering* is not top-tier: the core data model is stringly-typed to a degree that no serious analysis engine can sustain — 229 `stable_key: String` fields, 318 `BTreeMap<String, _>`, 476 `Vec<String>`, zero interner, and the Go RTA fixpoint's reachable set is a `BTreeSet<String>`. Extensibility is 999 `Language::` match sites and four competing language taxonomies with no adapter trait. Parallelism is four `par_iter` calls in 253k lines. The codebase is *clean* but *not fast, not extensible, and not memory-scalable*, and one class of bug (dropped parse errors) undermines the soundness story the product is sold on.

---

## (a) Scorecard

| # | Dimension | Grade | Evidence |
|---|-----------|-------|----------|
| 1 | **Error handling** | **B+** | 8 domain `thiserror` enums (`repo_fs.rs:11`, `baseline.rs:13`, `config/mod.rs:13`, `fs/mod.rs:14`, `analysis/error.rs:2`, `store/migrations.rs:30`, `adaptation/loader.rs:9`, `rule_error.rs:16`); `anyhow` (265 refs) confined to CLI/plumbing, never the public type. Only **17 `.unwrap()` and ~46 `.expect()` in true production paths** (raw greps say 34/297; the rest are `#[cfg(test)]`). `expect` messages name the violated invariant, not "should work". Rules run under `catch_unwind` at `core/mod.rs:7694,7722` and `analysis_plan.rs:421,437` — a panicking third-party rule yields one diagnostic, not a dead run. Malformed sources produce diagnostics with real spans and analysis continues on the partial AST (`ts/adapter.rs:489-523`, `go/adapter.rs:463-490`). **Marked down** for the parse-error-drop soundness gap (D1) and the 15 `unwrap()`s at `cli/mod.rs:2786-2800`. |
| 2 | **Data modelling** | **D** | **No string interner anywhere** (no `lasso`/`smol_str`/`compact_str`/`ustr`; `Box<str>` = 0 occurrences; `Arc<str>` = 32, nearly all in `analysis/identity/`). The only interner in 267k LOC is `js_points_to/solver.rs:173` `intern_token`, scoped to one solver. IDs *are* newtyped well — 14 `Copy` newtypes at `core/mod.rs:143-181` (`FileId(u32)`, `SymbolId(u64)`, …) — but every fact then carries a **redundant `stable_key: String`** beside its integer ID (229 field declarations, 544 `.clone()` sites). `SymbolFact` (`core/mod.rs:510`) = ~176 B inline + ~220-360 B heap over **3 allocations**. `oxc_allocator` is a dependency but the arena AST is copied out to owned `String`s immediately (`lower_ts.rs`: 65 `.to_string()` vs 11 `as_str()`). |
| 3 | **Allocation & cloning** | **D+** | 1,689 production `.clone()`, 1,724 `.to_string()`, 1,432 `format!`. **`Cow` appears 12 times in 253k lines.** The stated constraint (`AGENTS.md:17` "avoid cloning large source strings") **is** honoured — `SourceFile.source: Arc<str>` (`core/mod.rs:263`) — but only for source text; derived identity strings now dominate the profile instead. `go_rta/fixpoint.rs` alone has 202 `.to_string()`. `sdk/scope.rs:74` allocates a `format!("./{value}")` on every call in a path its own doc-comment describes as running "once per fact row (every file, function, and literal)". |
| 4 | **Trait design / extensibility** | **D** | 999 `Language::` occurrences across ~100 files; `Language` (`core/mod.rs:182`) is `pub`, in the SDK prelude, and **not `#[non_exhaustive]`**. Four parallel language taxonomies with no conversions: `Language`, `LanguageTag` (`analysis/identity/facts.rs:30`), `LanguageScope` (`analysis_kernel/provider.rs:57`), `RuleLanguage` (`cli/mod.rs:796`). **No language adapter trait exists** — `go/adapter.rs` and `ts/adapter.rs` are free functions called by hardcoded path from `analysis_kernel/mod.rs:191,209`. Of 17 traits, only 4 are real extension points (`SolverPolicy`, `AbstractDomain`, `SummaryDomain`, `BenchmarkAdapter`) and none is language-related. The 23-entry `PROVIDER_MANIFESTS` table (`analysis_kernel/provider.rs:255`) is **metadata only** — execution is a hand-written 877-line function. **Counterweight:** the rule SDK proc-macro, which derives `Capabilities` from typed fact-view parameters (`polint-macros/src/lib.rs:29-35`), is genuinely excellent design and would grade **A-** on its own. |
| 5 | **File / function size** | **C-** | 6 files >4,000 lines. Worst function crate-wide: `analysis_kernel/mod.rs:92-968` — **`run`, 877 lines**, 6 boolean gate flags, 22 top-level branches, one `let mut db` threaded through every stage. `impl AnalysisDb` = **4,823 lines / 288 methods** (`core/mod.rs:966-5789`). `impl TsValueFlowCollector` = **6,444 lines / 163 methods** (`ts_value_flows.rs:434-6878`). Mixed verdict: `ts_value_flows.rs` and `validation.rs` are decomposable domain complexity (flat, cohesive, just unbounded); `core/mod.rs`, `cli/mod.rs`, and `ts/adapter.rs` are accreted — 5, 3, and 5 unrelated concerns respectively. |
| 6 | **Testing** | **B-** | 2,429 unit tests vs 174 integration tests; tests are **51% of all source lines**. Real quality gates exist: a seeded 10-permutation determinism gate, a public-surface-leak gate compiled out-of-workspace, a polyglot canary, a SARIF-shape validator. **But:** `insta` is used for **5 assertions in exactly one file** (`diagnostics/mod.rs`) — snapshot testing is the natural idiom for an analyzer and is essentially unused; `proptest` = **4 blocks**; `tests/cli.rs` is a **12,166-line monolith**; no fuzzing, no `miri`, no coverage in CI; and there are **9 `include_str!("*.rs")` meta-tests that assert on the substring content of the project's own source** (`ts/tests.rs:598,670,684,721`, `semantic_graph/build.rs:2491,2520`, `summaries/builder.rs:979`, `mir/body.rs:246-248`). |
| 7 | **Lints & tooling** | **A-** | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` **passes clean** (verified, 44s). CI (`.github/workflows/ci.yml`) = 12 jobs: fmt, rustdoc `-D warnings`, `cargo deny --all-features --locked`, MSRV 1.95 check, clippy, tests on ubuntu/macos/windows, gates, install smoke, SARIF upload. `unsafe_code = "forbid"` workspace-wide; `polint` downgrades to `"deny"` *only* so one audited `getrusage`/`K32GetProcessMemoryInfo` FFI (`eval/bench/measure.rs:29,82,94`) can carry an explicit allow — and this is documented in the manifest. Escape hatches are few and honest: **91 `#[allow]` in prod / 80 in test**, categorized as 42 `dead_code`, 31 `clippy::too_many_arguments`, 10 `unreachable_pub`, 2 `unused_imports`. **Missing:** `miri`, fuzzing, coverage, any `[profile]` section, and the lint table is thin (no `clippy::pedantic`, no `missing_debug_implementations`, no `missing_docs` outside `sdk`). |
| 8 | **Concurrency** | **C** | **Four** rayon call sites in the entire 253k-LOC crate: `fs/mod.rs:133` (parallel file read), `ts/adapter.rs:306` and `go/adapter.rs:267` (parallel parse), `core/mod.rs:7738` (parallel rule execution). Everything between — symbol graph, MIR lowering, CFG, call graph, the RTA fixpoint, validation — is **single-threaded**. Upside: **zero `Mutex`**, 3 `RwLock`, 39 `Arc<`, and determinism comes free from pervasive `BTreeMap`/`BTreeSet` ordering. `unsafe` is confined to the 3 audited FFI lines. Downside: this is a hard ceiling, and the one piece of shared mutable state is an **unbounded, never-evicted process-global** `OnceLock<RwLock<HashMap<String, Option<GlobMatcher>>>>` at `sdk/scope.rs:52` sitting inside the rayon rule loop. |
| 9 | **Documentation** | **B** | `#![deny(missing_docs)]` on `sdk/mod.rs:9` and `RUSTDOCFLAGS=-D warnings` in CI — but the crate is almost entirely `pub(crate)` (only `runner` + `sdk` are `pub`, `lib.rs:7-8`), so the deny covers ~180 public items and nothing else. Overall comment density 3.5%; solver hot spots reach 20-23% (`go_rta/inputs.rs` 23.4%, `go_rta/fixpoint.rs` 20.9%). Those dense comments encode *review history* rather than intent — `fixpoint.rs:98-150` cites "D-07", "CR-01", "FIX 1", "FINDING 7", "R3", "IN-03" in 40 lines of prose. Future readers cannot resolve those tokens. |
| 10 | **Dependency hygiene** | **A-** | 273 crates in `Cargo.lock`, **26 direct dependencies** — lean for a multi-language analyzer. `cargo tree -d` shows only transitive duplicates (`fastrand` via `phf_generator`/`tempfile`, `foldhash` via `hashbrown`), none owned. `cargo-deny` enforces advisories + a 5-license allowlist + `wildcards = "deny"` + registry pinning. Two nits: `tempfile` is a **production** dependency used almost entirely in tests (1 real prod use, `rule_test.rs:257`), and `rusqlite` with `features = ["bundled"]` compiles SQLite from C source on every clean build. |
| 11 | **Compile-time / crate granularity** | **D** | **253,559 lines in a single crate.** Touching `ts_value_flows.rs` costs **14.1s** for `cargo check -p polint` alone (measured); a full workspace `--all-targets` clippy is 44s warm. rust-analyzer ships ~40 crates, ruff ~30, oxc ~25 — precisely to get crate-level compilation parallelism and incremental isolation. `polint` gets neither. This also forces the `pub(crate)`-everything + public-surface-leak-gate contortion instead of ordinary module privacy. |

---

## (b) Top 15 concrete defects

### D1 — Parse errors discarded at 9 of 12 TS re-parse sites (SOUNDNESS)
`crates/polint/src/analysis/calls/ts_value_flows.rs:63-71`
```rust
let parsed = Parser::new(
    &allocator,
    file.source.as_ref(),
    SourceType::from_path(&file.path).unwrap_or_default(),
).parse();
if parsed.panicked && parsed.program.body.is_empty() {
    continue;
}
```
`parsed.errors` is never read. A file with recoverable syntax errors yields a **truncated value-flow graph with no unknown marker** — the analyzer under-approximates silently, which is exactly the failure mode a static analyzer must never have. Same at `ts_value_flows.rs:324,396`, `semantic_graph/build.rs:235,284,952`, `ts/object_model/extract.rs:31`, `ts/scope/extract.rs:30`, `ts/inventory/extract.rs:25`. Only three sites do it correctly: `ts/adapter.rs:489`, `symbol_graph/ts.rs:165`, and `mir/lower_ts.rs:86-99`.
**Fix:** extract `lower_ts.rs:86-99`'s error→`unsupported`-span mapping into a shared `parse_ts(file) -> (Program, Vec<UnsupportedFact>)` helper and route all 12 sites through it. **Effort: M.**

### D2 — No string interner; every fact carries a redundant `stable_key: String`
`crates/polint/src/core/mod.rs:510` (`SymbolFact`), and 228 other declarations.
```rust
pub struct SymbolFact {
    pub id: SymbolId,          // already a unique u64
    pub name: String,          // 24 B + heap
    pub qualified_name: String,// 24 B + heap
    ...
    pub stable_key: String,    // 24 B + 150-300 B heap — the id, restated in text
}
```
~400-540 B and 3 mallocs per symbol; `ReferenceFact` (`core/mod.rs:548`) is worse and references outnumber symbols 5-10:1. `MirStatement`, `MirTerminator`, `MIR body` (two Strings), `place-fact record`, `CallSiteFact`, `CallTargetFact` all carry the same field at per-*node* cardinality.
**Fix:** `StableKeyId(u32)` + an interner in `AnalysisDb`; keep the text side-table for serialization only. This also converts 318 `BTreeMap<String, _>` lookups into integer compares. **Effort: L.**

### D3 — The Go RTA fixpoint's worklist is a `BTreeSet<String>`
`crates/polint/src/analysis/solver/go_rta/fixpoint.rs:104,112,118,157,176,239,241`
```rust
let mut reachable: BTreeSet<String> = inputs.roots.clone();
let mut frontier:  BTreeSet<String> = inputs.roots.clone();
let mut edges_by_key: BTreeMap<String, DerivedEdgeFact> = BTreeMap::new();
...
if !reachable.contains(callee) { newly_reachable.insert(callee.clone()); }   // :175-176
...
edges_by_key.entry(edge.stable_key.clone()).or_insert(edge);                 // :241
```
This is the innermost loop of the Go call-graph solver. Every reachability test is an O(log n) walk of full string comparisons over long common-prefixed keys (worst case for `BTreeMap` — the discriminating bytes are at the *end*); every newly-reached function heap-allocates.
**Fix:** the solver already has `SemanticNodeId`; run the fixpoint over `BTreeSet<SemanticNodeId>` (or a dense `FixedBitSet`) and materialize strings once at the boundary. Expect a 5-20× speedup on this pass. **Effort: M.**

### D4 — The same TS file is parsed up to 10 times per run
19 `Parser::new` sites (`rg -n 'Parser::new' crates/polint/src`); at least 10 of them re-parse the *same* `SourceFile` in different providers — `ts/adapter.rs:486`, `symbol_graph/ts.rs:162`, `ts/scope/extract.rs:30`, `ts/object_model/extract.rs:31`, `ts/inventory/extract.rs:25`, `semantic_graph/build.rs:235,284,952`, `mir/lower_ts.rs:80`, `ts_value_flows.rs:64`, `js_points_to/provider.rs:61`. Each allocates a fresh `oxc_allocator::Allocator`, parses, and throws the arena away.
**Fix:** a per-file parse cache keyed by `FileId`, with the arena owned by the kernel for the duration of the TS pipeline. This is the single largest available wall-clock win. **Effort: L** (lifetime plumbing is real work).

### D5 — `parse_source_type` copy-pasted verbatim 7 times
`ts/inventory/extract.rs:424`, `ts/object_model/extract.rs:1331`, `ts/scope/extract.rs:1323`, `ts/adapter.rs:550`, `symbol_graph/ts.rs:2151`, `semantic_graph/build.rs:1471`, `mir/lower_ts.rs:3107` — all identical:
```rust
fn parse_source_type(path: &Path) -> SourceType { SourceType::from_path(path).unwrap_or_default() }
```
…and `ts_value_flows.rs:67,327,399` inlines the body instead, so **there are two divergent strategies for deciding whether a file is TSX**. Combined with D1, a `.tsx` misclassified as JS parses to errors that are then discarded.
**Fix:** one `crate::ts::source_type(path)`. **Effort: S.**

### D6 — `Kernel::run` is 877 lines of gated straight-line mutation
`crates/polint/src/analysis_kernel/mod.rs:92-968`. Six boolean gates (`run_cross_file_analysis`, `run_semantic_pipeline`, `run_cfg_call_pipeline`, `run_full_refinement_pipeline`, `run_data_flow_pipeline`, `compact_domain_materialization`) computed up front, 22 top-level branches keyed off them, one `let mut db` threaded through every stage, and each provider invoked by hardcoded path + stringly-typed manifest lookup.
**Fix:** a `PipelineGates` struct plus a `Vec<Stage>` where `Stage` is `fn(&mut KernelCtx) -> StageOutput`; the digest fan-in becomes data, not 22 hand-written branches. **Effort: L.**

### D7 — Five same-typed positional `Digest` parameters (silent-corruption hazard)
`crates/polint/src/analysis/domains/provider.rs:20-32`
```rust
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_abstract_domains_with_cache_stats(
    db: &mut AnalysisDb, input_snapshot: &InputSnapshot, manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> AbstractDomainsProviderOutput
```
Five interchangeable `Digest` arguments in a row. Transposing any two **compiles silently** and produces a wrong cache key → stale analysis results served as fresh. This shape repeats across the 31 `#[allow(clippy::too_many_arguments)]` sites (`analysis/domains/provider.rs:20,46,72,128`, `analysis/identity/provider.rs:31,134`, `analysis/semantic_graph/provider.rs:58,449`, …). The `allow` is suppressing a lint that is correctly diagnosing a real hazard.
**Fix:** `struct UpstreamDigests { semantic_mir: Digest, cfg: Digest, calls: Digest, symbol_graph: Digest, module_topology: Digest, syntax: Vec<Digest> }`. Deletes 31 allows and makes transposition a type error. **Effort: M.**

### D8 — `glob_matches` allocates in a path its own doc-comment calls hot
`crates/polint/src/sdk/scope.rs:74`
```rust
Some(matcher) => matcher.is_match(value) || matcher.is_match(format!("./{value}")),
```
The comment 30 lines above (`scope.rs:36-46`) explains that this runs "once per fact row (every file, function, and literal…) — hundreds of thousands of times". The `format!` allocates on **every** call, and the `cached_matcher` lookup above it hashes the pattern `String` and takes an `RwLock` read — inside the rayon rule loop.
**Fix:** match against `value.strip_prefix("./").unwrap_or(value)` (zero-alloc), and hoist the compiled matchers into `RuleOptions` at config-load time so no lock or hash is taken per row. **Effort: S.**

### D9 — Unbounded, never-evicted process-global cache
`crates/polint/src/sdk/scope.rs:52-53`
```rust
static CACHE: OnceLock<RwLock<HashMap<String, Option<GlobMatcher>>>> = OnceLock::new();
```
The doc claims it is "bounded by the number of distinct patterns in `.polint.toml`", which is true for the CLI but **false for the library**: `polint` is published as a crate and `glob_matches` is `pub` in the SDK prelude, so any caller passing dynamic patterns leaks unboundedly for process lifetime. It is also the only shared-mutable state under `par_iter`.
**Fix:** move the cache into `RuleOptions`/`RuleCtx` (owned, scoped, lock-free). **Effort: S.**

### D10 — 15 `unwrap()` on a stringly-keyed lookup in the CLI
`crates/polint/src/cli/mod.rs:2786-2800`
```rust
public_fact_view("resolved_imports").unwrap(),
public_fact_view("module_graph").unwrap(),
... 13 more ...
```
`public_fact_view` (`cli/mod.rs:2812`) is a `match` over string literals. These are the only genuine production `unwrap()`s in the crate. Infallible today; a renamed capability panics `polint facts list` at runtime with no compile-time signal.
**Fix:** a `const FACT_VIEWS: &[FactViewKind]` array, or make `public_fact_view` take the enum. **Effort: S.**

### D11 — `LayerCacheReadStatus::Hit` doesn't carry its payload
`metrics.rs:78`, `go/adapter.rs:116`, `module_graph/mod.rs:653,1101`, `symbol_graph/mod.rs:154`:
```rust
.expect("layer cache hit should include ... payload")
```
Five `expect()`s all encoding the same convention — "`status == Hit` implies `value.is_some()`" — that the type system could enforce for free.
**Fix:** `enum LayerCacheReadStatus<T> { Hit(T), Miss, … }`. Deletes 5 panics. **Effort: S.**

### D12 — `AnalysisDb` is a 132-field god object with a 4,823-line impl
`crates/polint/src/core/mod.rs:658-825` (132 fields), `:966-5789` (288 methods). It holds every fact family, every index, and every optional store in one struct, and `core/mod.rs` additionally contains the ID newtypes, ~45 fact structs, a metadata/labeling layer (34 `*_metadata` + 32 `*_label` free functions), **and the entire rule engine** (`Capabilities`, `Rule`, `RuleCtx`, `RuleRegistry`, `run_rules`, `:7108-7825`). Nineteen consecutive `#[allow(dead_code)]` accessors at `core/mod.rs:2012-2100` are unused API surface kept alive by suppression.
**Fix:** split `core/{ids,facts,db,metadata,labels}.rs` and move the rule engine out of `core` entirely; delete the dead accessors. **Effort: L.**

### D13 — `missing_fact_metadata`: 245 lines of copy-paste
`crates/polint/src/core/mod.rs:3555-3799` — roughly 40 near-identical `for x in self.y() { push(...) }` blocks. Textbook accretion; every new fact family adds another block and there is nothing preventing one from being forgotten.
**Fix:** a `macro_rules!` over a `FactFamily` table, or a `FactFamily` iterator so the exhaustiveness is checked. **Effort: M.**

### D14 — `#[non_exhaustive]` applied to enums but never to the public fact structs
`core/mod.rs` marks 11 enums `#[non_exhaustive]` (`:374,383,391,402,412,424,448,459,470,486,499`) — but `SourceFile` (`:258`), `FunctionFact` (`:268`), `SymbolFact` (`:510`), `ReferenceFact` (`:548`) and their ~30 siblings have **all-public fields, no `#[non_exhaustive]`**, and are re-exported from `sdk::prelude` (`sdk/mod.rs:28-42`). Adding one field to the fact model is a **semver-major break for every rule pack**. `Language` (`core/mod.rs:182`) is likewise exhaustively matchable downstream — adding Python breaks every consumer.
**Fix:** `#[non_exhaustive]` on every prelude-exported struct and on `Language`. Do it before 1.0; it is free now and impossible later. **Effort: S** (plus fixing internal struct-literal construction sites).

### D15 — Meta-tests that grep the project's own source text
`crates/polint/src/ts/tests.rs:670-680`
```rust
let production_only = include_str!("adapter.rs");
let forbidden = concat!("file.source", ".to_string()");
assert!(!production_only.contains(forbidden),
    "parse_ts_file should not allocate a full String copy of the source");
```
and `ts/tests.rs:684-697`, which asserts `adapter.rs` textually contains `"fn parse_source_type(path: &Path) -> SourceType"`. Nine such `include_str!("*.rs")` tests exist. They break on rustfmt changes, pass on semantically-wrong code, and — most tellingly — the project ships a static-analysis engine that could enforce these as real rules against itself.
**Fix:** delete; replace with `polint` rules dogfooded on the repo, or with an allocation-counting test. **Effort: S.**

---

## (c) Systemic patterns that will hurt at 10×

**1. Strings are the identity model, and that is a hard scaling wall.**
This is not fifteen local defects; it is one decision repeated 229 times. `stable_key: String` is on every fact family, at per-*node* cardinality (`MirStatement`, `place-fact record`, `CallSiteFact`). At 10× repo size the memory profile is dominated by identity text, not by facts; `BTreeMap<String, _>` lookups degrade super-linearly because keys share long prefixes and differ only in their tails; and `MirOutput::normalized` (`analysis/mir/body.rs:73-86`) sorts four vectors by 200-byte `memcmp`. rust-analyzer solved this in 2019 with `SmolStr` + salsa interning; oxc solved it with arena `Atom<'a>`; ruff solved it with `ustr`. `polint` has the newtype IDs already — it just never made them the identity. **Every other performance item on this list is downstream of this one.**

**2. Extension by exhaustive match, in a project whose whole thesis is "more languages".**
999 `Language::` sites, four competing language enums, zero adapter trait, providers wired by hardcoded path in an 877-line function. Adding Python is an O(files-in-crate) edit, not a new module. The ironic part: the crate already contains a *better* pattern — `polint-macros` derives rule capabilities from typed fact-view parameters. Types drive metadata there and nowhere else.

**3. `#[allow]` used to silence lints that are diagnosing real problems.**
31 `#[allow(clippy::too_many_arguments)]` on functions with five interchangeable `Digest` parameters (D7) and 42 `#[allow(dead_code)]`, 19 of them consecutive on unused `AnalysisDb` accessors. The allows are honest and greppable — but they are load-bearing suppression of a signal, not annotation of an exception.

**4. Correctness is enforced by convention and by grepping source text, not by types.**
`Hit ⇒ Some` (D11), "don't clone the source" (D15), "the 15 fact-view names exist" (D10), "the cache key includes every upstream digest" (D7). Each is a real invariant. None is expressible in the type system as written, and each has a suppression or a string-matching test standing in for it. The determinism gate and the public-surface-leak gate show the team *can* build real gates — that instinct should be pointed at the type system instead.

**5. Parallelism was never designed in.**
Four `par_iter` calls; parse and rule execution are parallel, and the entire 250k-line middle of the pipeline is not. There is no `Mutex` because there is no concurrency to protect. Retrofitting parallelism onto `Kernel::run`'s single `let mut db` (D6) and onto `AnalysisDb`'s 132 mutable fields (D12) is substantially harder than designing it in, and it gets harder every month.

**6. One 253k-line crate.**
14s to `cargo check` after touching one file, no crate-level build parallelism, no incremental isolation, and the `pub(crate)`-everything workaround that forces a bespoke out-of-workspace leak-probe test just to know what the public API is. Every peer project this size is 25-40 crates.

**7. Comments encode review history instead of intent.**
`go_rta/fixpoint.rs:98-150` cites "D-07", "CR-01", "FIX 1", "FINDING 7", "R3", "IN-03" across 40 lines. At 20-23% comment density in the solvers this is a lot of text whose referents live in `.planning/` and will be unresolvable to any future maintainer.

---

## (d) Prioritized remediation

Effort: **S** ≤ 0.5 day · **M** 0.5-2 days · **L** 3-10 days · **XL** > 2 weeks.

### P0 — soundness and silent-corruption (do first)

| # | Item | Effort | Why now |
|---|------|--------|---------|
| 1 | **D1** — route all 12 oxc parse sites through one helper that maps `parsed.errors` to `unsupported` facts | M | The only finding that makes the analyzer *wrong*. Under-approximation with no unknown marker is the one bug a soundness-marketed tool cannot have. |
| 2 | **D5** — single `crate::ts::source_type()`; delete 7 copies | S | Compounds D1; two divergent TSX-detection strategies today. |
| 3 | **D7** — `UpstreamDigests` struct; delete 31 `too_many_arguments` allows | M | Transposed digests serve stale results silently. Type-level fix, mechanical. |
| 4 | **D14** — `#[non_exhaustive]` on prelude structs and `Language` | S | Free before 1.0, impossible after. Every future fact field and every future language is currently a breaking change. |

### P1 — the scaling wall (start now, lands over a quarter)

| # | Item | Effort | Why |
|---|------|--------|-----|
| 5 | **D2** — `StableKeyId(u32)` interner in `AnalysisDb`; migrate `stable_key` family by family | L (per family) / **XL** total | The keystone. Unlocks 6, 7, and most of the memory budget. Migrate `SymbolFact`/`ReferenceFact` first (highest cardinality), then MIR, then callsites. |
| 6 | **D3** — RTA fixpoint over `SemanticNodeId`/bitsets | M | Self-contained, immediate 5-20× on the Go call-graph pass, and a proof-of-concept for 5. |
| 7 | **D4** — per-`FileId` parse cache with kernel-owned arena | L | Largest single wall-clock win; ~10 redundant parses per TS file today. |
| 8 | **D8 + D9** — hoist compiled globs into `RuleOptions`; drop the global cache | S | Removes an allocation and a lock from the hottest documented path. Two hours. |

### P2 — structure (unblocks everything above)

| # | Item | Effort | Why |
|---|------|--------|-----|
| 9 | **D6** — decompose `Kernel::run` into `PipelineGates` + a stage sequence | L | Prerequisite for pipeline-level parallelism and for provider extensibility. |
| 10 | **D12** — split `core/mod.rs` into `{ids,facts,db,metadata,labels}`; move the rule engine out; delete 19 dead accessors | L | 11k lines / 288-method impl is the main obstacle to anyone new contributing. |
| 11 | Split `ts_value_flows.rs` (the free-fn tail `:7328-8533` and the `*Targets` types `:6985-7479` lift out with zero refactoring), `cli/mod.rs` (scaffold+templates ≈1,300 lines, zero coupling to the check path), `ts/adapter.rs` (5 independent visitors) | M each | Mechanical; the seams are already there. |
| 12 | **Introduce a `LanguageBackend` trait**; collapse `LanguageTag`/`LanguageScope`/`RuleLanguage` into `Language` + conversions | L | Do this *before* language #3, not after. Cost grows with every `Language::` site added. |
| 13 | **Split the workspace into ~8-12 crates** (`polint-core`, `-syntax`, `-ts`, `-go`, `-mir`, `-solver`, `-kernel`, `-sdk`, `-cli`) | XL | Build parallelism, incremental isolation, and it deletes the need for the `pub(crate)`-everything + leak-probe machinery. Natural to do *with* item 10. |

### P3 — hygiene

| # | Item | Effort |
|---|------|--------|
| 14 | **D10** static fact-view table; **D11** `LayerCacheReadStatus::Hit(T)`; **D13** table-drive `missing_fact_metadata` | S / S / M |
| 15 | **D15** delete the 9 `include_str!("*.rs")` meta-tests; dogfood `polint` on itself instead | S |
| 16 | Split `tests/cli.rs` (12,166 lines) into `tests/cli/{check,review,facts,init,skill,…}.rs` | M |
| 17 | Adopt `insta` for diagnostic/SARIF/facts output (currently 5 assertions in 1 file) | M |
| 18 | Add `[profile.release] panic = "unwind"` so the `catch_unwind` rule-isolation guarantee is asserted, not inherited; add `panic::set_hook` to suppress backtraces from contained rule panics | S |
| 19 | Add CI jobs: `cargo miri test -p polint --lib` (the 3 FFI lines), `cargo llvm-cov` with a floor, and a `cargo-fuzz` target on the Go/TS parse entry points | M |
| 20 | Widen the workspace lint table: `clippy::pedantic` (allow-by-exception), `missing_debug_implementations`, `clippy::needless_pass_by_value`; move `todo`/`unimplemented` from `warn` to `deny` | S |
| 21 | Replace review-ticket comment tokens ("D-07", "CR-01", "FIX 1") with intent prose or resolvable links | M |
| 22 | Move `tempfile` to `dev-dependencies` (gate `rule_test.rs:257` behind a feature) | S |

---

### What is already good and should not be touched

The rule-authoring SDK — `#[polint::rule]` deriving `Capabilities` from typed fact-view parameters (`polint-macros/src/lib.rs:8-35`), the opaque `RuleError(#[from] anyhow::Error)` boundary (`rule_error.rs:16`), the `catch_unwind` isolation (`core/mod.rs:7722`), and infallible-by-construction fact views — is the best-designed part of this codebase and is genuinely competitive with anything in the ecosystem. The CI gate set (determinism under 10 seeded permutations, public-surface leak probe, polyglot canary, SARIF-shape validation, MSRV, `cargo-deny`, 3-platform matrix) is stricter than most projects of this size. `unsafe` discipline is exemplary: `forbid` at the workspace, one `deny`-plus-documented-`allow` exception for two lines of `getrusage`/`K32GetProcessMemoryInfo` FFI, explained in the manifest itself. `SourceFile.source: Arc<str>` honours the one performance constraint the project wrote down. And the fact that `clippy --all-targets --all-features -- -D warnings` is *clean* across 267k lines is not nothing.

The gap is not care. It is that the care went into process gates rather than into the data model, and no amount of CI can compensate for `BTreeSet<String>` in a fixpoint loop.
