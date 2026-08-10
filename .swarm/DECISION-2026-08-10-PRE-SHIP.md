# Pre-ship decisions — binding

**Date:** 2026-08-10 · **Branch:** `static-analysis-architecture-review` · **Supersedes** the
"start with two, then discuss" framing in `M5-BEFORE-MERGE.md`.

**One PR.** Everything below lands on the integration branch. One PR to `main` at the end, opened by
a human.

---

## The line

> **Structure and identity are migration. Persistence and latency are capability.**

Migration ships in this PR. Capability does not. Every decision below follows from that one sentence.

---

## Ship checklist — binding, ordered

A PR to `main` may be opened when, and only when, all of these are true:

1. **M0–M4 accepted as landed** — verified by a single full gate pass at tip, not by re-reading task markers.
2. **StableKeyId interning complete** — slice C (§Q2). Zero `stable_key: String` fields remain in `crates/polint/src`.
3. **W5.1 crate split complete** — targeted cut set (§Q4). Public import paths byte-stable.
4. **Root `ARCHITECTURE.md` written** — replaces `AGENTS.md:67` "Architecture not yet mapped". Documents the provider DAG, the crate graph, the fact model, the honesty contract.
5. **`AGENTS.md` architecture section updated** to point at it.
6. **Full tip gate suite green in one run on the final commit** (§Q6).
7. **`.swarm/READY-TO-SHIP.md` written** with the checklist state, gate log, and a draft PR body.

**Explicitly NOT required for this ship, even though PLAN.md lists them:**
W5.2 persistent store · W5.3 demand-driven queries · W5.4 shareable rule packs · W5.5 Python ·
W5.6 external-index frontends · W5.7 framework models · solver densification · any LoC,
retained-bytes-per-LOC, or module-cycle numeric target · Rust self-dogfood · cross-language taint.

---

## Q1 — What "migration complete" means

**Decision: items 1–7 above. W5.2 and W5.3 move OUT of this PR.**

This contradicts the "then before merge: W5.2, W5.3" line in `M5-BEFORE-MERGE.md`. Stated plainly so
a human can override it, with the reason:

- **W5.2 creates a versioned on-disk schema. That is a one-way door.** Landing a one-way door inside
  an already-enormous PR, where it will get the least careful review of anything in the diff, is the
  highest-risk move available. It deserves its own PR and its own scrutiny.
- **Neither is needed for "the architecture is correct."** The thesis of the review was: providers
  pluggable, facts in stores, real IR, one principled interprocedural engine, correct identity,
  compiler-enforced layering. That is items 1–3. Persistence and editor latency are things the
  corrected architecture *enables* — they are the first customers of the new structure, not part of it.
- **Interning-before-store was the ordering argument, and it is satisfied.** The reason interning had
  to precede the store was to avoid baking duplicate identity into a persisted schema. Shipping
  interning and *not* the store fully honours that. The store then arrives with a correct thing to persist.

**Re-verify, don't re-litigate.** M0–M4 are accepted on the strength of a green tip gate pass. Do not
reopen landed design decisions.

---

## Q2 — Interning: scope and success bar

**1. Required before ship?** **Yes.** It is the last unmigrated part of the fact model, and it is
strictly cheaper now than after the crate split.

**2. Minimum ship slice: C** — interner + construction site + all fact families + `FactMeta` +
owner maps.

Not A or B: leaving `FactMeta.stable_key` and `stable_key_owners` as `String` keeps 2 of the 3
duplicate copies, so the migration would deliver a third of its point while leaving a permanent dual
path. **"No dual paths" is a locked human rule — an unmigrated family IS a dual path.**

**Not D.** Solver densification (Go RTA, points-to hot loops) is **explicitly out of scope**. It is an
optimization, not a migration, and it is the exact thing that failed. Do not attempt it. Do not gate
on it.

**3. Pass/fail measurement — structural, not performance:**

| Criterion | Gate |
|---|---|
| `rg -c 'stable_key: String' crates/polint/src` | **must be 0** — hard gate |
| Exactly one interner, `AnalysisDb`-scoped, no process-global | hard gate |
| Determinism gate (10 seeded permutations) | **must be green** — hard gate |
| Goldens | **byte-identical** — hard gate |
| Public surface leak | green, allowlist unchanged | hard gate |
| Retained bytes / RSS on the W0.A4 corpus | **measured and recorded. NOT a gate.** |
| Wall-clock on the golden corpus | **fail if worse than −10%** — guard against a repeat of the 1.8× regression |

