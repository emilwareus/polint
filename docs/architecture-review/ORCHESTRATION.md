# Orchestration — Running the Swarm

**Audience:** the orchestrator agent. Workers read [HANDOFF.md](HANDOFF.md) and their spec; they do
not read this file.

**Model:** one orchestrator, N workers. No human in the merge loop. Humans drain the hold queue
asynchronously and never block progress.

---

## 1. Roles

**Orchestrator** — owns the task graph, dependency resolution, lock arbitration, gate verification,
merge decisions, and holds. Writes no product code. Never implements a task itself, never
"fixes up" a worker's PR, never resolves an escalation.

**Worker** — claims exactly one task, works in an isolated git worktree, produces one PR-sized branch,
reports `DONE` or `ESCALATE`. Never merges. Never claims a second task while holding one. Never talks
to another worker.

**The orchestrator's most important property is that it does not improvise.** Every decision it makes
is a table lookup in this document. If a situation is not covered here, hold the task and move on.

---

## 2. State

Orchestrator maintains `.swarm/state.json` (gitignored):

```json
{
  "tasks": {
    "W0.A1": {"state": "MERGED",  "branch": "swarm/W0.A1", "attempts": 1},
    "W0.A2": {"state": "CLAIMED", "worker": "w3", "branch": "swarm/W0.A2", "attempts": 1},
    "W1.5":  {"state": "HELD", "reason": "step-1 measurement: parse cost 4% of run"}
  },
  "locks": {"golden": null, "fact_family": {}},
  "milestone": "M0"
}
```

**States:** `BLOCKED` (deps unmet) → `READY` → `CLAIMED` → `IN_REVIEW` → `MERGED` · or → `HELD`.

Append-only human-readable log at `.swarm/blocked.md` for every hold.

---

## 3. The task graph

**Parallel width** = how many workers can run concurrently at that point.

### M0 — Safety net

```
  ┌─ W0.1  accuracy gate      ─┐   no deps
  ├─ W0.2  cost columns       ─┤   no deps
  ├─ W0.3  layering rule      ─┤   no deps
  └─ W0.A1 golden corpus      ─┘   no deps          ← parallel width 4
              │
       ┌──────┴──────┬──────────────┐
       ▼             ▼              ▼
   W0.A2 harness  W0.A3 matrix   W0.4 scale run     ← parallel width 3
       │
    ┌──┴───┐
    ▼      ▼
 W0.A4  W0.A5                       W0.5 mem metric ← parallel width 3
 cost   accept-discipline           (needs W0.4)
```

**M0 GATE — all of `W0.1 W0.2 W0.3 W0.4 W0.5 W0.A1 W0.A2 W0.A3 W0.A4 W0.A5` MERGED and §5 green.**
Nothing in M1 or M2 starts before this. No exceptions.

### M1 — Cheap wins · parallel width 8

`W1.1 W1.2 W1.3 W1.4 W1.6 W1.7 W1.8 W1.9` — mutually independent, all deps satisfied by the M0 gate.
`W1.5` also ready (needs W0.A4, satisfied), but **has a mandatory measure-first checkpoint** (§7).

⚠️ `W1.1`, `W1.2`, `W1.3` are **behaviour-changing** and each needs the **golden lock** (§4) for its
regeneration PR. Their implementation PRs run in parallel; only the regeneration serializes.

**M1 GATE — all merged, §5 green, and: >90% of policy-query findings carry an evidence path;
`polint check --format ai-friendly` lists zero-finding rules.**

### M2 — Break the monolith

```
  W2.1 evict eval ──┐                    ← parallel width 2
  W2.2 split core ──┤
                    ▼
        ┌───────────┴───────────┐
        ▼                       ▼
   W2.3 interning         W2.4 provider   ← parallel width 2
        │                       │
        │              ┌────────┴────────┐
        │              ▼                 ▼
        │         W2.5 FactStore    W2.6 frontend  ← parallel width 2
        └──────── ⚠ fact-family lock ────┘
```

⚠️ `W2.3` and `W2.5` **must never hold the same fact family concurrently** (§4).

**M2 GATE — all merged, §5 green, and: `AnalysisDb` ≤ 30 fields; retained bytes/LOC < 2 KB (from
W0.A4 records); module cycles < 5; the W2.6 acceptance test shows ≤ 2 edits to add a frontend.**

### M3 / M4 — NOT DISPATCHABLE

The orchestrator **must not** create tasks for M3 or M4. They have no specs, by design. On reaching
the M2 gate, the orchestrator halts and writes `.swarm/M2-COMPLETE.md`. A human resumes.

