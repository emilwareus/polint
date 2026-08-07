# polint — Target Architecture

**Question asked:** is this codebase prepared to become the world's most capable static analysis
engine?

**Answer:** *Not as currently wired — but the hard part is already built.* The distance is disciplined
rewiring, not a rewrite: the expensive half (designing and implementing the right abstractions) is
largely done; the cheap half (connecting them) is missing. There is, however, a **closing window** —
every new language and analysis added on top of the current wiring multiplies the cost of fixing it.

Review date 2026-07-29 · base commit `1263208a` · 267,710 LOC Rust · 366 files · ~390 commits over
3 months by one human plus agents.

Supporting detail: [01 layering](01-layering-and-boundaries.md) ·
[02 Rust quality](02-rust-code-quality.md) · [03 frontends & IR](03-frontend-ir-and-language-scaling.md) ·
[04 analysis core](04-analysis-core-capabilities.md) · [05 incrementality](05-incrementality-and-store.md) ·
[06 performance](06-performance-and-scale.md) · [07 extension surface](07-extension-surface.md) ·
[08 evaluation](08-evaluation-and-correctness.md) · [09 declared direction](09-declared-direction-and-gaps.md) ·
[10 the bar](10-sota-landscape-and-bar.md)

---

## 1. The verdict

**What is genuinely excellent** — better than most commercial static-analysis tools, and worth
protecting through every refactor below:

- **Engineering discipline.** `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passes clean. `unsafe_code = "forbid"` with exactly one audited FFI exception. Rules run inside
  `catch_unwind`. Twelve CI jobs including a determinism gate and a public-surface-leak gate that
  compiles a probe crate *outside* the workspace to verify the API contract. Malformed sources
  produce diagnostics with real spans and analysis continues.
- **The fact model.** Stable keys, provider manifests with declared inputs/outputs/precision
  ceilings, capability planning, incremental digests, an honest unsupported-construct taxonomy. This
  is ahead of most open-source analyzers.
- **The rule contract.** `#[polint::rule]` deriving capabilities from typed fact-view parameters is
  the best design decision in the product. The signature *is* the capability request. For an
  LLM-authored rule, the type checker is the verifier — a wrong CodeQL query returns wrong results
  silently; a wrong polint rule fails to compile.
- **Self-honesty.** `research/static-analysis-2.0/00-critical-review.md` is sharper than most external
  reviews. `baselines/README.md:51-54` explicitly warns against claiming unmeasured recall lifts. The
  README makes no accuracy claims. This is the strongest single signal that the problems below are
  fixable.

**What blocks the goal** — five structural facts, each independently sufficient to cap the ceiling:

| # | Fact | Consequence |
|---|---|---|
| 1 | **17 traits and 10 `dyn` sites in 267k LOC.** No `LanguageFrontend`, no `Provider`, no `Analysis` trait. | Nothing is pluggable. Every extension is a shotgun edit. |
| 2 | **`AnalysisDb`: one struct, 132 fields, ~11k-LOC file, 288 methods.** Includes `ts_object_allocations` and `go_semantic_packages` — language-specific fields in the language-neutral core. | Adding an analysis or a language means editing the center of the universe. |
| 3 | **`AnalysisKernel::run()` is 877 lines of straight-line pipeline** (`analysis_kernel/mod.rs:92-968`). The 23-entry `PROVIDER_MANIFESTS` table declares the dependency DAG — and is **metadata only**; nothing executes from it. | The scheduler that would make providers pluggable is written down but not run. |
| 4 | **26 of 27 top-level modules are in import cycles**; `core`↔`analysis` alone is 318/98 references. | No crate split is possible without breaking cycles first. One 253k-LOC compilation unit, 66 s per `cargo check`, 49% CPU. |
| 5 | **Strings are the identity model.** 229 `stable_key: String` fields, 318 `BTreeMap<String,_>`, 476 `Vec<String>`, **zero interner**. The Go RTA fixpoint's reachable set is a `BTreeSet<String>`. | Memory and lookup cost scale with identity *text*, not with facts. Every perf item is downstream of this. |

**And three capability facts:**