**Memory being flat is an acceptable ship state.** The prior attempt was blocked by treating a
microbenchmark as the thesis. The thesis is *the identity model is correct and there is one copy of
each key*. Memory is the expected consequence, not the definition of done. Record the number; if it
is flat, ship and note it.

**4. If it stays slow: ship interning-only.** Densification is out of scope, so "stays slow" is not a
ship blocker. The only performance condition that blocks is the −10% wall-clock regression guard.

**Phasing (from `INTERNING-CONTINUE.md`, which is correct):** intern at construction first, then
migrate fact fields family by family, deleting the `String` field in the same PR as each family. No
compat shims.

---

## Q3 — Interning vs crate split sequencing

**1. Strictly serial. Interning first, then the crate split.** No parallelism between them.

Interning is a *type* change threaded through hundreds of call sites; the crate split is a *file
move* plus visibility churn. Doing the type change first means it happens once, inside one crate,
with one `cargo check` surface. Doing the split first means paying the type migration across 8 crate
boundaries with visibility churn on every family.

**2. May the split start on partial interning?** **No.** It starts when interning is complete and
green at tip.

**3. Forbidden intermediate state:** **the crate split must not begin while any dual `String`/`Id`
field exists anywhere in the tree.** This is the "no dual paths" rule applied to sequencing — moving
a half-migrated type across a crate boundary is how you get a permanent shim.

---

## Q4 — Crate split: target shape

**1. Required before ship?** **Yes.** It is what converts layering from convention into a compiler
guarantee — the stated point of the whole re-architecture. It is also strictly cheapest now, right
after M2 cleaned up the module boundaries and before any more code lands.

**2. Goal: option B — a targeted cut set, not the full 12-crate graph.**

The value is compiler-enforced layering. Eight crates deliver every invariant that matters; going to
twelve by splitting analyses into per-analysis crates buys aesthetics and multiplies risk in a PR
that is already enormous.

**Binding cut set:**

| Crate | Contains | May NOT depend on |
|---|---|---|
| `polint-core` | `FileId`, `Span`, `StableKeyId` + interner, `Diagnostic`, `LanguageId` | anything below |
| `polint-ir` | MIR: blocks, terminators, places, types | frontends, analyses |
| `polint-analysis-api` | `trait Provider`, `trait FactStore`, `ProviderManifest`, `CapabilityId` | concrete analyses, frontends |
| `polint-frontend-api` | `trait LanguageFrontend`, `FrontendProfile`, `AnalysisUnit` | concrete frontends |
| `polint-go` | Go frontend + lowering | other frontends, kernel |
| `polint-ts` | TS/JS frontend + lowering | other frontends, kernel |
| `polint-analysis` | all analyses incl. IFDS, points-to, CFG, summaries | any concrete frontend |
| `polint` | facade, kernel, sdk, runner, cli, host composition root | — |

The invariants the compiler must now enforce: **kernel cannot name a language · core cannot name a
fact · analyses cannot name a frontend · one composition root.**

**3. Public contract: byte-stable. Non-negotiable.** `polint::sdk`, `polint::sdk::prelude`,
`polint::runner`, `polint::rule` keep their exact import paths via facade re-exports. All 17 example
rule packs must compile **unchanged**. The public-surface leak gate must pass with the **same
allowlist count**. If a public path would move, stop — that is a product break, not a refactor.

**4. CI time:** accept a longer CI during the split. No crate cap; the cut set above is the cap.

---

## Q5 — W5.2 persistent store and W5.3 demand queries

**Both: explicit follow-up PRs after this ships to `main`.** See Q1 for the reasoning.

**Is interning without disk persistence acceptable?** **Yes — it is the correct order.** Interning
makes identity correct and compact; persistence then has something worth writing. The reverse would
have been the mistake.

**Minimum honest product path when they do land** (recorded now so the follow-up has a starting
point, not to be built in this PR): W5.2 = `SummaryKey`-keyed function summaries only, not full fact
persistence. W5.3 = demand queries for `FunctionSummary` and `FunctionCfg` only. Do not scope-creep
either into "persist everything."

---

## Q6 — Gate and verification policy