---

## 4. Locks

Two resources serialize work. The orchestrator grants them; a worker never takes one itself.

**`golden`** — held by any PR that regenerates golden files. Exactly one at a time. A worker whose
task will change golden output requests it before opening the regeneration PR; if unavailable the
worker parks (does not proceed, does not pick up other work).

**`fact_family:<name>`** — held per fact family by `W2.3` (interning) and `W2.5` (store migration).
Both migrate families one at a time; concurrent work on the same family produces unresolvable
conflicts. Grant per family, never globally.

---

## 5. Gates — the exact commands

Every one is exit-code checked. **The orchestrator runs these; a worker's claim that they pass is not
evidence.** Verified against this repo's CI (`.github/workflows/ci.yml`) and `Makefile`.

```bash
# G1  format
cargo fmt --all -- --check

# G2  lint
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# G3  full test suite
cargo test --workspace --all-features --locked

# G4  public surface contract
cargo test -p polint --test public_surface_leak --locked

# G5  determinism (10 seeded permutations)
cargo test -p polint --lib eval::determinism_gate --locked

# G6  polyglot canary
cargo test -p polint polyglot --lib --locked

# G7  golden characterization        ← does NOT exist until W0.A2 merges
cargo test -p polint --test golden --locked

# G8  docs
cargo doc --workspace --all-features --no-deps --locked   # RUSTDOCFLAGS="-D warnings"

# G9  supply chain
cargo deny check --all-features
```

`make check` runs lint + test + doc + install-smoke + deny and is a reasonable pre-flight.

### Mechanical rule checks — run on the diff, not the tree

```bash
# R1  PR budget  (HANDOFF §2.1)
git diff --stat origin/main...HEAD | tail -1     # ≤ 1500 insertions+deletions, ≤ 25 files

# R2  no new suppressions  (HANDOFF §2.4)
git diff origin/main...HEAD | grep -E '^\+.*#!?\[(allow|expect)\(' && echo FAIL

# R3  no golden edits outside a sanctioned regeneration PR  (HANDOFF §2.3)
git diff --name-only origin/main...HEAD | grep -q 'tests/golden/' && echo "REQUIRES GOLDEN LOCK"

# R4  no deleted or ignored tests  (HANDOFF §2.5)
git diff origin/main...HEAD | grep -E '^\+.*#\[ignore\]' && echo FAIL
git diff origin/main...HEAD | grep -E '^-.*#\[test\]' && echo REVIEW

# R5  no delivery-history references in source  (HANDOFF §2.9)
git diff origin/main...HEAD | grep -E '^\+.*(Phase [0-9]|D-[0-9]{2}|CR-[0-9]{2}|FINDING [0-9])' && echo FAIL

# R6  structure-or-behaviour, not both  (HANDOFF §2.2)
# If R3 fires AND the diff touches >3 non-test source files → FAIL, tell the worker to split.
```

### R7 — the wiring check (HANDOFF §3), automated

This is the one that stops the built-not-wired pattern, and it is a **mutation test the orchestrator
runs**, not a promise the worker makes.

If the diff adds a `trait`, or a `pub(crate) struct`/`enum` in a new module:

1. Find the call site the worker declared in its PR description.
2. Comment it out.
3. Run `G3`.
4. **A test must fail.** If everything still passes, the abstraction is not wired → **REJECT** with
   reason `unwired-abstraction`. Do not hold; this is a fixable worker error (§6).
5. Restore.

---

## 6. Orchestrator decision table

Worker reports, or a gate fails. Look it up. Do not reason beyond the table.

| Situation | Action |
|---|---|
| All gates + R1–R7 green | **MERGE**. Release locks. Recompute `READY` set. Dispatch. |
| G1/G2/G8 fail (fmt, lint, doc) | **RETURN to same worker**, verbatim compiler output. Max 2 returns, then hold. |
| G3/G6 fail (tests) | **RETURN to same worker** once. If the second attempt also fails → **HOLD**. |
| **G4 fails (public surface)** | **HOLD immediately.** Product contract. Never auto-retry. |
| **G5 fails (determinism)** | **HOLD immediately.** Never retry, never weaken the gate. |
| **G7 fails (golden) on a structural task** | **HOLD.** A refactor changed behaviour — that is the exact thing the harness exists to catch. |
| G7 fails on a sanctioned regeneration PR | Expected. Verify the diff is **added fields only**, or that the *set* of diagnostics is unchanged. If the set changed → **HOLD**. |
| R1 fails (over budget) | **RETURN** with instruction to split into ≤ 1500-line PRs. Not a hold. |
| R2/R4/R5 fail | **RETURN** once with the offending lines. Second occurrence → hold. |
| R6 fails | **RETURN** with instruction to split structure from behaviour. |
| R7 fails | **RETURN** with `unwired-abstraction` and HANDOFF §3. Max 1 return, then hold. |
| Worker reports `ESCALATE` | **HOLD.** Never reassign. See §7. |
| Worker silent / exceeds wall-clock | Kill, release locks, reset task to `READY`, `attempts += 1`. At `attempts == 3` → hold. |
| Two workers conflict on merge | Second worker rebases and re-runs all gates. If conflict is semantic, not textual → hold both. |