6. **It has never been measured on a repository.** The only whole-repo numbers that exist —
   ~1 GB peak RSS, 7.4 s cold, 4.6 s warm — are a *comment in a TOML file* for a private repo whose
   LOC count is recorded nowhere. Derived from the struct definitions, polint retains **≈5.6 KB per
   LOC** (≈8 facts/LOC × ~700 B/fact, of which **~66% is redundant copies of the same identity
   string**). That projects to **~5.6 GB at 1M LOC — an OOM on a standard 7 GB GitHub runner** — and
   hard OOM everywhere at 10M. Meanwhile the TypeScript corpus is re-parsed **up to 12 times per
   run, serially**, and file lookup inside the syntax-cache restore is **O(F²)**
   (`ts/adapter.rs:338-340`). Zero of the four `rayon` call sites in the crate are inside
   `analysis/` (112,545 LOC): whole-program analysis is 100% single-threaded.
7. **The MIR is not an IR.** No basic blocks, no terminators, no SSA, no types, no operators, no
   exceptions, no closures. `MirStatement` and `MirTerminator` are declared and **constructed
   nowhere**. The CFG layer therefore reconstructs loops by *substring-matching source text*
   (`analysis/cfg/lower_ts.rs:249-254`). The TS pipeline has already forked around it:
   `ts_value_flows.rs` (11,898 LOC) re-parses with its own `oxc_parser` and its own resolver.
8. **Taint — the headline capability — does not exist.** Sources are done (14 typed kinds).
   Propagation and sinks are enum variants with no producers. The interprocedural layer is BFS
   reachability with no call/return matching, so it can report unrealizable paths.

---

## 2. The pattern that explains everything

Read the reviews together and one pattern recurs with startling consistency:

| The right abstraction was designed and built… | …and then not connected |
|---|---|
| `PROVIDER_MANIFESTS` — 23 providers with declared inputs, outputs, cache policy, precision ceiling | Nothing schedules from it; `run()` is hand-written |
| `QueryKey` / `SummaryKey` / `DependencyEdge` / 5-action invalidation planner — ~11,500 LOC | No query function calls `lookup` before computing |
| SQLite store with migrations, connection policy, schema versioning | Contains exactly one table: `_polint_schema_migrations` |
| `analysis/evidence/` — 4,335 LOC of provenance, unknown reasons, omitted regions, replay keys | Actively stripped at `diagnostics/mod.rs:1136-1139` before any user sees it |
| `js_points_to/solver.rs` — a real field-sensitive Andersen solver with honest budget latching | Not the primary call-graph resolver; a recognizer bank is |
| `AbstractDomain` / `SummaryDomain` lattice kernel with widening | Solver is intraprocedural |
| `gates.rs` — 746 LOC of promotion gates, unit-tested against 9 scenarios | No production caller |
| Metrics module computing F0.5/F1/F2/F3 | **No test anywhere asserts a precision, recall, or F1 value** |
| `analysis/extensions/` — discovery, digesting, subprocess host, sink validation, cache keying | No CLI command, no docs, no `polint::extension` module; the only working example hand-writes JSON with zero dependencies |
| `analysis/slicing/` — 2,027 LOC | **Zero references anywhere in the crate** |
| `analysis/demand/` — 2,188 LOC | Referenced only as five string literals in `provider.rs:1816-1821` |

And the pattern has already cost real money once. **Phase 65** — 126 commits, 154 files, +85,404
lines, with review adding a further +50,329 *after* the phase was marked complete, CI time 20 → 60
minutes — was abandoned. Its own forensics record the lesson verbatim: *"complete artifacts did not
mean a healthy phase."* That is this pattern, at scale, with a bill attached.

The sharpest single illustration of the measurement half: the Jelly call-graph benchmark went from
**F1 1.07% → 88.75% over 62 iterations**, while runtime went **793 ms → ~90–105 s (≈110× slower)**.
**The runtime column was dropped from the log at iteration 57.** The one budget ever written (≤ 20 s)
was never enforced. Accuracy was bought with a two-order-of-magnitude regression, and then the
instrument that would have shown it was removed.

This is not incompetence. It is the predictable signature of **high-velocity phase-based development
by agents**: each phase built its deliverable well, and *connecting* things is nobody's phase. The
expensive half — designing and implementing the right abstractions — is largely done. The cheap
half — wiring them — is missing.

That is very good news. It also means the highest-leverage work is unglamorous.