**1. Mandatory on tip before the PR is opened — one run, one commit, all green:**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo test -p polint --test public_surface_leak --locked
cargo test -p polint --test golden --locked
cargo test -p polint --lib eval::determinism_gate --locked
cargo test -p polint polyglot --lib --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo deny check --all-features
```

Log the full output to `.swarm/gate-logs/FINAL-TIP-<sha>.log`. A gate pass on an earlier commit does
not count.

**2. Golden timing budget breach with identical diagnostics: retry once; if diagnostics are still
identical, PASS and record the delta.** Timing is **not** a ship gate for this PR. Rationale: the
human waived metric theater, CI timing is noisy, and diagnostics-identical is the real signal. Do
**not** regenerate cost baselines to make a number go green — record the delta honestly.

The one exception is the interning wall-clock guard in Q2: a sustained >10% regression on the golden
corpus is a real signal and blocks.

**3. Golden *diagnostic set* changes: forbidden for both remaining tasks.** Interning and the crate
split are pure structure. A changed diagnostic set means something broke. **ESCALATE — no golden lock
will be granted for either task.**

---

## Q7 — Process for the remaining work

**1. Model:**
- **Interning:** swarm, **max 2 concurrent workers**, `fact_family:*` locks. Families are independent;
  the construction-site change is not.
- **Crate split:** **single worker, exclusive.** No other task runs during it. A crate split cannot be
  parallelised — every worker would conflict on every file move.

**2. Locks:** `fact_family:<name>` during interning. `golden` is **not grantable** to either task
(Q6.3). During the crate split the orchestrator holds an implicit exclusive lock on the whole tree.

**3. Auto-open the PR? NO. Stop and wait.**

When the checklist is green: write `.swarm/READY-TO-SHIP.md` containing the checklist state, the
final tip gate log path, the crate graph as landed, the recorded memory/wall-clock numbers, and a
draft PR body. Then **halt and report**. A human opens the PR. The orchestrator never touches `main`.

---

## Q8 — Out of scope (confirmed, no exceptions)

| Item | Out? |
|---|---|
| W5.4 shareable rule packs | **Yes — out** |
| W5.5 Python frontend | **Yes — out** |
| W5.6 external-index frontends | **Yes — out** |
| W5.7 framework models productization | **Yes — out** |
| Rust language frontend / self-dogfood | **Yes — out** |
| Cross-language contract taint | **Yes — out** |
| W5.2 persistent store | **Yes — out** (moved out; Q1) |
| W5.3 demand-driven queries | **Yes — out** (moved out; Q1) |
| Solver densification (Go RTA / points-to) | **Yes — out** (Q2) |

No exceptions.

---

## Execution order

| # | Task | Depends on | Workers | Notes |
|---|---|---|---|---|
| **1** | **T-VERIFY** — full tip gate pass on current HEAD | — | 1 | Baseline. If red, fix before anything else. Nothing dispatches until green. |
| **2** | **T-INTERN-A** — interner on `AnalysisDb`; `stable_key_from_parts` returns `StableKeyId`; resolve at display/sort/digest boundaries | T-VERIFY | 1 | No family migration yet. Goldens byte-identical. |
| **3** | **T-INTERN-B** — migrate fact families to `StableKeyId`, one family per PR, deleting the `String` field in the same PR | T-INTERN-A | ≤2, `fact_family` locks | symbols → refs → MIR → calls → CFG → rest |
| **4** | **T-INTERN-C** — `FactMeta.stable_key` + `stable_key_owners`; preserve conflict-detection semantics and user-visible conflict text exactly | T-INTERN-B | 1 | Gate: `stable_key: String` count = 0 |
| **5** | **T-SPLIT** — crate split to the Q4 cut set | T-INTERN-C green at tip | **1, exclusive** | Public paths byte-stable; examples compile unchanged; leak allowlist unchanged |
| **6** | **T-ARCH-DOC** — root `ARCHITECTURE.md` + `AGENTS.md` update | T-SPLIT | 1 | Documents what landed, not what was planned |
| **7** | **T-SHIP-PREP** — final tip gate run, `.swarm/READY-TO-SHIP.md`, draft PR body, **halt** | T-ARCH-DOC | 1 | Never opens the PR |

---

## Escalate-only conditions

Stop and ask a human. Do not improvise, do not work around:

- Any golden **diagnostic set** change during interning or the split.
- The public-surface leak gate needing an allowlist change during the split.
- An example rule pack failing to compile after the split.
- A public import path that cannot be kept stable.
- Interning wall-clock regression >10% on the golden corpus, sustained after one retry.
- Determinism gate red at any point.
- A fact family whose `stable_key` is compared by prefix/substring rather than equality — that is an
  undocumented invariant on key format and needs a decision.
- Any temptation to add a compat shim, dual field, or `#[allow]` to get past a gate.
- The hold queue reaching 3 open items.
- Anything that would require touching `main`.
