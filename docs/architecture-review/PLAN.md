# polint — The Plan

**Purpose:** turn the review's findings into an executable sequence. Feeds `.planning/`; does not
replace it.

**Companion:** [00-TARGET-ARCHITECTURE.md](00-TARGET-ARCHITECTURE.md) says *what* the architecture
should be. This says *in what order, and how you'll know it worked.*

**No durations appear in this document.** Work is ordered by dependency, not by calendar. The only
questions that matter are *what must precede what*, *what can run in parallel*, and *what proves a
milestone is done*.

**One branch for the whole refactor.** All swarm work for this track lands only on
`static-analysis-architecture-review`. Do not merge that branch into `main` as part of orchestration
or milestone completion — `main` stays untouched until a human explicitly ships. See
[ORCHESTRATION.md](ORCHESTRATION.md) (Integration branch).

---

## 0. What "fantastic" means

Not "the problems are fixed." A destination, stated so that a stranger could check it:

| Property | Falsifiable form |
|---|---|
| **Languages compose** | Adding a language costs one crate + one line in the composition root. Measured: ≤ 2 edits outside its own crate. |
| **Analyses compose** | Adding an interprocedural analysis costs one crate + one registration. Nothing in `core` changes. |
| **Every finding explains itself** | > 90% of findings carry a replayable evidence path with declared precision and named unknowns. |
| **Scale is boring** | 10M LOC analysed under a fixed memory ceiling. Bytes retained per LOC < 1 KB. |
| **Edits are instant** | Warm re-analysis after a one-function edit touches only the invalidation frontier. |
| **Claims are reproducible** | A stranger runs `make bench` and gets the published precision/recall numbers. |
| **Rules compound** | A rule written in one repo runs in another, versioned and sandboxed. |
| **It proves itself on itself** | polint enforces polint's own architecture in polint's own CI. |

The last row is the north star. A static-analysis engine whose own layering is guaranteed by itself is
an argument no competitor can make.

**Strategy in one line:** polint does not win by out-depthing CodeQL or out-breadthing Semgrep. It wins
by being the only engine where a repository's own policies are *executable, compiler-verified,
provenance-explained, and fast* — with analysis depth accruing underneath that, one honestly-measured
capability at a time.

---

## 1. The scheduling call to make first

**v2.0 as currently scoped is out of order. Interning must precede store persistence.**

The v2.0 milestone (phases 63–71) persists semantic facts to SQLite. But ~66% of every fact's memory
is redundant copies of the same `stable_key` / `payload_digest` strings (stored 3× and 2×
respectively). Persisting the fact model *as it stands today* bakes that waste into a versioned
on-disk schema — and then every future fix requires a migration.

```
  Current v2.0 order:   [store persistence] → [summaries] → [incremental]
  Correct order:        [interning] → [store persistence] → [summaries] → [incremental]
```

Interning shrinks the thing you are about to persist by roughly 3×. Doing it after means paying for it
twice, the second time behind a migration. **This is the single highest-value scheduling change
available.**

Second call: **defer the type-directed call graph.** It is self-described as the "largest real-world
F1 lever," and it is — but it is a precision lever bolted onto a resolver that M4 replaces. Build it
on the IFDS engine, not on the recognizer bank.

---

## 2. Working rules (from the Phase 65 post-mortem)

Phase 65 was 126 commits, 154 files, +85,404 lines, with review adding +50,329 *after* it was marked
complete. CI went 20 → 60 minutes. It was abandoned. Its own forensics record the lesson: *"complete
artifacts did not mean a healthy phase."*

These rules exist so it doesn't happen twice:

1. **One integration branch; never auto-merge to `main`.** All work for this track lands on
   `static-analysis-architecture-review` only. Milestone gates do not imply a merge to `main`.
2. **Hard PR budget: ≤ 1,500 changed lines, ≤ 25 files.** If a task exceeds it, split *before*
   continuing, not after. This is a stop condition, not a guideline.