**The corollary for process:** at 267k LOC in 3 months, the binding constraint is not typing speed.
It is *conceptual integrity* — and the only thing that preserves conceptual integrity at agent
velocity is **mechanically-enforced invariants**. This codebase already proves it can build them
(the leak gate, the determinism gate). It should point that instinct at layering, identity, and
accuracy.

---

## 3. Seven inversions

The target architecture is the current architecture with seven things turned inside out. Each is
independently shippable and independently valuable.

### I1 — Control: pipeline → scheduler
**From** an 877-line `run()` with six boolean gate flags.
**To** a scheduler that topologically sorts `manifest().inputs` against `manifest().outputs`.

```rust
pub trait Provider: Send + Sync {
    fn manifest(&self) -> &ProviderManifest;   // reuse provider.rs:2-11 verbatim
    fn run(&self, ctx: &mut ProviderCtx<'_>) -> ProviderOutput;
}
```

The manifest data already exists. This deletes the 877-line function, both duplicated order
assertions, and the 20× repeated digest boilerplate — and it is what turns the extension protocol
(already built, `analysis/extensions/`) into something with more than four hard-coded sinks to plug
into.

### I2 — Data: god struct → provider-owned fact stores
**From** `AnalysisDb` with 132 fields, eagerly materialized, whole-corpus-resident, never shrunk.
**To** a `FactStore` keyed by family, where each provider owns its store. **Thirteen such stores
already exist** (`CallStore`, `DataFlowStore`, `EvidenceStore`, …); `AnalysisDb` becomes a keyed
container. `sdk/facts.rs` views already hold a private `db` field, so **the public SDK surface does
not change.**

This is what kills the language-specific fields in the neutral core, and it is the prerequisite for
eviction and persistence.

### I3 — Identity: strings → interned IDs
**From** 229 `stable_key: String` fields beside integer IDs that already exist, zero interner.
**To** `StableKeyId(u32)` interned in the store, migrated family by family (highest-cardinality
first: `SymbolFact`, `ReferenceFact`, then MIR, then call sites).

This is the keystone for memory and speed. Every other performance item is downstream. rust-analyzer
solved this in 2019 with `SmolStr` + interning; oxc with arena `Atom<'a>`; ruff with `ustr`. polint
has the newtype IDs already — it just never made them the identity.

### I4 — Language: closed enum → open registry
**From** `enum Language { Go, TypeScript, Tsx, JavaScript, Jsx, Unknown }` with **1,016 references
across 129 files**, plus three competing parallel taxonomies (`LanguageTag`, `LanguageScope`,
`RuleLanguage`).
**To**:

```rust
pub trait LanguageFrontend: Send + Sync {
    fn id(&self) -> LanguageId;                 // open, registry-assigned
    fn handles(&self, path: &Path) -> bool;     // replaces Language::from_path

    /// What this frontend can produce, and at what precision. The planner refuses
    /// capabilities no registered frontend can back, instead of silently
    /// under-approximating. Same honesty mechanism as `ProviderManifest`.
    fn profile(&self) -> &FrontendProfile;

    fn analyze(&self, ctx: &FrontendCtx<'_>, unit: &AnalysisUnit<'_>) -> FrontendOutput;
}
```

The signatures are *already identical* — `go/adapter.rs` and `ts/adapter.rs` both export
`analyze_with_plan_options_and_cache_stats` with byte-identical signatures, called back-to-back by
hand at `analysis_kernel/mod.rs:191` and `:209`. The trait is a formalization of what exists.

**Two deliberate generalizations beyond what exists**, both free now and expensive to retrofit:

1. **`FrontendProfile` — declare, don't assume.** A frontend states which fact families it can
   produce and at what precision. This is what lets the planner say "no registered frontend can back
   `symbols` for Python here" instead of running rules on placeholder facts. It is the frontend-side
   twin of `ProviderManifest.precision_ceiling`.
2. **`AnalysisUnit`, not `&[&SourceFile]` — a frontend is not necessarily a parser.** Writing
   production frontends for ten languages is a multi-year project; consuming SCIP / LSIF / LSP /
   `gopls` / `tsc` output is not. An indexer adapter has no AST and different fidelity, but it can
   still emit `FrontendOutput`. Defining the input as a unit of work rather than a slice of source
   text is what keeps "import someone else's index" a valid frontend rather than a special case
   bolted on later. **This is the cheapest available lever on language breadth and it must be
   honoured in the trait's first version.**

