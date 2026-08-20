# 09 — Declared Direction, Research Adoption, and Documentation Gaps

**Scope:** documentation archaeology. What has already been decided, researched, and planned — so that
recommendations build on it instead of duplicating or contradicting it.
**Sources:** `research/` (18 areas), `.planning/` (ROADMAP, MILESTONES, PROJECT, STATE, RETROSPECTIVE,
REQUIREMENTS, forensics), `docs/` (roadmap + audit trees), `README.md`, `action.yml`, and the code.
**Date:** 2026-07-29. **Code base:** `1263208a` (branch `static-analysis-architecture-review`).

**Verdict in one line.** The written direction is *unusually good* — falsifiable outcome gates, recorded
decisions, honest forensics — and the *research is essentially all adopted at the fact-model layer and
essentially all unshipped at the product layer*. The engine has 23 wired providers and 112k LOC of
analysis behind a `#[expect(dead_code)]` blanket, a public surface of 4 semantic capabilities, one
SQLite table, and a 29k-LOC evaluation harness that no command can run. The single largest
documentation problem is not absence — it is that **five doc trees describe five different versions of
the same system and nobody has declared which one is true**.

---

## (a) Research adoption table

18 research areas exist (not 22). `research/ROADMAP.md` covers 16 of them; `static-analysis-2.0` and
`local-semantic-store` were added 2026-07-07 and are *not* in that roadmap — the first structural
inconsistency in the tree.

**Reading the "Built?" column.** `crates/polint/src/analysis/mod.rs:1-6` puts a crate-wide
`#![cfg_attr(not(test), expect(dead_code))]` over the entire 112,545-LOC analysis tree, so the compiler
cannot tell you what is live. The judgment below is: **Yes** = reachable from
`AnalysisKernel::run` (`crates/polint/src/analysis_kernel/mod.rs:92`) *and* observable by a rule author
through `polint::sdk`; **Partial** = wired into `run()` but with no public surface, or built at a
different path than the standard specifies; **No** = orphaned or absent.

