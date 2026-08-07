# 01 — Layering, Module Boundaries, and Dependency Architecture

**Scope:** whether polint's architecture can absorb 10x more languages, analyses, and agent-authored
rules. Not a code-quality review; a load-bearing-structure review.

**Method:** import-graph extraction over `crates/polint/src` (359 `.rs` files, 253,559 LOC),
`rg`-based cross-module reference counting, direct reading of `core/mod.rs`,
`analysis_kernel/{mod,provider}.rs`, `sdk/`, `runner/`, and timed `cargo check` / `cargo build` runs
on this machine.

**Verdict in one line:** the *fact model* (MIR, stable keys, provider manifests, capability planning,
incremental digests) is genuinely well-designed and ahead of most open-source analyzers — but it is
implemented with **zero abstraction boundaries**: 17 traits and 10 `dyn` sites in 267k LOC, a
26-mutually-recursive-pair module graph, one 877-line hardcoded pipeline function, and one 132-field
god object. The design is 10x-ready; **the wiring is not**.

---

## (a) The actual current architecture

### a.1 Nominal layering (what the module names imply)

```
                       cli / runner            ← entry points
                            │
                    sdk (facts, policy)        ← rule-author contract
                            │
                    policy_queries              ← firewall
                            │
                    analysis_kernel             ← "kernel": orchestration + incremental + store
                            │
        analysis/*  (25 submodules: mir, cfg, calls, dataflow, solver, …)
                            │
              go / ts       │      module_graph / symbol_graph
                            │
                       core (IDs, facts, AnalysisDb)
                            │
                    fs / repo_fs / cache / config / diagnostics
```

### a.2 Real dependency graph (measured)

Edge = count of `crate::<target>` references. **Bold = wrong-direction edge.** All edges below are
production code unless marked `[test]`.

```
                    ┌──────────────────────── cli (5,010 LOC) ───────────────────────┐
                    │  →analysis 18  →core 11  →diagnostics 10  →cache 9              │
                    │  ↳ reaches 4 levels down: analysis::solver::store::SolverStore  │
                    │    (cli/mod.rs:2483), analysis::solver::engine::derive_edges    │
                    └────────────────────────────────┬───────────────────────────────┘
                                                     │
   runner (509) ──→ core 3, cache 3, repo_fs 6 ──────┤
                                                     │
   sdk (3,134) ──→ core 24 ──┐                       │
        ▲                    │                       │
        └── core →sdk 1 ◀────┘  ** CYCLE **          │
        └── policy_queries →sdk 2 / sdk →pq 5 ** CYCLE **
                                                     │
 ┌───────────────────────────────────────────────────▼──────────────────────────────┐
 │ analysis_kernel (26,022 LOC, 21 files)                                            │
 │   AnalysisKernel::run  = mod.rs:92–968  (877 lines, straight-line)               │
 │   PROVIDER_MANIFESTS   = provider.rs:255–884  (const [ProviderManifest; 23])      │
 │   ── 0 traits, 0 dyn, 0 registry in the whole directory ──                        │
 └──┬─────────────────────────────────────────────────────────────────────────────┬─┘
    │  →analysis 182  **→go 6**  **→ts 1**  →module_graph 14  →symbol_graph 5      │
    │  ◀── analysis →analysis_kernel 195 (95 of 218 files) ** THE BIG CYCLE **     │
    ▼                                                                              ▼
 ┌────────────────────────────────────────────┐         ┌──────────────────────────┐
 │ analysis/ (112,545 LOC, 218 files, 25 subs)│◀──78────│ ts/ (13,958)             │
 │  mir  cfg  calls  solver  data_flow        │───13───▶│ go/ (8,391)              │
 │  domains summaries entrypoints evidence …  │◀──34────│  ** CYCLE (both) **      │
 │                                            │───3────▶│                          │
 │  27,425 LOC of *language-named* files      │         └──────────────────────────┘
 │  live INSIDE this "neutral" layer (24%)    │
 └──────┬──────────────────────────────▲──────┘
        │ →core 318                    │ core →analysis 98  ** CYCLE **
        ▼                              │
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ core/mod.rs — 11,143 LOC, ONE FILE                                             │
 │   • 13 ID newtypes + Span/TextRange          (lines 141–256)                   │
 │   • `enum Language` — 6 closed variants      (line 184)  → 1,016 refs / 129 files│
 │   • 30 fact structs incl. TsComponentFact,                                      │
 │     TsClassFact, JsxAttributeFact            (lines 258–656)                    │
 │   • `struct AnalysisDb` — 132 FIELDS         (line 658, body to ~7100)          │
 │   • `struct Capabilities` — 24 bools         (line 7178)                        │
 │   • Rule / RuleCtx / RuleRegistry            (lines 7450–7660)                  │
 │   48 pub struct · 15 pub enum · 0 traits · 443 fns · 68 `use` lines             │
 └───────────────────────────────────────────────────────────────────────────────┘
        ▲              ▲                ▲                ▲              ▲
        │58            │43              │17              │43            │1
  module_graph    symbol_graph          go               ts        diagnostics
    (14,576)        (11,244)         (8,391)          (13,958)      (3,629)
                                                                        │
                                            diagnostics →analysis 5 ** CYCLE **

 ┌── infrastructure, also inverted ────────────────────────────────────────────────┐
 │ fs/ (398)   → core::AnalysisDb  (fs/mod.rs:2)   — file discovery builds the DB  │
 │ config/(906)→ analysis::solver::budget (config/mod.rs:8) — config knows solver  │
 │ cache/(1770)→ analysis_kernel::incremental (cache/mod.rs:168)                    │
 │ git/  (465) → core::ReviewChangeset (git/mod.rs:19)                             │
 └─────────────────────────────────────────────────────────────────────────────────┘

 eval/ (29,344 LOC) — `pub(crate) mod eval` (lib.rs:26), NOT cfg(test).
   `#![cfg_attr(not(test), expect(dead_code, …))]` (eval/mod.rs:1)
   → 11% of the crate compiles into every release build and is entirely dead there.
   → analysis 181, →analysis_kernel 28, →core 37.