**Adding Python today costs ~19 mandatory edit sites across three supposedly language-agnostic
modules, before one line of Python analysis. After this inversion it costs one crate plus one line
in the composition root.**

### I5 — IR: annotation format → real IR
Add the structural layer that is entirely absent: `MirBlock`, and terminators —
`Goto`, `Branch { predicate, then, else }`, `Switch`, `Return`, **`Throw { value, unwind }`**,
`Call { site, normal, unwind }`, **`Suspend { kind: Await|Yield|ChannelRecv|ChannelSend }`**,
`Unreachable`, `Unsupported`. Plus `BinOp`, `Aggregate`, `Closure { body, captures }`, and
`place-fact record.ty` referencing the existing type lattice.

Payoffs, in order of size:
- **`Throw` / `Call { unwind }`** is the difference between "Java, Python, C#, Kotlin, Swift are
  analysable" and "they are not."
- **Blocks + terminators** delete both per-language CFG lowerers (1,629 LOC) and the substring
  matching, permanently — for every future language.
- **`Closure` with explicit captures** is why `ts_value_flows.rs` forked; it is what lets 11,898 LOC
  fold back into the shared pipeline.
- **Branch predicates** are the prerequisite for path sensitivity, which is structurally dead today.

Keep `unsupported-semantic fact record` exactly as-is. Keep the
`mir_contract_source_does_not_store_parser_ast_objects` guard and extend it to every new IR file.

### I6 — Analysis: 18 unioned producers → one principled engine
**From** a call graph that is the set union of three resolvers with mutually inconsistent semantics,
tagged by which of 18 producers emitted each edge, with a downstream reachability filter masking a
53%-precision resolver to report 96%.
**To** an **IFDS/IDE solver over a repaired ICFG, with Andersen points-to underneath.**

- Promote `js_points_to` (a real Andersen solver) from auxiliary to primary; add object sensitivity.
- IFDS gives interprocedural taint *and* realizable-path discipline in one construction — items 2 and
  3 on the missing-analyses list are the same build.
- Constant propagation and nullability lift to IDE once the solver exists; the domains are already
  written and correct.

**Not Datalog.** The project already evaluated and rejected it
(`research/incremental-query-engine/RESEARCH-ANALYSIS.md:45`). That decision was right and should
stand.

### I7 — Evidence: stripped → shipped
Delete the three lines at `diagnostics/mod.rs:1136-1139` that null out `evidence_v1` and
`evidence_bundle` at the rule-host boundary, for the query families that already produce it. Make
`StructuredEvidenceV1` public and versioned.

This is the largest built-but-unshipped asset in the repository — 4,335 lines implementing exactly
the provenance model the goal requires. It is also the clearest product differentiator: an agent
consuming findings needs to know *why* to decide whether to act. A finding without provenance is a
coin flip an LLM will confidently rationalize.

---

## 4. Target architecture

```
                          polint  (facade — the only published crate)
                                       │
                 ┌─────────────────────┼─────────────────────┐
                 ▼                     ▼                     ▼
           polint-sdk            polint-runner          polint-cli
                 └──────────────────┬──┴─────────────────────┘
                                    ▼
                             polint-host                 ← COMPOSITION ROOT
                    the ONLY crate naming concrete           Vec<Box<dyn LanguageFrontend>>
                    frontends and concrete analyses          Vec<Box<dyn Provider>>
                                    │
    ┌──────────────┬────────────────┼──────────────────┬────────────────────┐
    ▼              ▼                ▼                  ▼                    ▼
polint-go     polint-ts      polint-py …      polint-analysis-*        polint-kernel
    │              │                │        (ifds, callgraph,       (scheduler from manifests,
    └──────┬───────┴────────────────┘         points-to, types,       demand queries, layer
           ▼                                  cfg, summaries)         cache, persistent store)
   polint-frontend-api                              │                        │
   trait LanguageFrontend                           ▼                        │
           │                                polint-analysis-api ◀────────────┘
           │                          trait Provider · ProviderManifest
           │                          trait FactStore · open CapabilityId
           └────────────┬───────────────────────────┘
                        ▼
                   polint-ir          MIR: blocks, terminators, places, types,
                        │             effects (Throw / Suspend / Unwind)
                        ▼
                  polint-core         FileId · Span · StableKeyId (interned)
                        │             LanguageId registry · Diagnostic
                        ▼
                   polint-vfs         discovery, bounded reads, no fact types
```