3. **No abstraction merges without a product-path caller** *and* a test that fails if the caller
   stops using it. This is the direct antidote to the built-not-wired pattern that produced
   `slicing/` (2,027 LOC, zero references), a store with one table, and 746 LOC of ungated gates.
4. **Every PR green on all M0 gates.** No "fix the benchmark later."
5. **Every accuracy change records runtime and peak RSS.** The Jelly benchmark went 110× slower while
   F1 improved, and the runtime column was deleted at iteration 57. Never again.
6. **One-way doors get a written decision.** On-disk schemas, public SDK types, and the wire protocol
   are one-way. Everything else can be revisited.
7. **Comments explain enduring behaviour, never delivery history.** "D-07" / "CR-01" / "FINDING 7"
   are unresolvable to any future reader — including future you.

---

## 3. The milestones

Each table's **Depends on** column is the real schedule. Anything with no dependency can start
immediately and in parallel with anything else at the same level.

> **Implementing agents:** read [HANDOFF.md](HANDOFF.md) then the binding spec in
> [specs/](specs/README.md) for your item. **Precedence: spec > HANDOFF > PLAN > review documents.**
> Items linked below have specs; those specs correct several imprecisions in this document.

### M0 — Safety net

**Nothing structural starts until this is green.** You cannot refactor what you cannot measure, and
right now nothing in CI can fail on accuracy, architecture, or observable behaviour.

#### Why the existing suite does not cover this

The test suite is large and, for this specific job, the wrong shape:

| Measurement | Value | Why it matters here |
|---|---|---|
| Inline `#[cfg(test)]` tests in `src/` | **2,429** | Live *inside* the modules being restructured. Splitting `core/mod.rs`, introducing `FactStore`, interning `stable_key` — these break, move, or get rewritten by the same agents doing the refactor. |
| Integration tests in `tests/` | **174** | 93:7 ratio. The thin layer is the only part that survives an inversion untouched. |
| Snapshot directories | **0** | `insta` is 5 inline assertions in one file. There is no golden-output corpus. |
| Timing assertions in the integration suite | **0** | Performance can regress arbitrarily without a single test failing. |
| Test naming | `phase41_`, `phase55_`, `phase56_`… | Organised by *delivery history*, not by capability. Coverage is "whatever the phase added," not "the capability surface." |

What `tests/cli.rs` (12,166 lines, 174 tests) actually asserts is **contracts**: schema shape,
public-surface leaks, help text, capability plans, JSON field stability. That work is genuinely good
and must be preserved. But it is orthogonal to the question that matters during a re-architecture:

> **Given this repository, does polint still find exactly the same things, in the same time, using the
> same memory?**

Nothing in the tree asserts that today. That assertion is the only one that is simultaneously
implementation-agnostic and capability-sensitive — which makes it the one an agent fleet cannot
accidentally erase, and the one that catches a silent capability loss.

#### M0.A — Behavioural lock-in (do this first, before any other M0 item)

| # | Work | Depends on |
|---|---|---|
| W0.A1 | **Golden corpus.** Assemble the analysis targets: the 17 example rule packs, the 27 `tests/eval-fixtures/` trees, and the 3 declared-but-never-fetched scale repos (pinned by commit). This is the input surface. | — |
| W0.A2 | **Characterization harness.** Example self-pairs only: each `examples/<name>/` with its own `.polint/rules`, format **`json` only**. Run the real CLI, normalize (sort by stable fingerprint; strip absolute paths, timings, versions), commit goldens. Eval-fixtures are **not** in the golden cartesian. Scale checkouts: optional loud-skip. **No-unintended-change** assertions — generate from current behaviour; do not hand-author. Binding: HANDOFF §5. | W0.A1 |
| W0.A3 | **Capability matrix.** One fixture per `(fact view × language)` across the full prelude, asserting the view returns non-empty, well-formed data. This pins *capability* rather than *output*, and it is what catches "the refactor quietly dropped Go symbol resolution." | W0.A1 |
| W0.A4 | **Per-case cost record.** Wall-clock and peak RSS for every golden case, committed alongside the output, with a per-case regression budget. Wire the existing `eval/bench/measure.rs` RSS instrumentation. | W0.A2 |
| W0.A5 | **Baseline-acceptance discipline.** Golden updates require an explicit opt-in env flag (`POLINT_UPDATE_GOLDENS`) that **CI never sets**, plus a human-readable diff in the PR. See the agent-fleet rule below. | W0.A2 |