| # | Area | Recommendation (one line) | Built? | Evidence |
|---|---|---|---|---|
| 1 | **abstract-interpretation** | Reduced-product kernel of many small lattices over MIR/CFG; 9 SDK views (`Nilness`, `Constants`, …) | **Partial** | Kernel built and wired: `analysis/domains/lattice.rs` (`AbstractDomain`), `analysis_kernel/mod.rs:476,491`. Zero of the 9 SDK views exist in `sdk/facts.rs`. |
| 2 | **agent-extension-surface** | `#[polint::extension]` repo-local Rust crates as process-isolated executables emitting typed facts through sinks; CLI `polint extension new\|test\|diff` | **Partial (host only)** | Host + sinks + discovery built and wired: `analysis/extensions/{host,sinks,discovery}.rs`, `discovery.rs:12` (`.polint/extensions`), `analysis_kernel/mod.rs:669`. **No `Command::Extension` in `cli/mod.rs:161-190`**, no README mention, no template. The surface exists but no user can reach it. |
| 3 | **agent-rule-authoring** | Typed Rust `#[polint::rule]` over typed fact views as *the* authoring surface + fixture-first `polint test` | **Yes** | The one fully-shipped area. `crates/polint-macros/`, `sdk/facts.rs` (27 views), `cli/mod.rs:161-190` (`NewRule`, `Test`, `Inspect`, `Facts`, `Explain`, `AddSkill`), `docs/facts/` (18 pages), `docs/schemas/` (9 JSON schemas). |
| 4 | **analysis-kernel** | Private hybrid provider DAG + provenance/validation/merge gates + layer-split cache keys; internal relation/fixpoint sub-engine | **Yes (DAG) / No (demand)** | 23 provider manifests at `analysis_kernel/provider.rs:257-875`, driven by `analysis_kernel/mod.rs:92`. But `run()` is a hardcoded straight line, not demand-driven — see row 12. |
| 5 | **call-graphs** | Layered `analysis::calls` facts with algorithm/confidence/status/unresolved-reason on **every** edge; retire `FunctionFact.calls: Vec<String>` | **Partial** | Built + wired: `analysis/calls/`, `analysis/refined_calls/`, `analysis_kernel/mod.rs:382,834`. `Calls<'_>` is public and Supported (`analysis_plan.rs:706`). **The legacy string call list was never retired — `core/mod.rs:277` still declares `pub calls: Vec<String>`**, contradicting implementation-bootstrap D3 with no recorded decision. `CallGraph<'_>` declared at `sdk/facts.rs:845` but `Unsupported` at `analysis_plan.rs:724`. |
| 6 | **cfg-control-flow** | Native CFG facts (op nodes *and* basic blocks), typed abrupt/exceptional/cleanup edges, derived dominance/control-dependence | **Partial** | Built + wired: `analysis/cfg/{builder,derived,graph,lower_go,lower_ts}.rs`, `analysis_kernel/mod.rs:355`. **Path diverges from STANDARD**, which specified `crates/polint/src/cfg/` — that directory does not exist. `ControlFlow<'_>` is Supported; the specified `Cfg<'_>` view (`sdk/facts.rs:839`) is `Unsupported` (`analysis_plan.rs:724`) — consistent with cfg decision D10, so this one is a *deliberate* deferral. |
| 7 | **data-flow** | General value-flow family, tiered local-sparse → direct-call → summary-projected → model → query-scoped paths; taint is a layer on top | **Partial (tiers 0-3 of 6)** | Built + wired: `analysis/data_flow/{local,direct_calls,summary_edges,query}.rs`, `analysis_kernel/mod.rs:866`. `DataFlow<'_>` public and Supported (`analysis_plan.rs:718`). No `algorithms/ifds` or `algorithms/ide` — tiers 5-6 unbuilt, as the ladder allows. |
| 8 | **effects-summaries** | Summary *kernel* + **typed, versioned summary domains** (9 named: `ControlEffects`, `DataFlowTito`, `TaintEffects`, …), never one generic format | **Partial** | Kernel + SCC closure built and wired: `analysis/summaries/{core,domain,scc,closure}.rs`, `analysis_kernel/mod.rs:531,555`. The multi-domain split is not realised as 9 typed domains, and `Effects<'_>` does not exist in `sdk/facts.rs`. |
| 9 | **evaluation-harness** | External benchmarks are primary evidence + narrow native fixtures; ship `polint eval` hidden/`pub(crate)` | **Partial (test-only)** | 29,344 LOC at `crates/polint/src/eval/` with adapters `eval/external/{jelly_callgraph,go_x_tools_callgraph,secbench_js,gosec}.rs` and gates `eval/gates.rs`. **`crate::eval` has zero references outside `eval/` — no CLI, no `main.rs` path; it runs only under `cargo test`.** The `real_app_callgraph` adapter named in static-analysis-2.0 does not exist. |
| 10 | **framework-entrypoints** | Native entrypoint/lifecycle/trust-boundary facts *before* CG/DF; SDK `Entrypoints<'_>`; the first serious kernel consumer | **Partial** | Built + wired: `analysis/entrypoints/`, `analysis_kernel/mod.rs:597`. **Path diverges** — the STANDARD specified `crates/polint/src/framework/`, which does not exist. `Entrypoints<'_>` (the stated first vertical slice, kernel D10) is absent from `sdk/facts.rs`. |
| 11 | **implementation-bootstrap** | Private `pub(crate) mod analysis` bootstrap; do **not** grow `core::AnalysisDb`; do **not** upgrade `FunctionFact.calls`; no public views in slice 1 | **Partial — one rule violated** | `lib.rs:18` declares `pub(crate) mod analysis`. But `RUST-ARCHITECTURE.md:26-33` says "Avoid A Mega `core` Module … The semantic engine should not be implemented there" — `crates/polint/src/core/mod.rs` is **11,143 LOC** and holds the 132-field `AnalysisDb`. No recorded decision reverses this. |
| 12 | **incremental-query-engine** | Native layered incrementality: `InputSnapshot` → layer cache → `DependencyIndex` → `InvalidationPlan` → demand queries. Explicitly *not* Salsa-first | **Partial — top half only** | Vocabulary built at `analysis_kernel/incremental/{input_snapshot,keys,change_set,dependency_index,invalidation,layer_cache,demand}.rs` plus an extension-isolation module (**path diverges** from the specified `analysis/incremental/`). The demand-query layer — `analysis/demand/` (2,188 LOC: `query.rs`, `context.rs`, `scc.rs`, an extension-isolation module, `trace.rs`) — is **referenced nowhere**; its only mentions are five string literals in `analysis_kernel/provider.rs:1816-1821`. |
| 13 | **local-semantic-store** | SQLite via `rusqlite` bundled; ~30 tables; 12 store modules; `polint graph {used-by,callers,path,taint}` | **No** | `analysis_kernel/store/` contains `mod.rs`, `connection.rs`, `migrations.rs`, `tests.rs`. 9 of the 12 specified modules (`schema`, `ids`, `ingest`, `query`, `graph`, `payloads`, `search_manifest`, `generation`, `summaries`) do not exist. `migrations.rs:8` = `CURRENT_SCHEMA_VERSION: i32 = 1`; `migrations.rs:13` creates exactly one table, `_polint_schema_migrations` — the migration bookkeeping table itself. **Zero facts persist.** No `polint graph` command. First delivery attempt abandoned (§d). |
| 14 | **module-graph** | Many typed topology fact layers with ecosystem-specific providers (`go`, `js`, `python`, `jvm`, `rust_cargo`, `monorepo`) + 16 manifest/lockfile format parsers | **Partial (2 of 6 ecosystems)** | Built + wired: `module_graph/{go.rs,ts.rs,topology.rs,formats/}`, `analysis_kernel/mod.rs:227`. `resolved_imports` and `module_graph` are Supported (`analysis_plan.rs:692`). No `python`, `jvm`, `rust_cargo`, or `monorepo` provider. |
| 15 | **program-slicing-evidence** | Evidence + slice **query layer** over existing facts, attached to diagnostics; thin slices, ranked path explanations | **Split: Partial / No** | Evidence built + wired (`analysis/evidence/`, `analysis_kernel/mod.rs:896`) but `docs/facts/README.md` classes it **internal** — it is stripped before users see it. **`analysis/slicing/` (2,027 LOC: `local.rs`, `interprocedural.rs`, `paths.rs`) has *zero* references anywhere in the crate outside itself.** Fully orphaned. |
| 16 | **semantic-index** | Language-owned typed fact providers emitting normalized scope/symbol/reference/import/alias facts; not a generic AST resolver, not an LSP wrapper | **Yes** | Built + wired at `symbol_graph/{go,ts,semantic,model}.rs` — **path diverges** from the specified `crates/polint/src/semantic/`, which does not exist. `Symbols<'_>` (`sdk/facts.rs:487`) and `References<'_>` (`:559`) are public and Supported (`analysis_plan.rs:693`). The resolution-status vocabulary shipped (`polint unknowns --cap references`). |
| 17 | **static-analysis-2.0** | "Tiered resolution + compositional summaries + a queryable local semantic store, with verified ML at the edges" (README:84) | **Partial** | Tiering + summaries built (rows 5,7,8). Store: **No** (row 13). Type-directed callgraph tier — named "the largest real-world F1 lever" — **deliberately excluded from v2.0** (`.planning/REQUIREMENTS.md:175`). ML at the edges: not started. |
| 18 | **type-alias-points-to** | Layered types → values → places → summaries → points-to, with **alias as a query service**, Andersen first, no mandatory whole-repo pass | **Partial** | `analysis/types/` wired as `polint.type_value_alias` (`analysis_kernel/mod.rs:695`); a real Andersen heap exists at `analysis/calls/js_points_to/`. But the specified `analysis/alias/` query service does not exist; `analysis/aliases/` is reachable only from `eval/observed.rs`; `analysis/points_to/` is reachable only from `analysis_kernel/validation.rs` and `eval/`. None of `Types<'_>`, `Values<'_>`, `Aliases<'_>` exist in `sdk/facts.rs`. |

