# Deferred to follow-up PRs

Recorded 2026-08-10 at the final pre-merge review, so these do not become
"we'll fix it later" and then never. Each is a real item with a reason, not a wish.

None of them blocks this merge: the tree is green, the architecture is correct, and the public
contract is intact.

---

## 1. W5.1 — the actual crate split · **highest value**

**State:** not done. `cargo metadata --no-deps` reports four workspace packages
(`polint`, `polint-bench`, `polint-eval`, `polint-macros`). What landed is a *module*
reorganization inside `crates/polint/src/`. `.swarm/T-SPLIT-LAND.md` originally claimed eight
crates and cited a `cargo metadata` check; that claim was false and is now corrected in place.

**What is true:** the layering *directions* are correct — verified zero wrong-direction edges for
`internal_core → analysis*`, `ir → analysis|frontend`, `analysis_neutral → go|ts`.

**What is missing:** compiler enforcement. Modules in one crate cannot produce a cycle error.

**Mitigation shipped instead:** `crates/polint/tests/module_layering.rs` asserts every forbidden
edge and fails the build on a new one. Verified to actually fail — injecting a rule that the tree
violates reported 451 edges with file and line. This captures the enforcement value at a fraction
of the risk.

**Why deferred rather than done tonight:** moving ~250k LOC across eight crate boundaries is the
single largest-churn operation remaining, it would invalidate every gate result already collected,
and the value it adds over the layering guard is compile parallelism and a stronger guarantee —
neither of which is worth doing at speed.

**When it happens:** its own PR, single exclusive worker, per `.swarm/DECISION-2026-08-10-PRE-SHIP.md` §Q4.
The layering guard's rule table is the specification for the crate boundaries — port it directly.

---

## 2. Delivery-history references in shipped source · **cosmetic, but a stated policy violation**

**State:** 306 `D-NN` style references in comments under `crates/polint/src`.

`AGENTS.md` forbids this explicitly: comments must explain enduring behaviour, never plan or
decision identifiers. Their referents live in `.planning/` and are unresolvable to any future
reader — including the author.

**Why deferred:** 306 comment edits touching files across the whole tree, on the night of a merge,
to fix something with zero correctness impact, is precisely the churn that breaks a green build.

**When it happens:** a mechanical follow-up. Each site needs the *reason* written out in domain
terms, not the identifier deleted — deleting the reference without replacing the meaning makes the
comment worse, not better.

One instance was fixed tonight because it sat in a build file rather than source:
`crates/polint/Cargo.toml` cited `63-01-SUMMARY.md "Deviations from Plan"` to justify the
`unsafe_code` policy; it now states the actual reason (platform FFI: process containment,
non-blocking fds, RSS measurement).

---

## 3. Suppression drift · **watch item**

**State:** net +36 `#[allow]`/`#[expect]` versus `main` (171 added, 135 removed); 269 total in
`crates/polint/src`. Composition of the additions: 31 `dead_code`, 20 `too_many_arguments`,
7 `unused_imports`, 6 `unsafe_code`, 1 `unreachable_pub`.

**Assessed tonight:**
- The 6 `unsafe_code` are legitimate and correctly scoped — Windows job objects for process
  containment, non-blocking fds, RSS measurement. Each carries an explicit greppable allow, which
  is the documented policy.
- The 7 `unused_imports` are refactor residue and should simply go.
- The 31 `dead_code` are the ones worth auditing: that lint is the tree's early-warning system for
  the built-but-not-wired pattern that caused this whole re-architecture.

**When it happens:** follow-up sweep. Start with `unused_imports`, then audit each `dead_code`
against the question "is this reachable from the product path?"

---

## 4. W5.2 persistent store · W5.3 demand queries

Already decided out in `.swarm/DECISION-2026-08-10-PRE-SHIP.md` §Q1 and §Q5. W5.2 creates a
versioned on-disk schema — a one-way door that should not land inside a very large PR where it
gets the least careful review. Interning shipping without the store is the correct order: identity
is now compact and correct, so the store will have something worth persisting.

---

## 5. Test-count delta · **verified benign, recorded for completeness**

553 `#[test]` attributes removed versus `main`; suite now 2,193 and fully green.

This tracks the deliberate deletions: `ts_value_flows.rs` (11,898 LOC), the parallel
`calls/js_points_to` Oxc pipeline, recognizer banks, unmatched-BFS paths, and scoring filters —
all removed with their tests, as the no-dual-paths rule required. Not a coverage regression;
deleted code does not need tests.

---

## 6. Wall-clock assertions in the test suite · **flaky gate — fix this early**

**State:** `polint-eval/src/harness/fixtures.rs` asserts `run.metrics.runtime_budget_failed == 0`
inside an ordinary `#[test]`. That is a wall-clock budget check running on shared CI. It failed the
`windows-latest — lib tests` job on PR #97 — a PR whose entire diff is 31 lines of Python that
cannot touch Rust — and passed on rerun.

**Evidence it is chronic, not a one-off:** this branch's CI history is 5 failures across ~14
completed runs, and three separate commits already went into this area — `fix: stabilize Windows
runtime gates`, `fix: resume contained Windows subprocesses`, `fix: handle Windows cache
contention`. The class keeps coming back because the assertion is timing-based on a machine whose
speed is not controlled.

**Why it matters more than a normal flake:** every gate in `ORCHESTRATION.md` assumes a red build
means something is wrong. A gate that fails for reasons unrelated to the change trains both humans
and agents to retry rather than investigate, which is precisely how a real regression gets waved
through. A flaky gate is worse than no gate, because it carries authority it has not earned.

**Fix direction:** a runtime budget belongs in the cost record (W0.A4), where a regression is
reported against a recorded baseline and reviewed, not in a pass/fail unit test. Either move the
budget check out of `cargo test` into the benchmark path, or make it advisory there — record and
warn, never fail. Keep the correctness assertions next to it (`false_negatives`, `forbidden_hits`)
exactly as they are; those are the ones that should stay hard.