#### M0.B — Gates

| # | Work | Depends on |
|---|---|---|
| W0.1 | **Accuracy gate.** Replace the silent `return` at `eval/external/mod.rs:27-29`; regenerate the `null` baseline with the real numbers; `assert!(f1 >= baseline - 0.005)`. | — |
| W0.2 | **Cost columns.** Record runtime + peak RSS on every benchmark iteration; fail the build on a budget breach. **Coexists with W0.1** in one test path and one baseline JSON (F1 + cost columns). Binding: HANDOFF §5. | — |
| W0.3 | **Layering dogfood — MERGED-NOOP for M0.** Do not invent a Rust frontend. Deferred until a Rust language adapter exists (W2.6). Written rationale in HANDOFF §5; M0 gate treats this as closed without claiming layering is enforced. | — |
| W0.4 | **Scale corpus run.** Measure what fits under a documented ceiling; publish LOC + peak RSS + wall-clock. Suites that OOM record the failure **loudly** in the artifact (with LOC attempted). No Grafana `full_pipeline` requirement for M0. Binding: HANDOFF §5. | W0.A1 |
| W0.5 | **Memory metric.** Retained-bytes-per-LOC in CI, with a ceiling. | W0.4 |

**Exit gate:** CI fails on **any change to the golden diagnostic set**, a per-case time or RSS
regression, an F1 drop, or a silent scale-corpus measurement loss (OOM must be recorded, not skipped).
Layering dogfood is deferred (W0.3 MERGED-NOOP) until a Rust frontend exists. Every capability in the
prelude has at least one fixture proving it produces data for every language that claims to support
it. **polint has published its first real numbers on a real repository.**

#### The agent-fleet rule

The single most likely way this refactor fails is: **an agent sees a red golden test and updates the
golden file.** That converts the safety net into a rubber stamp, silently, and no reviewer notices
because the diff looks like "test data changed."

Therefore:
- Golden files are **append-only from CI's perspective**. Regeneration requires a local env flag CI
  never sets.
- Any PR touching a golden file must state, in prose, **which behaviour changed and why it is
  intended**. A golden diff with no behavioural justification is an automatic reject.
- Behaviour changes and refactors go in **separate PRs**. A PR may change structure or output, never
  both. This is the single most important rule in this document after the 1,500-line budget — it is
  what makes a red test unambiguous.

---

### M1 — Cheap wins

Independently shippable, user-visible, and structurally isolated. Runs in parallel with the start of
M2. Buys credibility and momentum before the hard part.

| # | Work | Depends on |
|---|---|---|
| [W1.1](specs/W1.1-parse-error-honesty.md) | **Fix the parse-error drop.** Route all 12 oxc parse sites through one helper mapping `parsed.errors` → `unsupported` facts. *The only finding that makes the analyzer wrong.* | — |
| [W1.2](specs/W1.2-ship-evidence.md) | **Un-strip evidence.** Delete the null-out at `diagnostics/mod.rs:1136-1140` for query families that already produce it; make `StructuredEvidenceV1` public and versioned. | — |
| [W1.3](specs/W1.3-rule-telemetry.md) | **Rule-execution telemetry.** Emit `{rule_id, planned, capabilities_ok, files_in_scope, diagnostics_emitted}` per planned rule. | — |
| W1.4 | **Kill the O(F²) scans** (`ts/adapter.rs:338`, `go/adapter.rs:299`, `cfg/lower_ts.rs:346`). `FileId` is dense; these are index lookups written as linear scans. | — |
| [W1.5](specs/W1.5-parse-cache.md) | **MERGED-NOOP** — measure-first: parse ~6.23% of run (~1 parse/file); do not invent a cache. Evidence: [`W1.5-STEP1-MEASUREMENT.md`](W1.5-STEP1-MEASUREMENT.md). | — |
| W1.6 | **Gate `validate_fact_metadata`** behind a flag instead of running 5,780 LOC of validators on every user invocation. | — |
| W1.7 | **Bound source reads** through the `repo_fs` helper that already exists. One 2 GB generated file currently gets read whole. | — |
| W1.8 | **`#[non_exhaustive]` on SDK prelude types and `Language`.** Free before 1.0, impossible after. | — |
| W1.9 | **Agent-surface hygiene.** Byte-equality test for generated vs checked-in `SKILL.md`; rename `positive`/`negative` fixtures to `clean`/`violating`; fix the two broken doc anchors. | — |