### Built-but-unwired inventory (the `expect(dead_code)` shadow)

| Module | LOC | Reachability |
|---|---|---|
| `crates/polint/src/analysis/slicing/` | 2,027 | **Zero references** outside itself. Pure orphan. |
| `crates/polint/src/analysis/demand/` | 2,188 | **Zero code references**; five string literals in `provider.rs:1816-1821`. |
| `crates/polint/src/analysis/aliases/` | — | `eval/observed.rs` only. |
| `crates/polint/src/analysis/points_to/` | 1,306 | `analysis_kernel/validation.rs` + `eval/` + `cli/mod.rs:4236` (status rendering) only. |
| `crates/polint/src/analysis/values/`, `access_paths/` | — | `eval/observed.rs` + `core/mod.rs` status plumbing only. |
| `crates/polint/src/eval/` | 29,344 | Test-only; no command, no binary path. |

### Public surface vs. built capability

`analysis_plan.rs:684-742` is the honest ledger. **Supported:** `syntax`, `imports`, `resolved_imports`,
`module_graph`, `symbols`, `references`, `events`, `calls`, `control_flow`, `dataflow`, plus the syntactic
metric/literal families. **Unsupported (declared-but-reserved):** `cfg`, `call_graph`, `coverage_facts`,
`test_suite_metrics` — all four have `pub struct`s in `sdk/facts.rs` (`:839`, `:845`, `:928`, `:946`).
Languages: `core/mod.rs:184-191` = Go, TypeScript, Tsx, JavaScript, Jsx. **No Python. No Java.**

---

## (b) Declared trajectory and current milestone gates

### The declared arc

- **`research/ROADMAP.md`** (2026-05-era): 16 research tracks, all `[x]` complete; 22 implementation PRs,
  **all `[ ]` unchecked**. Five phase groups: Foundation → Semantic Backbone → Interprocedural Substrate →
  Precision → Promotion. PR 22 = public SDK views `Calls`, `CallGraph`, `DataFlow`, `Effects`, `Evidence`.
  *This roadmap was never checked off even though ~15 of its 22 PRs shipped in some form.*
- **`docs/roadmap/00_ROADMAP.md`**: 10 capability entries; **only item 1 is checked**. Items 2-7 have
  demonstrably shipped. Last touched 2026-05-20; repo is at phase 65. **The most stale doc in the tree.**
