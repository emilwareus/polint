# Handoff — Rules of Engagement for Implementing Agents

**Read this before touching anything.** It supersedes the other documents where they conflict.

*Orchestrator? You want [ORCHESTRATION.md](ORCHESTRATION.md). This file is for workers.*

The review documents (`01`–`10`) are *analysis*. [PLAN.md](PLAN.md) is *sequence*. This document is
*execution*: what is safe to do, what is not, and how you will be judged.

---

## 1. Errata — where the review documents are wrong

The review was produced by multiple agents working in parallel. Several numbers disagree between
documents. **This table is authoritative. Re-verified directly against the tree.**

| Claim | Wrong value (and where) | **Correct value** | Verify with |
|---|---|---|---|
| `AnalysisDb` field count | 133 (`01`), 167 (early notes) | **132** | `awk 'NR>=658 && NR<=825' crates/polint/src/core/mod.rs \| grep -cE '^    [a-z_]+:'` |
| `PROVIDER_MANIFESTS` entries | 25 (`02`) | **23** | `rg -c '^\s+ProviderManifest \{' crates/polint/src/analysis_kernel/provider.rs` |
| `stable_key: String` fields | 207 (`06`) | **229** | `rg -c 'stable_key: String' crates/polint/src --glob '*.rs' \| awk -F: '{s+=$2} END {print s}'` |
| `Language::` references | 999 (`01`, `02`) | **1,016** across **129** files | `rg -o 'Language::' crates/polint/src --glob '*.rs' \| wc -l` |
| `analysis` ↔ `analysis_kernel` cycle | — | **195 / 182** (confirmed correct) | `rg -c 'crate::analysis_kernel::' crates/polint/src/analysis` |

**Confirmed correct, do not re-litigate:** `AnalysisKernel::run` spans lines 92–968 (877 lines);
17 trait declarations crate-wide; exactly 4 `rayon` call sites, none under `analysis/`;
`analysis/slicing/` has zero references outside itself; evidence is stripped at
`diagnostics/mod.rs:1136-1140`; the SQLite store contains exactly one table
(`_polint_schema_migrations`); 2,429 inline tests vs 174 integration tests.

**Numbers you must treat as estimates, not facts** — they are labelled as such in `06` and must never
be quoted as measurements: `~5.6 KB retained per LOC`, `~8 facts/LOC`, `~700 B/fact`, the 1M/10M LOC
projections, and the "up to 12 serial full-corpus re-parses" count. **W0.A4 exists to replace these
with real measurements. Until it lands, do not make decisions that depend on their precision.**

**Two review claims are imprecise and are corrected by their specs. The spec wins:**

| Review says | Actually | Corrected in |
|---|---|---|
| "12 oxc parse sites drop errors" (`02`) | `ts/adapter.rs:489-523` handles them **correctly**. The ~9 *secondary* sites check only `panicked && body.is_empty()`, so a **recoverable** error yields a silently partial AST. | [`specs/W1.1`](specs/W1.1-parse-error-honesty.md) |
| "polint holds ASTs for all files" (early notes) | **False.** Every `Allocator` is function-local and dropped at function exit, defended by regression tests. Peak AST memory is bounded. Do not "fix" this. | [`specs/W1.5`](specs/W1.5-parse-cache.md) |

**One review claim was verified as more useful than stated:** the 23 `PROVIDER_MANIFESTS` entries are
in **exactly** the execution order of the 23 `provider_output_for` calls in `run()`. This turns W2.4
from a redesign into a verification exercise — see [`specs/W2.4`](specs/W2.4-provider-trait-and-scheduler.md).

---

## 2. Hard rules

Violating any of these fails review regardless of whether the code works.

1. **≤ 1,500 changed lines, ≤ 25 files per PR.** This is a stop condition. If your task exceeds it,
   stop and split — do not finish first and split after.
2. **A PR changes structure OR behaviour, never both.** If a refactor changes any golden output, you
   have done two things; split them.
3. **Never update a golden file, expected value, or baseline to make a test pass.** If a test goes
   red, the default assumption is that *your change* is wrong. Updating an expectation requires a
   separate PR whose description states which behaviour changed and why it is intended.
4. **Never add `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` to silence a lint.** Fix the
   cause. If you believe an exception is genuinely warranted, report ESCALATE — do not merge it.
5. **Never delete or `#[ignore]` a failing test.** Report ESCALATE instead.
6. **Never widen visibility** (`pub(crate)` → `pub`) to make something reachable. The public surface is
   guarded by `crates/polint/tests/public_surface_leak.rs` and is a product contract.
7. **No new abstraction without a caller.** See §3 — this is the single most important rule.
8. **Do not touch files outside your task's declared scope.** No opportunistic cleanup, no drive-by
   renames, no reformatting adjacent code.