**Four invariants the crate graph enforces by construction:**

1. `polint-kernel` never names a concrete analysis or language — only `polint-analysis-api`.
2. `polint-core` never names a concrete fact.
3. `polint-analysis-*` never names a frontend. Only `polint-lower-*` crates sit between them.
4. `polint-host` is the single composition root. **Adding a language = one crate + one line.**

---

## 5. Sequencing

**The executable sequence lives in [PLAN.md](PLAN.md)** — milestones, per-item dependencies, exit
gates, and the scoreboard. It is the single source of truth for ordering; this section states only the
dependency spine so the architecture and the plan cannot drift apart.

```
  [gates: accuracy + layering]          ← nothing structural starts before these are green
              │
              ├─── cheap wins (evidence, telemetry, parse cache, O(F²)) ── independent, parallel
              │
              ▼
  [split core] ──▶ [I3 interning] ──▶ [store persistence] ──▶ [summaries] ──▶ [demand queries]
       │
       └──▶ [I1 Provider trait + scheduler] ──▶ [I2 FactStore] ──▶ [I4 LanguageFrontend]
                                                                          │
                                                                          ▼
                                              [I5 real MIR] ──▶ [I6 IFDS/IDE] ──▶ [taint]
                                                                          │
                                                                          ▼
                                                              [crate split] · [Python] ·
                                                              [shareable rule packs]
```

Three ordering constraints are load-bearing and easy to get wrong:

- **Interning before store persistence.** Persisting today's fact model bakes ~66% redundant identity
  strings into a versioned on-disk schema, behind a migration. See [PLAN.md](PLAN.md) §1.
- **Real MIR before IFDS.** An interprocedural solver over a graph where nothing is control-dependent
  on anything, and branch shapes are recovered by grepping source text, produces confident nonsense.
- **Gates before refactor.** Agents will break invariants silently; the accuracy and layering gates
  are what make the rewiring safe to attempt at all.

Everything up to the crate split happens **inside the current single crate**. The split is not a
prerequisite for anything — it falls out once the cycles are gone.

### If you only do three things

1. **Wire the accuracy gate.** Without it, no capability claim is falsifiable, including to yourself.
2. **Intern the identity model.** Everything else about scale is downstream.
3. **Build the `Provider` trait and scheduler.** It is the difference between a pipeline and a
   platform, and the data it needs is already written down.

---

## 6. What to stop doing

- **Stop adding languages and analyses on the current wiring.** Language #3 costs ~19 mandatory edit
  sites now and ~1 after I4. Analysis #24 costs 8 edit sites and has no persistence story — 16 of 17
  existing analyses have already opted out of the layer cache. Every addition makes the rewiring
  more expensive, superlinearly.
- **Stop building abstractions without a caller.** The pattern in §2 is the dominant failure mode.
  New rule: an abstraction is not done until something in the product path uses it and a test would
  fail if it stopped. Corollary: **narrow the crate-wide `expect(dead_code)`** at
  `analysis/mod.rs:1-6` that currently covers 112k LOC and hides `slicing/` (2,027 LOC, zero
  references) from the compiler.