- **`.planning/ROADMAP.md`**: the live one. Milestone **v2.0 "Static Analysis 2.0 Implementation"**,
  phases 63-71, 2 of 9 complete (22%), stalled since 2026-07-20.

### Current milestone: v2.0, goal (`.planning/PROJECT.md:66`, verbatim)

> **Goal:** Build the Static Analysis 2.0 implementation foundation that turns polint's existing private
> analysis engine into a durable, queryable local semantic layer for custom Rust rules, agentic review,
> and future local graph exploration.

### The four falsifiable outcome gates

Re-anchored 2026-07-08/09 (`.planning/REQUIREMENTS.md:229`), committed in `ba5813b1`. Framing at
`REQUIREMENTS.md:26`: *"It must measurably move scale and latency, not only land infrastructure. Every
roadmap phase must name which outcome gate it advances; a phase that advances none is plumbing."*

1. **Scale gate** (`REQUIREMENTS.md:28`) — *"Peak RSS on the large-monorepo benchmark stays proportional
   to the analyzed working set. Store ingest must not resurrect the eager whole-repo pipeline … that
   previously caused 30GB+ OOM (… current baseline ~1GB peak on the reference monorepo). Initial
   regression budget until warm reuse lands: **at most +20% peak RSS and +25% cold wall-clock** versus
   the store-disabled baseline; budgets are revisable only with a recorded decision."*
2. **Latency gate** (`:29`) — *"warm `polint review` on a small diff re-analyzes only the invalidation
   frontier (changed functions/SCCs plus transitive summary dependents), with the recompute set
   instrumented and a p50/p95 warm-latency target set from the Phase 0 baseline and then enforced."*
3. **Honesty gate** (`:30`) — *"Unknown, partial, setup-missing, unsupported, and budget-exceeded states
   remain visible end to end … now durably persisted, never collapsed."*
4. **Accuracy visibility gate** (`:31`) — *"v2.0 does not have to raise callgraph F1 … but it must measure
   and surface recall/precision of the persisted graph on real-repo benchmarks so `polint graph` answers
   are never overtrusted."*

Supporting requirement families: **BENCH-01..04** (`:65-68`, all `[x]` via Phase 63), **PERF-01..04**
(`:72-75`, only PERF-03 done), **REV-01..03** "Warm Review Payoff" (`:112-114`, all pending —
*"the practical payoff of this milestone … a first-class deliverable, not an emergent property"*).

**Locked decisions** (`:45-49`, *"Changing any of these requires a recorded decision, not a silent edit"*):
benchmark suite pinned to `grafana/grafana` (primary polyglot scale target), `gohugoio/hugo`,
`excalidraw/excalidraw`, Jelly + Go x/tools oracles, and a private monorepo (~1GB peak, cold 7.4s /
warm 4.6s). **Phase 70 (Tantivy lexical search) is the designated scope-cut.**

### Phase ladder

63 Ground Truth & Perf Baseline ✅ · 64 Store Foundation & Boundary Proof ✅ · **65 Generation Manifest —
attempted, abandoned** · 66 Validated Fact/Graph Ingest · **67 Summary Persistence + Invalidation Frontier
+ Warm Review (keystone)** · 68 Internal Query Engine · 69 Public `polint graph` · 70 Lexical Search (cut
candidate) · 71 Recovery/Pruning/Scale Gates.

### Explicitly deferred, by decision