**Exit gate:** > 90% of query-family findings carry an evidence path · a silent rule is diagnosable in
one command · measured wall-clock improvement on the M0 scale corpus · zero parse errors silently
dropped.

---

### M2 — Break the monolith

The structural rewiring. W2.1 and W2.2 are mechanical and unblock everything else.

| # | Work | Depends on |
|---|---|---|
| W2.1 | **Evict `eval/` to a dev-only crate.** 29,344 LOC, dead in release, 3 non-test refs. Kills a 181-reference cycle for free. | — |
| W2.2 | **Split `core/mod.rs`** (11,143 lines) into `{ids, lang, span, facts/*, db, rule, capability}`. No API change. | — |
| [W2.3](specs/W2.3-interning.md) | **`StableKeyId` interner.** Prove it on the Go RTA fixpoint (`BTreeSet<String>` → bitsets over `SemanticNodeId`), then migrate fact families by cardinality: `SymbolFact` → `ReferenceFact` → MIR → call sites. | W2.2 |
| [W2.4](specs/W2.4-provider-trait-and-scheduler.md) | **`Provider` trait + scheduler.** Topologically sort `manifest().inputs` against `manifest().outputs`. The data is already declared at `provider.rs:255-884`. Deletes the 877-line `run()`, both duplicated order assertions, and 20× repeated digest boilerplate. | W2.2 |
| [W2.5](specs/W2.5-fact-store.md) | **`FactStore` trait.** `AnalysisDb` becomes a keyed container; each provider owns its store (13 already exist). The SDK surface does not change. | W2.4 |
| [W2.6](specs/W2.6-language-frontend.md) | **`LanguageFrontend` trait + open `LanguageId`.** Introduce alongside the enum; migrate the 999 call sites opportunistically. Must ship with `FrontendProfile` (declared fact families + precision) and an `AnalysisUnit` input rather than `&[&SourceFile]`, so an **external-index adapter (SCIP/LSIF/LSP) is a valid frontend, not a later special case.** Cheapest available lever on language breadth; expensive to retrofit. | W2.4 |

**Exit gate:** retained bytes/LOC **5.6 KB → < 2 KB** · adding a trivial third frontend costs **≤ 2
edits** outside its own module · adding a provider costs **≤ 1** registration edit · module cycles
**26 → < 5** · `cargo check` **66 s → < 30 s**.

---

### M3 — Make the IR real

Unlocks languages and taint simultaneously. Nothing in M4 can be built correctly first.

| # | Work | Depends on |
|---|---|---|
| W3.1 | `MirBlock` + terminators: `Goto`, `Branch { predicate, then, else }`, `Switch`, `Return`, `Unreachable`. | M2 |
| W3.2 | **`Throw { value, unwind }`, `Call { normal, unwind }`, `Suspend { Await\|Yield\|ChannelRecv\|ChannelSend }`.** The difference between "Java/Python/C#/Kotlin/Swift are analysable" and "they are not." | W3.1 |
| W3.3 | `BinOp`, `Aggregate`, `Closure { body, captures }`, and `place-fact record.ty` into the existing type lattice. | W3.1 |
| W3.4 | **Delete both per-language CFG lowerers** (1,629 LOC) and the substring-matching loop recovery. | W3.1 |
| W3.5 | Fold `ts_value_flows.rs` (11,898 LOC) back onto the shared pipeline, or retire it. `Closure` with explicit captures is why it forked. | W3.3 |