```

### a.3 The headline metric

```
MUTUALLY-RECURSIVE MODULE PAIRS: 26  (of 27 top-level modules)
TOTAL cross-module edges: 155     TOTAL cross-module references: 2,137
```

Every top-level module except one participates in at least one production import cycle. In Rust this
is legal *inside* one crate and illegal *across* crates — which is precisely why the crate has never
been split, and why splitting it later will be expensive.

Top cycles by weight:

| A | A→B | B→A | B |
|---|---|---|---|
| `analysis` | 318 | 98 | `core` |
| `analysis` | 195 | 182 | `analysis_kernel` |
| `analysis` | 3 | 181 | `eval` |
| `analysis` | 78 | 13 | `ts` |
| `analysis_kernel` | 61 | 2 | `core` |
| `core` | 5 | 58 | `module_graph` |
| `analysis` | 47 | 5 | `diagnostics` |
| `core` | 4 | 43 | `ts` |
| `analysis` | 34 | 3 | `go` |
| `analysis` | 17 | 2 | `config` |
| `core` | 2 | 17 | `go` |
| `core` | 1 | 24 | `sdk` |

---

## (b) Evidence table — boundary violations

Severity: **P0** = blocks 10x scale directly · **P1** = large tax per new language/analysis ·
**P2** = hygiene.

| # | Violation | Evidence (`file:line`) | Count | Sev |
|---|---|---|---|---|
| **B1** | The "kernel" is a hardcoded straight-line function, not a scheduler. 23 stages called by direct path, `&mut AnalysisDb` threaded through all of them. | `analysis_kernel/mod.rs:92`–`968` (877 lines). Language stages hardcoded at `:191` (`crate::go::analyze_with_plan_options_and_cache_stats`), `:209` (`crate::ts::…`), `:425` (`crate::go::semantic::provider::…`). | 23 stages | **P0** |
| **B2** | The provider "model" carries a dependency DAG (`inputs`/`outputs` string lists) that **nothing ever sorts**. Order is source-statement order, pinned twice more in tests. | `analysis_kernel/provider.rs:2-11` (struct: no fn ptr, no closure, no trait object); `:255–884` (`const PROVIDER_MANIFESTS`); order asserted at `:936-965` **and duplicated at** `:967-996`. | 3 copies of the order | **P0** |
| **B3** | Manifest lookup is stringly-typed and panics on miss. | `analysis_kernel/mod.rs:1063-1068` `…find(\|m\| m.id == provider_id).unwrap_or_else(\|\| panic!("missing provider manifest {provider_id}"))` | 23 call sites | P1 |
| **B4** | Provider IDs are duplicated string literals with no single source of truth. | `"polint.module_graph"` at `core/mod.rs:124`, `module_graph/go.rs:28`, `module_graph/ts.rs:1584`. Also ×2 each: `polint.go.syntax` (`core/mod.rs:122`, `go/adapter.rs:21`), `polint.ts.syntax`, `polint.symbol_graph`, `polint.go.semantic`, `polint.entrypoints`. | 6 IDs duplicated | P1 |
| **B5** | **`core` ↔ `analysis` cycle.** The lowest layer imports 98 symbols from the highest analysis layer — `core/mod.rs` lines 1–107 are almost all `use crate::analysis::…`. | `core/mod.rs:1` `use crate::analysis::access_paths::facts::AccessPathFact;` … `:66` `use crate::analysis::solver::budget::BudgetStatus;` | 98 | **P0** |
| **B6** | **`analysis` ↔ `analysis_kernel` cycle, production code.** 95 of 218 `analysis/` files (43%) import the kernel; 7 of 21 kernel files import `analysis`. | `analysis/provider.rs:7-8`; `analysis/cfg/provider.rs:14-15`; `analysis/solver/provider.rs:37-38`; `analysis/data_flow/provider.rs:18-21` ↔ `analysis_kernel/validation.rs` (80 refs), `debug.rs` (47), `mod.rs` (23) | 195 / 182 | **P0** |
| **B7** | **Analyses bypass MIR and read language frontends directly.** MIR (`analysis/mir/op.rs:21`, 9 neutral op kinds) is a good neutral IR — but 17 files under `analysis/` skip it. | `analysis/semantic_graph/build.rs:40-54` (11 `use crate::ts::…`); `analysis/calls/ts_value_flows.rs` (44 `crate::ts::` refs, 11,898 LOC — the largest file in the repo, parses oxc AST itself at `:4-13`); `analysis/solver/go_rta/inputs.rs` (15 `crate::go::`); `analysis/refined_calls/provider.rs` (6); `analysis/identity/provider.rs:` (2, and **0** MIR refs) | 11 files →`ts`, 7 →`go` | **P0** |
| **B8** | **27,425 LOC of language-specific code lives inside the "language-neutral" `analysis/` tree** — 24% of it. | `analysis/calls/ts_value_flows.rs` 11,898 · `mir/lower_ts.rs` 3,952 · `entrypoints/recognizers_ts.rs` 2,397 · `mir/lower_go.rs` 1,913 · `entrypoints/recognizers_go.rs` 1,498 · `types/ts_js.rs` 1,264 · `types/go.rs` 848 · `cfg/lower_ts.rs` 827 · `cfg/lower_go.rs` 802 · `refined_calls/ts_js.rs` 696 · `refined_calls/go.rs` 636 · `identity/render/go_relstring.rs` 266 · plus dirs `solver/go_rta/`, `solver/ts_tokens/`, `solver/ts_object_model/`, `calls/js_points_to/` | 13 files + 4 dirs | **P0** |
| **B9** | **Language-specific IDs in the neutral ID module** — `ts/` must import its own identifiers *from* `analysis/`. | `analysis/ids.rs:34,37,40,43,46` define `TsInventoryFunctionId`, `TsInventoryCallsiteId`, `TsScopeId`, `TsBindingId`, `TsDirectBindingId` (5 of 53). Consumed at `ts/inventory/facts.rs:3`, `ts/scope/facts.rs:5`, `ts/binding/facts.rs:5`, … | 13 back-refs | P1 |
| **B10** | **`AnalysisDb` is a 132-field god object with 17 language-prefixed fields.** | `core/mod.rs:658`. Fields include `ts_components`, `ts_classes`, `jsx_attributes`, `ts_object_allocations`, `ts_property_writes`, `ts_property_reads`, `ts_receiver_bindings`, `ts_prototype_links`, `ts_object_model_store`, `go_semantic_packages`, `go_semantic_functions`, `go_semantic_callsites`, `go_semantic_method_sets`, `go_semantic_address_taken`, `go_semantic_instantiated_types`, `go_semantic_dynamic_dispatch`, `go_semantic_rta_edges`, `go_semantic_package_errors` | 132 fields | **P0** |
| **B11** | **`Capabilities` is a closed 24-bool struct in `core`, with language-specific members.** Every new capability = struct field + builder fn + `requested_names()` entry + `analysis_plan.rs` match arm + `analysis_kernel/provider.rs` list. | `core/mod.rs:7178` (struct), `:7204` `go_tests`, `:7218` `ts_components`, `:7220` `ts_classes`, `:7224` `jsx_attributes`, `:7355-7384` (`requested_names` hardcoded array), `analysis_plan.rs:687-688` (string match), `analysis_kernel/provider.rs:274,294,1126,1141` | 5 edit sites/cap | **P0** |
| **B12** | **`Language` is a closed 6-variant enum referenced 999 times in 129 files**, with an `is_ts_family()` special case used 49 times. | `core/mod.rs:184` (enum), `:194` (`from_path` — hardcoded extension table), `:209` (`is_ts_family`). Hotspots: `module_graph/mod.rs` 92 refs, `core/mod.rs` 64, `analysis_kernel/validation.rs` 39, `symbol_graph/mod.rs` 33, `sdk/facts.rs` 26 | 999 / 129 files | **P0** |
| **B13** | **`FactFamily` is a closed 88-variant enum** every analysis must extend. | `analysis_kernel/metadata.rs:6`; 1,110 `FactFamily::` references across 72 files | 88 variants | P1 |
| **B14** | **The layer/cache abstraction has been abandoned mid-flight.** `LayerKind` has 15 variants for 23 providers; the 10 most recently added providers shipped bespoke `cache_key.rs` files instead. | `analysis_kernel/incremental/keys.rs:40` (`LayerKind`), `:203-210` (`debug_assert!` hardcoding "syntax layers only exist for Go and TS"). 17 `*/cache_key.rs` files totalling 2,465 LOC; **only 1** (`analysis/extensions/cache_key.rs`) uses `LayerKey`. Persistent `LayerCacheStore` used by only 6 modules. | 16 of 17 bypass | **P0** |
| **B15** | Three pipeline-gating constants hold **identical values**, i.e. the gating taxonomy is already fictional. | `analysis_kernel/mod.rs:43`, `:47`, `:55-56` — all three are `&["calls", "control_flow", "dataflow"]` | 3 | P2 |
| **B16** | **Bottom-layer inversions.** File discovery constructs the god object; config schema imports solver internals; cache imports the kernel. | `fs/mod.rs:2` `use crate::core::{AnalysisDb, Language};` and `fs/mod.rs:100` `load_analysis_files(…) -> Result<AnalysisDb>`; `config/mod.rs:8` `use crate::analysis::solver::budget::{GoRtaSubBudget, JsObjectModelSubBudget, JsTokensSubBudget};`; `cache/mod.rs:168-169` returns `crate::analysis_kernel::incremental::LayerCacheStore` | 4 | P1 |
| **B17** | **CLI reaches 4 levels into solver internals.** | `cli/mod.rs:2483` `store: &crate::analysis::solver::store::SolverStore`, `:4238` `use crate::analysis::solver::engine::derive_edges`, `:4236` `analysis::points_to::facts::{PointsToPrecision, PointsToStatus}`, `:2296` `analysis::unknown_taxonomy::collect::…` | 18 | P1 |
| **B18** | **Diagnostics (output layer) imports analysis (deep layer).** | `diagnostics/mod.rs:787` `pub(crate) id: crate::analysis::ids::EvidenceBundleId`, `:2205` `crate::analysis::evidence::render::sarif_thread_flow_steps` | 5 | P1 |
| **B19** | **`eval/` (29,344 LOC) ships in the production library** and is dead there. | `lib.rs:26` `pub(crate) mod eval;` (no `#[cfg(test)]`); `eval/mod.rs:1-7` `#![cfg_attr(not(test), expect(dead_code, …))]`. Only 3 non-eval references, all in `#[cfg(test)]` (`analysis/calls/ts_value_flows.rs:11066,11115,11164`). | 11.6% of crate | P1 |
| **B20** | **`sdk` re-exports 70 internal types verbatim** from `pub(crate)` modules, with public fields and zero `#[non_exhaustive]`. | `sdk/mod.rs:29-40` (55 items from `crate::core`), `:41-45` (15 from `crate::diagnostics`), `:64-135` (`__private` exposing `AnalysisDb`). Prelude size hard-asserted at 115 in `tests/public_surface_leak.rs`. `FunctionFact` (`core/mod.rs:267`) has 9 public fields. `rg '#\[non_exhaustive\]' src/sdk/` → 0. | 70 of 115 | P1 |
| **B21** | **Test-only visibility escape hatches at scale** — evidence that even *inside* the crate, module internals must be punched through. | 559 `_for_test` occurrences across 57 files; 683 `#[cfg(test)]` blocks; 2,429 `#[test]` fns in 301 `src/` files vs 3 integration test files | 559 | P1 |
| **B22** | **Essentially no polymorphism to extend.** 17 traits and 10 `dyn` sites in 267k LOC. The only real plugin point is `SolverPolicy`. | `analysis/solver/policy.rs:77` (`trait SolverPolicy`), `analysis/solver/provider.rs:141-150` (`Vec<Box<dyn SolverPolicy>>`, hand-built vec). Everything else is inherent impls. | 17 traits | **P0** |
| **B23** | Duplicated Go sidecar source tree; only one copy is in `go.work`. | `go.work:4-6` lists `./tools/polint-go-symbols` and `./crates/polint/go-sidecar/polint-go-frontend`, but **not** `crates/polint/go-sidecar/polint-go-symbols`, which exists and has diverged (`diff` shows `internal/symbols/emit_test.go` differs). | 1 orphan tree | P2 |
| **B24** | `feature = "bench"` exists only to punch a hole in crate privacy for `polint-bench`. | `crates/polint/Cargo.toml:12-14`; `lib.rs:45-97` (`pub mod _bench` re-exporting `cache`, `config`, `core`, `fs`, `go`, `ts`, `keys`). Only **2** `cfg(feature = …)` sites exist in the entire crate. | 2 | P2 |