**Merge order is arbitrary among green PRs. Always re-run the full gate set after rebase — never
merge on stale results.**

---

## 7. Escalation = hold, never reassignment

Every `Escalate if` clause in a spec marks a case where **the correct answer requires information the
swarm does not have.** A different worker will not do better; it will produce a confident wrong
answer. That is precisely how this codebase acquired a store with one table and 2,027 lines of
unreferenced slicing code.

On `ESCALATE`, the orchestrator:

1. Sets `HELD` with the worker's verbatim reason.
2. Abandons the branch (keeps it, does not merge, does not delete).
3. Releases locks.
4. Appends to `.swarm/blocked.md`: task, reason, branch, spec section, timestamp.
5. **Marks every dependent task `BLOCKED`** and continues with independent work.
6. **Never retries. Never reassigns. Never resolves it.**

Three escalations are *expected outcomes*, not failures — the spec anticipates them:

- **W1.5 step 1** — if measured parse cost is a small fraction of the run, the correct result is
  "don't do this task." Hold with `not-worth-doing` and mark it `MERGED-NOOP`; dependents proceed.
- **W2.3 step 1** — if the Go RTA conversion shows no measurable win, the memory model is wrong.
  Hold the whole of W2.3. **Do not let W2.5 proceed on the assumption interning is coming.**
- **W2.4 step 4** — if topological sort ≠ declared manifest order, hold. Do not edit the
  expected value, do not special-case the sort, do not "fix" the manifest data.

If the hold queue reaches **3 open items**, the orchestrator **stops dispatching new work** and
writes `.swarm/HALT.md`. A swarm accumulating unresolved blockers is producing debt, not progress.

---

## 8. Worker dispatch contract

Each worker gets exactly this, and nothing else:

```
TASK:        <id>
SPEC:        docs/architecture-review/specs/<file>.md   (or HANDOFF §4 row for table-only tasks)
RULES:       docs/architecture-review/HANDOFF.md        (read §1 errata, §2 rules, §3 wiring — all binding)
BRANCH:      swarm/<id>            (worktree, branched from origin/main)
LOCKS HELD:  <golden | fact_family:<name> | none>

You own one task. Read the spec end-to-end before writing code.
Design decisions (D1, D2, …) are binding — if you disagree, report ESCALATE. Do not deviate.

Report exactly one of:
  DONE <branch>  — plus, in the PR description:
                     • what changed
                     • which gates you ran and their results
                     • what you deliberately left alone
                     • if you added a trait/type: the product-path call site, and which test
                       fails if that call site is removed
  ESCALATE <reason> — for any "Escalate if" trigger, or any situation the spec does not cover

Do NOT: merge · touch files outside the spec's scope · update a golden or expected value ·
add #[allow] · delete or ignore a test · widen visibility · start a second task ·
resolve your own escalation.
```

---

## 9. Preflight — before the first worker

The orchestrator runs once, and halts on any failure:

```bash
git fetch origin && git rev-parse origin/main
cargo build --workspace --locked          # tree builds
cargo fmt --all -- --check                # G1 clean at HEAD
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings   # G2 clean
cargo test --workspace --all-features --locked                                  # G3 green
cargo test -p polint --test public_surface_leak --locked                        # G4 green
cargo test -p polint --lib eval::determinism_gate --locked                      # G5 green
```

**If the baseline is not green, stop.** You cannot attribute a failure to a worker when HEAD was
already broken. Record the baseline durations of G3 — they become the regression reference until
W0.A4 lands.

G7 (golden) does not exist yet. **W0.A1 → W0.A2 create it. Until W0.A2 merges, no structural task may
be dispatched** — that is the whole reason M0 precedes everything.