**Exit gate:** no analysis reads source text to recover control flow · both CFG lowerers deleted · **a
throwaway Python frontend lowers to MIR without introducing a single new IR concept.** That last one is
the real test — if Python needs new IR, the IR isn't done.

---

### M4 — One principled engine

The capability payoff. This is where taint becomes real.

| # | Work | Depends on |
|---|---|---|
| W4.1 | Promote `js_points_to` (the real Andersen solver) to primary resolver; add object sensitivity. | M3 |
| W4.2 | **IFDS/IDE solver** over the repaired ICFG. Gives interprocedural taint *and* realizable-path discipline in one construction. | M3 |
| W4.3 | **Taint**: sources → sanitizers → sinks, with replayable paths. Sources already exist (14 typed kinds). | W4.2 |
| W4.4 | Retire the recognizer bank and the reachability filter that masks a 53%-precision resolver as 96%. | W4.1, W4.3 |
| W4.5 | Lift constant propagation and nullability to IDE. The domains are already written and correct. | W4.2 |

**Exit gate:** a taint benchmark exists — 150–200 labelled cases across the 10 shipped security
templates, with explicit FP traps and *distinct* sanitizer names so a fixture cannot pass by
name-matching — and its precision/recall is published and gated. **No unrealizable path can be
reported.** The 10 shipped security templates are measured, not assumed.

---

### M5 — Compound

| # | Work | Depends on |
|---|---|---|
| W5.1 | **Crate split** into ~10–12 crates. Largely falls out once cycles are gone; deletes the need for the `pub(crate)`-everything + out-of-workspace leak-probe contortion. | M2 |
| W5.2 | **Persistent store** + `SummaryKey` persistence — the type is already correctly designed and never written. | **W2.3 (interning)** |
| W5.3 | **Demand-driven queries.** Add the missing `lookup` call; add import-shape and public-signature digests so `ChangeKind::{ImportShape, PublicApiShape}` become reachable. → editor latency. | W5.2 |
| W5.4 | **Shareable rule packs**: exactly-pinned versioning, `--locked`, and subprocess sandboxing **before** distribution, not after. | M2 |
| W5.5 | **Python**, as the proof that M2+M3 worked. | M3 |
| W5.6 | **External-index frontends** (SCIP / LSIF / LSP / `gopls` / `tsc`). Language breadth without writing production frontends. Only possible if W2.6 honoured `AnalysisUnit` + `FrontendProfile`. | W2.6 |
| W5.7 | **Framework models as data.** Promote `.polint/models/*.toml` (already exists, private, undocumented) to a real artifact: routes, DI containers, ORMs, RPC boundaries, serialization. Real code's entrypoints are not `main`. | M4 |

---

### The frontier — required for the goal, not yet plannable

**Cross-language flow.** A TS frontend calling a Go backend over an HTTP contract; taint that crosses
the boundary. The IR hourglass gives per-language uniformity — it does **not** give this. It needs a
contract/boundary model layered on the call graph, and it is genuinely unsolved by every incumbent.

That makes it the clearest white space available, and the right thing to attempt *after* M4 proves the
single-language interprocedural engine works. Listed here so it is not forgotten, deliberately not
scheduled: designing it before there is a working IFDS engine to extend would be guessing.

---

## 4. What to stop, defer, and archive

**Stop now:**
- Adding languages and analyses on the current wiring. Language #3 costs ~19 mandatory edits today and
  ~1 after M2. Every addition makes the rewiring more expensive, superlinearly.
- Merging abstractions without a product-path caller.
- Accuracy work without a recorded cost column.