### b.1 The one genuinely good abstraction

`analysis/mir/op.rs:21` — `MirOperationKind` has exactly 9 variants (`StorageLive`, `Bind`, `Assign`,
`Read`, `Write`, `Branch`, `Call`, `Return`, `Unsupported`), zero language-specific variants, and
`MirValue` (`:70`) has 5. This is a correct, language-neutral IR. `lower_go.rs` and `lower_ts.rs`
are the only files that *should* import `crate::go` / `crate::ts`, and they do (`lower_ts.rs:31`).

**The failure is not the IR. The failure is that MIR is not the *sole* contract** — 15 other files
under `analysis/` reach around it into the frontends (B7).

### b.2 The second genuinely good design that is not wired up

`go/adapter.rs:78` and `ts/adapter.rs:110` have **byte-identical signatures**:

```rust
pub(crate) fn analyze_with_plan_options_and_cache_stats(
    db: &mut AnalysisDb, cache: &crate::cache::Cache, config_hash: &str,
    rule_hash: &str, plan: &AnalysisPlan, parallel: bool,
) -> ProviderAnalysisResult
```

They differ only in the file filter (`go/adapter.rs:89` `file.language == Language::Go` vs
`ts/adapter.rs:121` `file.language.is_ts_family()`). A `trait LanguageFrontend` already exists
structurally — it has simply never been written down. This is the single cheapest high-value
refactor available.