Remote package-summary registry (REG-FUT-01..03); stable vector search (SEARCH-FUT); **type-directed
callgraph tier** (ACC-FUT — `REQUIREMENTS.md:175`: *"the largest real-world F1 lever. v2.0 deliberately
excludes it … but it is the default headline for the milestone after v2.0"*); public graph JSON schema;
MCP/LSP integration; SDK graph fact views. Out-of-scope table at `:192-207` (11 rows) bans a public
query language, raw graph SDK views, `polint graph` as a CI gate, a graph DB by default, and any daemon.

---

## (c) Contradictions and undocumented divergences

### C1. Path divergences from a written STANDARD, with no recorded decision

| Standard says | Code has | Recorded? |
|---|---|---|
| `crates/polint/src/cfg/` | `crates/polint/src/analysis/cfg/` | No |
| `crates/polint/src/semantic/` | `crates/polint/src/symbol_graph/` | No |
| `crates/polint/src/framework/` | `crates/polint/src/analysis/entrypoints/` | No |
| `crates/polint/src/analysis/incremental/` | `crates/polint/src/analysis_kernel/incremental/` | No |
| `analysis/alias/` (query service) | absent; `analysis/aliases/` is eval-only | No |

Individually harmless. Collectively they mean **no research STANDARD can be used to navigate the code**,
which is precisely what a STANDARD is for.

### C2. Divergences of substance, with no recorded decision

- **`FunctionFact.calls: Vec<String>` was never retired.** call-graphs FINAL-REPORT names it legacy;
  implementation-bootstrap **D3** says do not upgrade it, add `CallSiteFact`/`DirectCallTargetFact`
  instead. Both happened — the typed facts were added *and* the string list survives at `core/mod.rs:277`.
- **"Avoid a mega `core` module"** (`research/implementation-bootstrap/implementation/RUST-ARCHITECTURE.md:26-33`)
  vs `core/mod.rs` at 11,143 LOC holding the 132-field `AnalysisDb`.
- **The demand-driven half of the kernel was silently dropped.** analysis-kernel D1 and the
  incremental-query-engine STANDARD both make demand queries load-bearing; `analysis/demand/` (2,188 LOC)
  is unreferenced and `AnalysisKernel::run` is a straight line of 23 passes.
- **Evidence was designed as "the user-facing product"** (`program-slicing-evidence/decisions/001-evidence-is-the-user-facing-product.md`)
  and shipped as an internal-only fact family.
- **`polint eval` was decided (evaluation-harness D5) as a hidden command.** It was built as a test module
  instead. The distinction matters: a hidden command can be run on a real repo; a test module cannot.

### C3. Documents that contradict each other

- **`docs/roadmap/00_ROADMAP.md` vs `docs/architecture-review/`** — same system, 10 weeks apart, disagree
  about what exists. The roadmap lists symbols/call graph/CFG/dataflow as unbuilt; they are wired.
- **`docs/ANALYSIS-ROADMAP.md`** (linked from `README.md`, i.e. **user-facing**) lists module resolution,
  symbols, call graph, CFG, dataflow, taint, and points-to as **"Planned"**. All but taint are built;
  four are publicly Supported. This is the only stale doc that users actually read.
- **`research/ROADMAP.md` PRs 1-22 all unchecked** while ~15 shipped.
- **`docs/RUST-AUDIT-IMPROVEMENT-PLAN.md:274-278`** puts the Go semantic sidecar out of scope; Phase 46
  shipped it 2026-06-02.
- **`.planning/ROADMAP.md:109` still tags Phase 65 "Research flags: none — mirrors existing kernel
  metadata patterns"** — the exact framing its own forensics report blames for the failure — and
  `.planning/STATE.md:47` still says "Ready for discussion". Neither was updated by `0f3741dc`.
  **The restart plan (R0-R6) exists only in `research/` and has never been reconciled into the roadmap.**
- **`.planning/RETROSPECTIVE.md` covers only v1.0 and v1.2.** There is no v1.3, v1.4, or v2.0
  retrospective and **no Phase 65 entry at all** — the largest failure in the project's history is absent
  from the document whose job is to record failures.

### C4. Documents vs code

- **`AGENTS.md:67`: "Architecture not yet mapped."** It sits inside a `GSD:architecture source:ARCHITECTURE.md`
  marker block whose source file does not exist anywhere in the repo.
- **CI enforces three gates, not the milestone's gates.** `.github/workflows/ci.yml:156-195` runs the
  polyglot canary, the public-surface leak gate, the determinism gate, and one semantic-store boundary
  measurement. **No accuracy gate and no scale/latency regression budget runs in CI**, despite
  BENCH-03 (`REQUIREMENTS.md:67`) declaring the regression gate a phase-boundary requirement from
  Phase 64 onward. The headline number (Jelly F1 89.06%) is not defended by anything that can fail.
- **Open repo-admin item T-42-04-10** (`.planning/STATE.md:54-56`): the leak gate is still not in branch
  protection — *"until then a PR can merge with the v1.3 leak gate failing."*

### C5. What is publicly promised (README + action.yml) vs what the code supports

Shipped commands (`cli/mod.rs:161-190`): `init`, `add-skill`, `new-rule`, `baseline`, `cache`, `inspect`,
`facts`, `unknowns`, `explain`, `test`, `check`, `review`, `ignores`. `action.yml` is a thin composite
wrapper (`version`, `args`, `cache`, `cache-key-prefix`, `working-directory`, `fail-on`).

**The README is honest.** It promises an SDK, parsers, typed facts, diagnostics, caching, CI output,
diff-gated review rules, and 10 rule *scaffolds* — explicitly *"These are scaffolds, not bundled rules"* —
and *"polint ships no built-in policy rules."* Everything it claims is real. Three flags, none severe:

1. **`docs/ANALYSIS-ROADMAP.md` is linked from the README** and materially understates the engine (C3).
2. **`polint init` creates `.polint/output/`** per README:87; `.polint/extensions/` is never mentioned,
   so the entire extension surface (row 2) is invisible to users.
3. **Language support is never stated.** The README shows Go and TS/JS examples and mentions Go module
   roots, but never says "Go and TypeScript/JavaScript only." A reader could reasonably infer broader
   support from *"multi-language, repo-local static-analysis rules"* (`lib.rs:1`).

`docs/roadmap/08_ENTRY_8_PYTHON_ADAPTER.md` (51 lines) and `09_ENTRY_9_JAVA_ADAPTER.md` (53 lines) commit
to **contract reuse, not implementation**: same SDK prelude, fact model, diagnostics, cache, capability
plan. Both are unscheduled (items 9 and 10 of 10, unchecked, no phase, no owner, no date), both gated
behind *"after Go and TS/JS prove the complete model"*, and both leave the parser choice explicitly open
(*"Prefer a Rust parser or tree-sitter"* / *"Choose between a self-contained parser path and a
javac/JavaParser semantic path per capability"*). Effort: Python M→XXL, Java L→XL. Neither is a design.

---

## (d) Recorded failures worth preserving

### Phase 65 — the scope-collapse forensic (the most valuable document in the repo)

Recorded in `0f3741dc` (2026-07-19, docs-only, +753/-17):
`.planning/forensics/report-20260719-phase-65-scope-collapse.md`,
`.planning/phases/65-generation-manifest-and-metadata-mirroring/65-LEARNINGS.md`,
`research/local-semantic-store/{RESTART-PLAN,REVIEW-FINDINGS-TRIAGE,IDENTITY-READINESS}.md`.

**What happened.** Phase 65 ("Generation Manifest and Metadata Mirroring", 4 requirements) produced a
**126-commit, 154-file, +85,404/-4,762 PR with a ~1-hour CI critical path**. At phase-complete it was
+36,456; the *review loop then added +50,329* — review added more code than the planned implementation.
`go/semantic/process.rs` alone gained 20,505 lines after completion. CI went 20 min → 60 min; the Windows
job hit its cap. The branch was **abandoned**, not merged, not rebased.

**Root cause** (`forensics:89-93`, verbatim):

> The research answered "what would a maximally defensive semantic store require?" but not "what is the
> smallest independently valuable change we can safely review and merge?" The plan converted every
> missing prerequisite into in-phase work, and the review loop converted every transitive defect into
> same-PR work. No mechanism forced a stop when the plan reached 19 parts, the implementation reached
> 119 files, review crossed into unplanned runtime subsystems, or CI exceeded the product's feedback-time
> target.

**The eight lessons** (`65-LEARNINGS.md:68-124`), condensed:

1. *"Existing types with the right names are not automatically canonical persistence contracts."*
   Reuse requires a readiness audit — 15 plans of prerequisite redesign preceded any DB work.
2. *"A plan with nineteen sequential parts is already multiple phases … Detailed planning did not make
   the phase safer; it concealed that the delivery unit was far too large."*
3. Open-ended "fix every missing critical" rules cause silent scope growth without a re-planning checkpoint.
4. **Review findings need disposition, not automatic implementation** — core / prerequisite / independent
   product bug / design-dependent are four different dispositions.
5. *"Persisted analysis identity needs deterministic tool inputs, but that does not automatically require
   sealing an entire local Go toolchain closure in the same PR."*
6. Performance tests must not govern ordinary correctness concurrency — global serialization to protect
   fixture budgets took CI to an hour.
7. *"A 'smoke' must be small."* The boundary smoke ran twelve full analyses and compiled a second profile.
8. **Verification must include delivery economics** — *"Correctness counts and artifact completeness are
   insufficient when a change is not human-reviewable or cannot provide timely CI feedback."*

**The sharpest surprise** (`:207-212`): *"Complete artifacts did not mean a healthy phase. All expected
GSD documents existed and verification passed."* Anomaly 8 puts it flatly: *"The failure was not skipped
GSD ceremony; it was that the ceremony had no size or scope brake."*

**The brake that came out of it** (`RESTART-PLAN.md:119-133`, *"stop-and-split triggers, not aspirational
metrics"*): ≤3 implementation tasks · ≤15 changed files · ≤2,500 handwritten added lines · ≤1 new durable
schema family per PR · ≤1 provider family · required CI ≤5 min · no required test >60s · no global
serialization of correctness tests · no new platform runtime subsystem unless it *is* the PR goal.

**Restart slices** (`RESTART-PLAN.md:49-117`): R0 identity-readiness audit (tests/docs only) → R1 minimal
generation state machine → R2 minimal run manifest → R3 provider-outcome correctness (no SQLite) →
R4 mirror one provider family → R5 expand → R6 private enablement with measured reuse.
**R0's verdict is already recorded** (`IDENTITY-READINESS.md`, 24 rows): store facade/connection/migration
**Ready**; `InputSnapshot` v1, `ProviderOutputMeta`, provider outcome, capability state, `LayerKey`,
`DependencyIndex`, `FactMeta`, workspace identity **Not ready**; `Digest`, `config_hash`,
`ProviderManifest`, `SummaryKey` **Conditional**.

**Research decisions amended:** D9 downgraded from *"must reuse existing kernel contracts"* to
*"should reuse … only after each contract passes an explicit persistence-readiness audit"* (confidence
lowered to medium); **D10 Delivery Unit** and **D11 Conservative Non-Reuse** added.

**Preserve these six patterns** (`65-LEARNINGS.md:126-168`): atomic active-pointer rotation; typed decode
before trust; provider-scoped dependency projection; **mutation pair tests** (every input gets a
must-invalidate *and* an unrelated must-preserve-hit case); conservative omission over speculative
certification; bounded review with backlog routing.

### Secondary recorded failures

- **The 30GB+ OOM** on the reference monorepo (`REQUIREMENTS.md:28`) — fixed by capability gating and
  rule-scoped discovery (`analysis_kernel/mod.rs:97-140`). This is the origin of the scale gate and the
  reason `run()` is gated in slices. Do not undo it.
- **v1.2 pattern** (`RETROSPECTIVE.md:70`): *"Public SDK/query promotion should be treated as a separate,
  gated decision after internal fixtures and benchmarks prove the contract"* — the discipline Phase 65
  violated, written down two milestones earlier.
- **The leak-gate/release-lock drift** (`ca9704dc`) — the public-surface leak gate breaks on every version
  bump because the excluded probe's `Cargo.lock` is not updated by release tooling.

---

## (e) Gaps vs. "the world's most capable static analysis engine"

### Premature or over-engineered relative to the goal

- **The v2.0 store milestone is sequenced before the accuracy work it is meant to serve.** `REQUIREMENTS.md:175`
  concedes the type-directed callgraph tier is *"the largest real-world F1 lever"* and then defers it for
  a persistence layer that currently persists nothing. Six of nine v2.0 phases (66-71) are storage and
  query plumbing; the differentiating capability is in the *next* milestone.
- **Phase 70 (Tantivy lexical search)** is already flagged as the cut. It should be cut now, not held.
- **Phase 69 (`polint graph` public promotion)** promotes a query surface over a store whose accuracy
  gate (BENCH-04) has never been measured, on 2 of 6 planned ecosystems.
- **4,215 LOC of orphaned analysis** (`slicing/` + `demand/`) built against standards, never wired.
  Both were built *before* the consumer that would use them existed.
- **The 132-field mutable `AnalysisDb` inside an 11,143-LOC `core/mod.rs`** is the structural debt that
  makes both incrementality and the store hard. Every research doc says not to do this.

### Missing entirely for the stated goal

1. **Nothing that can fail on accuracy.** There is a metrics module with F0.5/F1/F2/F3, promotion gates
   (`eval/gates.rs`), four external oracle adapters, and a tiering system — and **none of it runs in CI**
   (`.github/workflows/ci.yml:156-195`). The headline 89.06% Jelly F1 / 92.50% Go RTA F1 are unenforced.
   A "most capable" claim requires a gate that turns red when capability regresses. **This is the single
   cheapest, highest-leverage gap in the repo** — the machinery already exists.
2. **No real-app callgraph benchmark.** static-analysis-2.0 names `real_app_callgraph` as the adapter that
   would make the accuracy claim meaningful; only micro-fixture and oracle adapters exist. Micro-benchmark
   F1 is not a capability claim.
3. **Two languages.** Go + TS/JS. Python and Java are 50-line briefs with no parser decision. The
   *adapter contract itself* does not exist — there is no `LanguageAdapter` trait, and adding language #3
   currently means touching the whole tree (see doc 03).
4. **No interprocedural taint reaching a user.** `DataFlow<'_>` is Supported, but IFDS/IDE is unbuilt,
   `analysis/slicing/` is orphaned, and `Evidence` is internal-only. A rule author cannot get a
   source-to-sink path with an explanation — the thing security policy actually needs.
5. **No incrementality.** The vocabulary exists; the engine is a whole-program straight line with a
   content-addressed layer cache. The latency gate (warm review = invalidation frontier) has no
   implementation path shorter than Phase 67.
6. **No rule distribution.** Rules live in one repo and cannot be shared. For an engine whose thesis is
   "the policies only your team knows," the absence of a package format caps network effects at one repo.
7. **No "why didn't my rule fire?"** `polint explain` covers capability planning; there is no
   provenance-backed negative explanation, despite provenance being built end to end.
8. **The extension surface has no front door** (row 2). The host, protocol, sinks, and discovery are all
   built and wired; there is no command, no docs page, no template.
9. **No architecture document** (§f).

---

## (f) Recommendation: documentation architecture

**Is there a single coherent architecture document?** **No.** `AGENTS.md:67` says so verbatim, pointing at
an `ARCHITECTURE.md` that does not exist. The nearest candidates and why each falls short:

- `.planning/research/ARCHITECTURE.md` (173 lines, 2026-07-07) — genuinely good, but scoped to the v2.0
  store, and **its module plan is mostly unbuilt** (9 of 12 store modules absent). It describes a design,
  not the system.
- `docs/roadmap/12_RESOLVED_IMPORTS_MODULE_GRAPH_ARCHITECTURE.md` — a real architecture doc that
  explicitly disclaims everything else (*"It is not symbols, call graph, CFG, or dataflow"*).
- `research/implementation-bootstrap/implementation/RUST-ARCHITECTURE.md` (149 lines) — conventions, and
  partly violated.
- `docs/architecture-review/01-08` — reconstruct the pipeline from source. They are evidence that no
  architecture document exists, not a substitute for one.

### What should exist: one `ARCHITECTURE.md` at the repo root

Fill the marker block `AGENTS.md` already points at. Target ~600-900 lines. It must contain the six
things that are currently unrecoverable without reading 253k LOC:

1. **The provider DAG as the spine** — the 23 manifests at `provider.rs:257-875` in dependency order,
   each with: what it consumes, what it produces, which capability triggers it, and which of the five
   pipeline gates in `run()` (`mod.rs:97-113`) turns it on. This is the actual architecture and it is
   written down nowhere.
2. **The capability ladder** — `analysis_plan.rs:684-742` rendered as a table: capability → status →
   providers → SDK view → docs page. This is the contract between the engine and its users.
3. **The fact-model vocabulary** — `StableKey`, `FactMeta`, `Precision`, `Confidence`, `ValidationStatus`,
   `Digest`, `LayerKey`, `InputSnapshot`, the unknown/status taxonomy. Genuinely good work, invisible.
4. **The honesty contract** — how unknown / partial / setup-missing / unsupported / budget-exceeded
   propagate from provider to `polint unknowns`. This is the project's real differentiator.
5. **The boundary map** — `sdk`/`runner` public; everything else `pub(crate)`; the `_bench` escape hatch;
   the leak gate that enforces it. (Consolidate `docs/API-VISIBILITY-PLAN.md` into this section.)
6. **An explicit "built but not wired" register** — `slicing/`, `demand/`, `aliases/`, `points_to/`,
   `eval/`, the store — with, for each, the consumer that would activate it. The `expect(dead_code)` at
   `analysis/mod.rs:1-6` makes this register the only way to know what is live.

**Then delete the blanket `expect(dead_code)`.** Move it to the specific orphaned modules with a
`reason` naming the phase that will consume each. A crate-wide dead-code exemption over 112k LOC is
itself an architecture decision, and an undocumented one.

### Keep as current

`docs/facts/` (18 pages, the user-facing fact contract — load-bearing, a release gate requires a page
here) · `docs/schemas/` (9 versioned JSON schemas) · `docs/API-VISIBILITY-PLAN.md` (fold into
ARCHITECTURE.md §5) · `.planning/{ROADMAP,REQUIREMENTS,STATE}.md` · `research/local-semantic-store/RESTART-PLAN.md`
(the only credible plan for the current milestone) · `docs/architecture-review/01-09` (fresh; they are the
input to the target architecture, not the architecture).

### Archive to `docs/archive/` with a one-line "superseded by" header

`docs/roadmap/00_ROADMAP.md` and entries 01-09 (10 weeks stale, actively misleading; entries 08/09 should
be re-filed as `research/` sketches, since they are speculative briefs and not roadmap commitments) ·
`docs/ANALYSIS-ROADMAP.md` (**and remove the README link — it is the one stale doc users read**) ·
`docs/RUST-AUDIT-IMPROVEMENT-PLAN.md` · `docs/RUST-AUDIT-LIVING.md` · `docs/rust-audit-incoming/` (all
findings landed 2026-05) · `docs/RULE-AUTHORING-PLATFORM-REVIEW.md` (superseded by its own Fix Status) ·
`docs/INITIAL_PROMPT.md`.

### Reconcile, do not archive

- **`research/ROADMAP.md`**: mark PRs 1-22 with their actual disposition (shipped / shipped-at-different-path
  / dropped). Add `static-analysis-2.0` and `local-semantic-store` as tracks 17 and 18.
- **Each research `STANDARD.md`**: add an "As Built" line giving the real module path. Five of them
  currently point at directories that do not exist.
- **`.planning/ROADMAP.md:109` and `.planning/STATE.md:47`**: import the R0-R6 restart slices and delete
  *"Research flags: none."* The plan of record for the active milestone currently lives only in
  `research/` and contradicts the roadmap.
- **`.planning/RETROSPECTIVE.md`**: add the Phase 65 entry and the PR budget triggers. A retrospective
  that omits the largest failure trains the next agent to repeat it.
- **`docs/CAPABILITY-FULFILLMENT-RESEARCH.md`**: still the best statement of product direction; retitle
  its "Current Gap" section, which describes completed work as future.