**Defer:**
- Type-directed call graph → after M4 (build it on IFDS, not on the recognizer bank).
- Store persistence → after interning (§1).
- WASM sandboxing → until rule distribution is real; subprocess is the right first answer.
- Any query DSL. The ADR decided this and the decision is right — **but measure rule-pack cold compile
  time**, which is unmeasured and is the ADR's own stated revisit trigger. Don't let that decision get
  made for you by a number nobody looked at.

**Write one document:** a root `ARCHITECTURE.md` — the provider DAG as the spine, the capability
ladder, the fact-model vocabulary, the honesty contract, the boundary map, and an explicit
**built-but-not-wired register**. `AGENTS.md:67` currently says "Architecture not yet mapped" and
points at a file that does not exist.

**Archive:** `docs/roadmap/*`, both RUST-AUDIT docs, `rust-audit-incoming/`.
**Reconcile (don't archive):** the five research STANDARDs the implementation has silently drifted
from, `research/ROADMAP.md` (all 16 tracks checked, all 22 PRs unchecked, ~15 actually shipped),
`.planning/ROADMAP.md` (still says Phase 65 has "Research flags: none"), and `RETROSPECTIVE.md`
(no Phase 65 entry at all).

**Narrow** the crate-wide `expect(dead_code)` at `analysis/mod.rs:1-6` that covers 112k LOC and hides
genuinely dead code from the compiler.

---

## 5. The scoreboard

One table, updated at every milestone gate. If a number isn't moving, the plan is wrong.

| Metric | Today | M2 | M4 | Fantastic |
|---|---|---|---|---|
| Retained bytes / LOC | ~5.6 KB | < 2 KB | < 1.5 KB | < 1 KB |
| Largest repo analysed | *unknown* | 1M LOC | 5M LOC | 10M LOC |
| Module cycles | 26 | < 5 | 0 | 0 |
| Edits to add a frontend | ~19 | ≤ 2 | ≤ 2 | ≤ 2 |
| `cargo check` warm | 66 s | < 30 s | < 20 s | < 20 s |
| Findings with evidence path | 0% | > 90% | > 90% | 100% |
| Tests asserting accuracy | **0** | ≥ 1 gated | taint gated | every analysis gated |
| Interprocedural taint | none | none | shipped + measured | cross-language |
| Warm re-run after 1-fn edit | full | full | frontier | frontier, interactive |
| Rules shareable across repos | no | no | no | yes, versioned + sandboxed |

---

## 6. Risks

| Risk | Mitigation |
|---|---|
| **Refactor stalls momentum.** | M1 runs in parallel with M2 and every M1 item ships user-visible value. M2 is where discipline is required — and by then the gates make it safe. |
| **Another Phase 65.** | §2 rules are stop conditions, not guidelines. The 1,500-line budget is the single most important one. |
| **Agents break invariants silently during the refactor.** | Precisely what M0 exists for. **Do not start M2 before W0.1 and W0.3 are green.** |
| **Interning touches everything at once.** | One fact family at a time behind newtype IDs that already exist. Go RTA fixpoint first — self-contained, immediate 5–20×, proves the pattern. |
| **IFDS looks like a research project.** | It isn't — it's a well-specified solver with reference implementations (WALA, SootUp, Heros). The real risk is starting it before M3. Don't. |
| **Scope explosion toward "most capable."** | §0 strategy line and [10](10-sota-landscape-and-bar.md)§f. Depth accrues underneath the wedge; it is not the wedge. |

---

## 7. Start here

**M0.A first — the golden corpus and characterization harness.** It is the prerequisite for letting a
fleet of agents touch anything, because it is the only artifact that answers "did we lose a
capability?" in a way that survives the refactor and that an agent cannot quietly rewrite. Build it
before the inversions, not alongside them.

Then, in parallel and with no dependencies between them:

**W0.1** accuracy gate · **W0.3** layering rule (dogfooded) · **W1.2** un-strip evidence ·
**W1.4** kill the O(F²) scans.

After M0: every capability is pinned by a behavioural test with a cost budget, the engine cannot
silently regress on accuracy or architecture, every policy-query finding explains itself, and the
worst algorithmic wart is gone. That is the point at which the structural work becomes safe to
parallelise.