---

## (c) What breaks at 10x scale

### c.1 Compile and test throughput — measured on this machine

| Command | Time | CPU |
|---|---|---|
| `cargo check -p polint --all-targets` (warm deps, cold crate) | **66 s** | 49% |
| `cargo check -p polint --all-targets` after `touch core/mod.rs` | 9.6 s | 76% |
| `cargo build -p polint --lib --tests` | **54 s** | 360% |

The 49% CPU on `check` is the tell: **there is one crate, so there is one type-checking unit.**
Rust parallelizes across crates, not within them. Today's 267k LOC already costs a minute per
full check. At 10x (2.7M LOC) with the same shape this is ~10 minutes per check, single-threaded,
with no incremental crate-level caching possible — every contributor pays it, every CI job pays it.

CI already shows the ceiling. `.github/workflows/ci.yml:96-102` splits tests into exactly **two**
buckets — `--lib` and `--test '*'` — because that is the maximum granularity Cargo offers for one
crate. `--lib` alone runs **2,429 tests in one binary**. There is no third split available without
a crate split. The recent history (`60db1e20 perf(ci): split test suite into parallel jobs`,
`1263208a Parallelize native eval fixture coverage`) is the team already grinding against this wall.

### c.2 Adding language #3 (say Python or Java)

Every one of these is a mandatory edit, not an addition:

| Site | What must change |
|---|---|
| `core/mod.rs:184` | `enum Language` variant |
| `core/mod.rs:194` | `from_path` extension table |
| `core/mod.rs:209` | `is_ts_family()` — the whole concept breaks; 49 call sites need re-triage |
| `core/mod.rs:658` | ~9 new `py_*` fields on the 132-field `AnalysisDb` |
| `core/mod.rs:7178` | new `Capabilities` bools + `:7355` array |
| `analysis_plan.rs:687-688` | capability-name string match arms |
| `analysis_kernel/provider.rs:57` | `LanguageScope` variant |
| `analysis_kernel/provider.rs:255-884` | new `ProviderManifest` literals at the right index |
| `analysis_kernel/provider.rs:936,967` | **both** duplicated order assertions |
| `analysis_kernel/mod.rs:92-968` | new stage in `run()` + `Language` match at `:149-160` |
| `analysis_kernel/incremental/keys.rs:40` | `LayerKind::PySyntax` |
| `analysis_kernel/incremental/keys.rs:203-210` | the `debug_assert!` that hardcodes "Go or TS only" |
| `analysis_kernel/incremental/keys.rs:25-36` | build-topology input filenames (`pyproject.toml`, …) |
| `analysis_kernel/incremental/input_snapshot.rs:127,339,737` | per-language lifecycle snapshot |
| `analysis_kernel/validation.rs` | per-language fact validation (39 `Language::` refs today) |
| `analysis_kernel/metadata.rs:6` | `FactFamily` variants |
| `analysis/mir/lower_py.rs` | new (analogue of 3,952-line `lower_ts.rs`) |
| `analysis/cfg/lower_py.rs`, `analysis/types/py.rs`, `analysis/refined_calls/py.rs`, `analysis/entrypoints/recognizers_py.rs`, `analysis/solver/py_*/` | 5+ new modules inside the "neutral" layer |
| `sdk/facts.rs` | 26 `Language::` refs; new per-language fact views |

That is **~19 mandatory edit sites across 3 modules that are supposed to be language-agnostic**,
before writing a single line of Python analysis. This is the expression problem, unmitigated. And
because `Language` appears 999 times in 129 files, the compiler will only catch the subset that are
exhaustive matches (24 files) — the rest are `if lang == Go` / `is_ts_family()` conditionals that
silently do the wrong thing for the new language.

### c.3 Adding analysis #24