- **Stop deferring the one document that would prevent all of this.** `AGENTS.md:67` says
  "Architecture not yet mapped" and points at an `ARCHITECTURE.md` that does not exist — while
  22 research areas, 12 roadmap entries, and two RUST-AUDIT documents describe fragments of it. Write
  one root `ARCHITECTURE.md`: the provider DAG as the spine, the capability ladder, the fact-model
  vocabulary, the honesty contract, the boundary map, and an explicit **built-but-not-wired
  register**. Archive `docs/roadmap/*`, both RUST-AUDIT docs, and `rust-audit-incoming/`; reconcile
  (don't archive) the five research STANDARDs the implementation has silently drifted from.
- **Stop letting accuracy work run without a cost column.** Every benchmark iteration records
  runtime and peak RSS alongside F1, or the iteration doesn't count.
- **Stop encoding review history in comments.** `go_rta/fixpoint.rs:98-150` cites "D-07", "CR-01",
  "FINDING 7", "R3" across 40 lines whose referents live in `.planning/` and will be unresolvable to
  any future reader. This directly violates the repo's own shipped-code comment policy.
- **Stop the meta-tests that grep the project's own source** (9 `include_str!("*.rs")` assertions).
  Dogfood a polint rule instead — that is literally the product.
- **Do not build a query DSL.** The ADR
  (`research/agent-rule-authoring/decisions/001-typed-rust-rules-not-dsl-first.md`) decided this and
  the decision is right. But **measure rule-pack cold compile time** — it is currently unmeasured and
  is the ADR's own stated revisit trigger. Don't let that decision get made for you by an unmeasured
  number.
- **Do not chase soundness or language-count parity.** Be *honest* instead: declare precision per
  finding, name the unknowns. Bounded-and-honest is a stronger position than sound-in-principle, and
  it is the position the evidence system was built for.

---

## 7. How you will know it worked

Falsifiable gates, in the spirit of the milestone gates already adopted. Each should be a CI check,
not a doc claim.

| Gate | Metric | Target |
|---|---|---|
| **Layering** | wrong-direction module edges | monotonically decreasing from 155; zero new |
| **Cycles** | mutually-recursive top-level module pairs | 26 → 0 |
| **Language cost** | edits outside its own crate to add a trivial frontend | ≤ 2 |
| **Analysis cost** | edits outside its own module to add a provider | ≤ 1 (registration) |
| **Compile** | `cargo check -p <largest crate> --all-targets`, warm | 66 s → < 20 s |
| **Memory** | retained bytes per LOC | ~5.6 KB/LOC → < 1 KB/LOC; absolute ceiling that fails CI |
| **Scale** | largest repo ever actually analysed | *unknown today* → a public number, on a public repo, in CI |
| **Runtime** | benchmark wall-clock, recorded on **every** accuracy iteration | never regress 110× again — restore the column that was dropped at Jelly iteration 57 |
| **Accuracy** | Jelly F1, and a taint benchmark that does not yet exist | asserted in CI with a regression budget |
| **Precision** | FP rate on a labelled corpus per shipped template | measured, then floored |
| **Evidence** | share of findings carrying a replayable path | 0% → >90% for query-family rules |
| **Incrementality** | warm re-run after a one-function edit | measured hit/miss/recompute, published |

The evaluation review's judgement is the one to internalize: *"polint is measuring far less than it
has built the capacity to measure, and the gap between its real engineering progress and its
verifiable evidentiary record is now its largest strategic risk."*

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| **Refactor stalls feature work and momentum dies.** | The gate and cheap-win work ships user-visible value on every merge (evidence, telemetry, accuracy gate) and runs in parallel with the structural track. The inversions are where discipline is required — and by then the gates make them safe. |
| **Agent-driven refactor breaks invariants silently.** | This is exactly what Phase A exists for. Do not start Phase C before A1 and A2 are green. |
| **Interning migration touches everything at once.** | Migrate one fact family at a time behind the existing newtype IDs. The Go RTA fixpoint is a self-contained first proof with an immediate 5–20× payoff. |
| **IFDS is a research project.** | It is not — it is a well-specified solver with reference implementations (WALA, SootUp, Heros). The risk is starting it before the ICFG is repaired (I5). Do not. |
| **Scope explosion toward "most capable".** | §6 and [10](10-sota-landscape-and-bar.md)§f. The realistic path is not matching CodeQL on depth or Semgrep on breadth; it is being the only engine where a repo's own policies are executable, compiler-verified, provenance-explained, and fast — with depth accruing underneath. |

---

## 9. The one-paragraph version

polint is a remarkably well-engineered **pipeline** that has been designed, throughout, as if it were
a **platform** — and the platform wiring was never connected. The manifests, the query algebra, the
persistent store, the evidence model, the Andersen solver, the lattice kernel, the promotion gates:
all built, none load-bearing. Fixing this is rewiring, not rewriting, and the ordering is unusually
clear: make the invariants enforceable, intern the identity model, execute the provider DAG that is
already declared, make the IR real, then build one principled interprocedural engine on top. Do that
and the answer to "is it prepared?" becomes yes. Keep adding capability breadth on the current
wiring and the answer hardens into no.