9. **Comments explain enduring behaviour.** Never write phase numbers, plan IDs, task IDs, or
   references like "D-07" / "FINDING 7" into source, tests, or fixtures.

---

## 3. The one pattern that must not repeat

This codebase's dominant defect is **abstractions that were built and never connected**: a 23-provider
manifest table nothing schedules from, 11,500 LOC of query-key machinery no query calls, a database
with one table, 4,335 lines of evidence code stripped before any user sees it, `analysis/slicing/`
with zero references.

**This is exactly the output a well-meaning agent produces**: the trait compiles, the tests pass, the
task looks complete, and nothing uses it.

**Therefore, a task producing a new trait, type, or module is not complete until all three are true:**

1. Something on the **product path** calls it — reachable from `AnalysisKernel::run` or the CLI, not
   only from `eval/` or `#[cfg(test)]`.
2. A test **fails if the caller stops calling it**. Not a test that the thing works in isolation — a
   test that proves it is wired.
3. The **old code path it replaces is deleted** in the same PR. Not deprecated, not left behind a
   flag. Deleted.

If you cannot satisfy all three, your task was scoped wrong. Report ESCALATE.

**Self-check before opening any PR:** *"If I deleted the thing I just built, which test goes red?"*
If the answer is "none," you are not done.

---

## 4. What you may and may not work on

Not every item in PLAN.md is safe to hand to an implementing agent. Three tiers.

### 🟢 GREEN — mechanical, fully specified, execute now

Clear inputs, clear acceptance, no design judgment required.

| Task | Scope | Done when |
|---|---|---|
| **W0.A1–A5** Golden corpus + harness | New test infrastructure only | See §5 — spec'd in full |
| **W0.1** Accuracy gate | `crates/polint/src/eval/external/mod.rs:27-29` + baseline JSON | Benchmark fails CI on an F1 drop; skipping is loud, not silent |
| **W0.3** Layering rule | New rule in `.polint/rules/` + CI wiring | CI fails when a new wrong-direction module edge is introduced |
| **W1.1** Parse-error honesty | See [`specs/W1.1`](specs/W1.1-parse-error-honesty.md) — doc 02's "12 sites drop errors" is imprecise | Recoverable syntax errors record an `unsupported` fact instead of silently analysing a partial AST |
| **W1.4** O(F²) scans | `ts/adapter.rs:337-341`, `go/adapter.rs:298-302`, `cfg/lower_ts.rs:352` | Linear-scan `.find()` replaced by an index; **golden output byte-identical**; timing improves on the corpus |
| **W1.6** Gate `validate_fact_metadata` | `analysis_kernel/mod.rs:942` | Validators run under a flag/`debug_assertions`, not on every production run; golden output unchanged |
| **W1.7** Bound source reads | `fs/mod.rs:135` → existing `repo_fs` bounded helper | Oversized file yields a `polint/capability` diagnostic instead of being read whole |
| **W1.8** `#[non_exhaustive]` | SDK prelude types + `Language` | Added; leak-gate test updated; nothing else changes |
| **W1.9** Agent-surface hygiene | `cli/skill.rs`, `.claude/skills/`, `.agents/skills/`, fixture dir names | Byte-equality test between generated and checked-in `SKILL.md` passes |
| **W2.1** Evict `eval/` | Move 29,344 LOC to a dev-only crate | `cargo build --release` no longer compiles it; all tests still pass |
| **W2.2** Split `core/mod.rs` | 11,143 lines → files under `core/` | **Zero API change. Zero behaviour change. Pure file moves.** Golden output byte-identical |

### 🟢 GREEN — specified in `specs/`, execute from the spec

These were YELLOW until their design specs were written. **The specs now exist and are binding.**
Read the spec end-to-end before writing code; each carries design decisions, ordered PR-sized steps,
exact acceptance commands, anti-goals, and escalation triggers.

| Spec | Covers |
|---|---|
| [`specs/W1.1-parse-error-honesty.md`](specs/W1.1-parse-error-honesty.md) | The correctness bug. Note: it **corrects** doc 02's framing — read the spec, not the doc |
| [`specs/W1.2-ship-evidence.md`](specs/W1.2-ship-evidence.md) | Make provenance reach users |
| [`specs/W1.3-rule-telemetry.md`](specs/W1.3-rule-telemetry.md) | Make silent rules diagnosable |
| [`specs/W1.5-parse-cache.md`](specs/W1.5-parse-cache.md) | Stop re-parsing. **Has a mandatory measure-first checkpoint** |
| [`specs/W2.3-interning.md`](specs/W2.3-interning.md) | `StableKeyId`. Go RTA fixpoint proves it first |
| [`specs/W2.4-provider-trait-and-scheduler.md`](specs/W2.4-provider-trait-and-scheduler.md) | Execute the manifest DAG |
| [`specs/W2.5-fact-store.md`](specs/W2.5-fact-store.md) | Decompose `AnalysisDb` |
| [`specs/W2.6-language-frontend.md`](specs/W2.6-language-frontend.md) | Open language registry |