Per the kernel's own structure, a new provider requires editing **at minimum 8 places** (schema
const, manifest literal, two order assertions, `run()` body, trigger-capability consts,
`FactFamily`, a bespoke `cache_key.rs`, plus `validation.rs`). The `~30-line
`_dependency_output_digest` unwrap boilerplate in `run()` is repeated verbatim **20 times** today
(e.g. `analysis_kernel/mod.rs:833-844`).

Worse: 16 of 17 analyses have already opted out of the layer-cache system (B14), so a 24th analysis
has no persistence story unless it hand-rolls one. **The incremental substrate — the thing that makes
a 2.7M-LOC-repo analyzer viable at all — is being bypassed by every new analysis.**

### c.4 Agent/LLM-authored rules

This is the goal that the current boundary hurts most, and it is the one place where the team has
already built real machinery (`tests/public_surface_leak.rs`, 7 gate tests, a 115-name hand-maintained
allowlist, marker scanning over live JSON output and generated `SKILL.md`). But:

- The contract is **115 names, 70 of them verbatim internal structs with public fields, zero
  `#[non_exhaustive]`, at version 0.1.17.** Any field added to `FunctionFact` breaks every generated
  rule. An LLM writing rules against a surface with no semver and no stability annotations will
  produce rules that rot silently.
- Three fact views are **vocabulary stubs with zero methods** — `Cfg` (`sdk/facts.rs:837`),
  `CallGraph` (`:843`), `TestSuiteMetrics` (`:945`). An agent reading the SDK surface will confidently
  request `CallGraph` and get a capability diagnostic. The type system advertises capability that
  doesn't exist.
- `policy_queries.rs` — 3,497 lines, 93 impl functions, 5 `pub(crate)` entry points, 8 imports into
  `crate::analysis::*` — is a well-placed *firewall*, but it means all five query families
  (`matching_events`, `forbidden_reachable`, `missing_guards`, `missing_cleanup`, `forbidden_flows`)
  share one file and one review surface. Adding query family #6 means growing a 3.5k-line god-file.
- There is **no rule registry** — `runner::run_cli(Vec<Rule>)` requires a hand-written `main`.
  Fine for 10 rules; a bottleneck for 10,000 agent-authored ones.

### c.5 The cycles are the real blocker

You cannot split a crate along a cycle. Today **26 of 27 top-level modules** are in a cycle. That
means *no* crate split is currently possible without first breaking cycles — and the largest ones
(`core`↔`analysis` at 318/98, `analysis`↔`analysis_kernel` at 195/182) are not accidental drift;
they are how the system is designed to work: `AnalysisDb` (in `core`) owns concrete storage for every
analysis's facts, so `core` must know every analysis's types.

---

## (d) Recommended target layering

### d.1 Target crate graph (acyclic, by construction)

```
                       polint            (facade: re-exports sdk + runner; the ONLY published crate)
                         │
        ┌────────────────┼─────────────────┐
        ▼                ▼                 ▼
   polint-sdk       polint-runner     polint-cli
        │                │                 │
        └────────┬───────┴─────────────────┘
                 ▼
          polint-host   ← COMPOSITION ROOT: the only crate that names concrete
                 │        frontends and concrete analyses. Builds the registries.
     ┌───────────┼─────────────┬──────────────────┬────────────────┐
     ▼           ▼             ▼                  ▼                ▼
polint-go   polint-ts    polint-py …    polint-analysis-*     polint-kernel
     │           │             │        (callgraph, dataflow,  (scheduler,
     └─────┬─────┴─────────────┘         types, cfg, solver)    incremental,
           ▼                                    │                store)
   polint-frontend-api                          │                   │
   (trait LanguageFrontend,                     ▼                   ▼
    FrontendOutput)                    polint-analysis-api ◀────────┘
           │                          (trait Provider, ProviderManifest,
           └──────────┬────────────────  FactStore, CapabilityId)
                      ▼
                 polint-ir
              (MIR, Place, CFG shape)
                      │
                      ▼
                 polint-core
      (FileId/Span/StableKey, LanguageId registry,
       Diagnostic, FactFamily-as-open-id)
                      │
                      ▼
       polint-vfs   (fs, repo_fs, path_context — no fact types)
```

Rules the graph enforces:

1. **`polint-kernel` never names a concrete analysis or language.** It depends only on
   `polint-analysis-api`. This inverts B6 and B1 — the compiler makes the cycle impossible.
2. **`polint-core` never names a concrete fact.** `AnalysisDb` becomes a `FactStore` trait +
   per-provider store types owned by their providers. This kills B5 and B10.
3. **`polint-analysis-*` never names a frontend.** Only `lower_*` crates
   (`polint-lower-go`, `polint-lower-ts`) sit between a frontend and MIR. This kills B7/B8.
4. **`polint-host` is the composition root** — the one place a `Vec<Box<dyn LanguageFrontend>>` and
   a `Vec<Box<dyn Provider>>` are assembled. Adding a language = adding one crate + one line here.

### d.2 The four abstractions that must be written down

**1. `trait LanguageFrontend`** (`polint-frontend-api`) — already exists structurally (§b.2):