**Where a spec and a review document disagree, the spec wins.** The specs were written against the
tree after the reviews and correct several of their imprecisions.

**Do not implement any of these from PLAN.md alone.** PLAN.md gives one sentence per item — a
direction, not a specification. The spec is the contract.

### 🔴 RED — do not delegate to implementing agents at all

**M3 (real IR) and M4 (IFDS/taint).**

These are genuine design problems. The terminator set (`Throw`, `Call { unwind }`, `Suspend`,
`Closure { captures }`), the abstract domain, the summary representation, and the IFDS flow-function
design each require analysis expertise and each is a one-way door — a half-right IR is worse than
none, because analyses will fork around it exactly as `ts_value_flows.rs` (11,898 LOC) already did.

Attempting these from the plan text will produce confident, plausible, unusable code. They are blocked
on M0–M2 anyway.

---

## 5. W0.A — the golden harness, specified in full

This is the first task and everything else depends on it. It gets a full spec because getting it
wrong invalidates every later verification.

**Purpose:** answer one question, forever, cheaply — *given this repository, does polint still find
exactly the same things, in the same time, using the same memory?*

### Inputs
- The 17 rule packs under `examples/*/.polint/rules/`
- The 27 fixture trees under `tests/eval-fixtures/`
- The 3 scale repositories declared in `research/evaluation-harness/suites/` — **pinned by commit
  SHA**, cloned by a `make` target, never floating

### Output shape
For each `(target × rule pack × format)`: run the **real CLI binary** — not an internal API — and
commit the normalized result as a golden file.

**Normalization is the load-bearing part.** Before comparison, you must:
- Sort diagnostics by `stable_fingerprint` (it already exists on `Diagnostic`)
- Replace absolute paths with repo-relative ones
- Strip or fix: timestamps, durations, polint version, machine name, thread counts, temp dirs
- Serialize with stable key ordering

**A golden suite whose failures are unreadable gets disabled within a month.** Invest in the diff
output: on failure, print added/removed diagnostics as a set difference, not a raw text diff.

### The capability matrix (W0.A3) — distinct from the goldens, do not merge them
One fixture per `(fact view × language)` across the full SDK prelude, asserting the view returns
non-empty, well-formed data. Goldens catch *output changes*; the matrix catches *capability loss*. A
refactor that silently drops Go symbol resolution produces a golden diff that looks like a plausible
output change — the matrix makes it unambiguous.

### Cost record (W0.A4)
Wall-clock and peak RSS per case, committed beside the output, with per-case budgets.
`crates/polint/src/eval/bench/measure.rs` already has RSS instrumentation — wire it, do not rewrite it.
Budgets should be generous initially (fail on > 20% regression); tighten once the numbers are stable.

### Acceptance
- `cargo test -p polint --test golden --locked` passes from a clean checkout
- Deliberately breaking one analysis produces a **readable** failure naming the lost diagnostics
- Regenerating requires an env flag CI never sets (W0.A5)
- Every prelude fact view has ≥ 1 capability-matrix fixture per language claiming support

### Explicitly NOT in scope
Do not fix any bug you discover while building this. Do not "improve" any output that looks wrong.
**Record current behaviour as-is, including behaviour you believe is incorrect** — that is what
characterization means. File findings separately; fixing them is a later, separate PR that will show
up as an intentional golden change.

---

## 6. Escalate instead of guessing

Report `ESCALATE` to the orchestrator when any of these is true. Guessing costs more than stopping.
The orchestrator quarantines the task and continues with independent work — you are not blocking anyone.

- A task requires a design decision not written down anywhere
- A golden file or baseline appears to need updating
- A test fails and you cannot explain why in one sentence
- Your change would exceed the PR budget
- You need to add an `#[allow]`, widen visibility, or delete a test
- The review documents contradict each other on something load-bearing and §1 does not resolve it
- The task is 🟡 YELLOW and no design spec exists
- You believe the plan is wrong

**The last one is not insubordination — it is the most valuable thing you can report.** The plan was
written from a snapshot at commit `1263208a`. If the tree has moved, say so.

---

## 7. Definition of done

Every PR, without exception:

- [ ] Within budget: ≤ 1,500 lines, ≤ 25 files
- [ ] Structure **or** behaviour, not both
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] Full test suite passes; **no golden file modified** (or a separate PR justifying it in prose)
- [ ] No new `#[allow]`, no widened visibility, no deleted or ignored tests
- [ ] If it added an abstraction: a product-path caller exists, a test fails without it, and the
      replaced path is deleted
- [ ] No phase numbers or plan IDs in source, comments, tests, or fixtures
- [ ] PR description states what changed, what was verified, and what was deliberately left alone