```rust
pub trait LanguageFrontend: Send + Sync {
    fn id(&self) -> LanguageId;
    fn handles(&self, path: &Path) -> bool;          // replaces Language::from_path
    fn provider_id(&self) -> &'static str;
    fn analyze(&self, ctx: &FrontendCtx<'_>, files: &[&SourceFile]) -> FrontendOutput;
}
```

Replaces the closed `Language` enum (B12) with an open `LanguageId` (interned string or `u16` from a
registry). `is_ts_family()` becomes a frontend-declared family tag, not a `core` method.

**2. `trait Provider`** (`polint-analysis-api`) — attaches behaviour to the manifest that already
exists:

```rust
pub trait Provider: Send + Sync {
    fn manifest(&self) -> &ProviderManifest;         // reuse provider.rs:2-11 verbatim
    fn run(&self, ctx: &mut ProviderCtx<'_>) -> ProviderOutput;
}
```

The scheduler then **topologically sorts `manifest().inputs` against `manifest().outputs`** — the
data is already declared at `provider.rs:255-884`, it just needs to be used. This deletes the
877-line `run()` (B1), both duplicated order assertions (B2), the string lookup panic (B3), and the
20× repeated digest boilerplate.

**3. Open `CapabilityId`** replacing the 24-bool `Capabilities` struct (B11). Capabilities become
`&'static str` (or interned) values *contributed by providers*, not enumerated in `core`. The
capability→provider resolution that `analysis_plan.rs` does with hardcoded string matches becomes a
lookup over the registry.

**4. `trait FactStore`** replacing the 132-field `AnalysisDb` (B10). Each provider owns its store
(`CallStore`, `DataFlowStore`, `EvidenceStore` — **13 of these already exist**); `AnalysisDb` becomes
a keyed container. `sdk/facts.rs` views already have a private `db` field, so the SDK surface does
not change.

---

## (e) Migration sequence — no rewrite required

Each step is independently shippable, independently valuable, and testable with the existing suite.
Steps 1–5 happen **inside the current single crate** — no crate split until the cycles are gone.

### Step 0 — Make the invariant enforceable (1 day)

Dogfood. Write a repo-local polint rule (`.polint/rules/`) using `module_graph` +
`resolved_imports` capabilities that asserts the target layer order and fails CI on new
wrong-direction edges. Seed it with the 26 cycles as a baseline (`src/baseline.rs` already exists for
exactly this). **Without this, every later step regresses.** This is also the strongest possible
marketing artifact: polint enforcing polint's own architecture.

### Step 1 — Split `core/mod.rs` into files (2–3 days, mechanical, zero behaviour change)

11,143 lines → `core/{ids,lang,span,facts/*,db,rule,capability,changeset}.rs`. No API change, no
crate change. This alone makes the next four steps reviewable. Do it first because every subsequent
step touches `core`.

### Step 2 — Evict `eval/` to a dev-only crate (1–2 days)

29,344 LOC, `expect(dead_code)` in release, only 3 non-test references (all `#[cfg(test)]`). Move to
`crates/polint-eval` as a `dev-dependency`. **Immediate ~11% cut to every `cargo check`.** Also
removes the `analysis`↔`eval` cycle (181 refs) for free. Lowest-risk, highest-immediate-payoff step
in the list.

### Step 3 — `trait LanguageFrontend`, in-crate (1 week)

The signatures are already identical (§b.2). Define the trait in `analysis_kernel`, implement for
`go` and `ts`, and change `AnalysisKernel::run:191/209` to iterate `&[&dyn LanguageFrontend]`.
Introduce `LanguageId` alongside `Language` (don't delete the enum yet — add
`Language::id() -> LanguageId` and migrate call sites opportunistically). **Acceptance test: adding a
stub third frontend requires editing exactly one list.**

### Step 4 — `trait Provider` + topological scheduler (2–3 weeks) — *the keystone*

1. Add `trait Provider` with `manifest()` + `run()`.
2. Wrap each of the 23 existing `derive_*_with_cache_stats` free functions in a unit struct impl —
   pure mechanical, no logic moves.
3. Replace `AnalysisKernel::run`'s 877 lines with: build registry → topo-sort on
   `manifest().inputs`/`outputs` → filter by requested capabilities → run.
4. Delete both duplicated order assertions (`provider.rs:936`, `:967`); replace with *one* test that
   asserts the topo-sort is deterministic and matches the historical order.
5. Fold the three identical `*_TRIGGER_CAPABILITIES` consts (`mod.rs:43,47,55`) into manifest-declared
   capability requirements.

Determinism is protected by the existing `eval::determinism_gate` (`ci.yml:186`) and the
`provider_order_for_test` snapshot — run both against the sorted order to prove byte-identical output.

**This is the step that unlocks everything else.** After it, adding an analysis is +1 crate +1
registry line, and the `analysis_kernel → analysis` edge (182 refs) drops to zero because the kernel
only sees `dyn Provider`.

### Step 5 — Break `core` ↔ `analysis` (3–4 weeks)

The 98 `use crate::analysis::…` lines at the top of `core/mod.rs` exist because `AnalysisDb` stores
every analysis's facts. Fix in two moves:

- **5a.** Move each provider's `AnalysisDb` fields into the `*Store` type that provider already owns
  (13 exist). `AnalysisDb` keeps only the shared spine: files, functions, imports, symbols, spans.
  Target: 132 fields → ~30.
- **5b.** Move the 5 `Ts*Id` types from `analysis/ids.rs:34-46` into `ts/ids.rs` (B9).

Also in this step: move `analysis/mir/lower_{go,ts}.rs`, `cfg/lower_*`, `types/{go,ts_js}.rs`,
`refined_calls/{go,ts_js}.rs`, `entrypoints/recognizers_*`, `solver/{go_rta,ts_tokens,ts_object_model}`,
and `calls/ts_value_flows.rs` **out of `analysis/` into `go/` and `ts/`** (27,425 LOC, B8). They keep
importing MIR; MIR stops importing them. This is a directory move plus import fixups, not a rewrite.

### Step 6 — Open `CapabilityId` (1–2 weeks)

Replace the 24-bool struct with provider-contributed capability IDs. Keep the builder API
(`Capabilities::new().calls()`) as sugar over the registry so no rule breaks. Retire
`ts_components`/`ts_classes`/`jsx_attributes`/`go_tests` from `core` into their owning frontends.

### Step 7 — Crate split (2–3 weeks, now mostly mechanical)

Only now, with the cycles gone, do the split — in dependency order, bottom-up:

`polint-vfs` → `polint-core` → `polint-ir` → `polint-frontend-api` + `polint-analysis-api` →
`polint-go`/`polint-ts` → `polint-analysis-*` → `polint-kernel` → `polint-host` → `polint-sdk` /
`polint-runner` / `polint-cli` → `polint` facade.

`crates/polint/Cargo.toml`'s `feature = "bench"` hole (B24) disappears: `polint-bench` just depends
on `polint-kernel`. The `public_surface_leak` probe simplifies to "the facade depends only on
`polint-sdk` + `polint-runner`" — a Cargo-enforced invariant instead of a 115-name allowlist.

### Step 8 — SDK stability (parallel with 6–7)

- `#[non_exhaustive]` on every prelude-exported fact struct (currently 0 in `sdk/`).
- Delete or implement the three zero-method vocabulary stubs (`Cfg`, `CallGraph`, `TestSuiteMetrics` —
  `sdk/facts.rs:837,843,945`). Advertising unimplemented capability in the type system is worse than
  omitting it, especially for LLM authors.
- Split `policy_queries.rs` (3,497 lines) into one module per query family behind the same
  5-function neck.
- Add a rule registry (`inventory`/`linkme`) so agent-generated rules don't require editing `main`.

### Sequencing rationale

| Step | Unblocks | Risk | Cost |
|---|---|---|---|
| 0 Layering gate | everything (prevents regression) | none | 1 d |
| 1 Split `core/mod.rs` | reviewability of 3–7 | none | 3 d |
| 2 Evict `eval/` | −11% compile, kills 1 cycle | none | 2 d |
| 3 `LanguageFrontend` | language #3 | low | 1 w |
| **4 `Provider` + topo-sort** | **analysis #24; kernel↔analysis cycle** | **medium** | **3 w** |
| 5 Break `core`↔`analysis` | the crate split | high | 4 w |
| 6 Open capabilities | agent-authored capability discovery | medium | 2 w |
| 7 Crate split | parallel compile, real boundaries | low (after 5) | 3 w |
| 8 SDK stability | agent rule authoring at scale | low | 2 w |

**Do not attempt 7 before 5.** Do not attempt 5 before 4. Steps 0, 1, 2 can start today and are
strictly positive regardless of what the rest of the plan becomes.

---

## Closing assessment

The team has built the hard parts correctly: a language-neutral MIR with 9 op kinds, stable keys,
digest-based incrementality, capability-driven planning, precision ceilings, an evidence/provenance
model, and a public-surface leak gate that scans live CLI output and generated agent skills. That is
a genuinely sophisticated foundation — most static analyzers never get a neutral IR at all.

What is missing is **one layer of indirection in three places**: a frontend trait, a provider trait,
and an open capability ID. All three have their data models already written (`ProviderManifest`
carries a full dependency DAG at `provider.rs:255-884` that nothing sorts; `go`/`ts` adapters have
byte-identical signatures). The abstractions are not missing conceptually — they are missing
*syntactically*.

Consequently the current architecture is not 10x-ready, but it is **10x-reachable without a rewrite**.
The single highest-leverage change is Step 4: turning 877 hardcoded lines into a topological sort over
manifests that already exist. Everything else follows from it.
