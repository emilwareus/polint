# M5 before merge — what each part is

> **SUPERSEDED IN PART — see [`.swarm/DECISION-2026-08-10-PRE-SHIP.md`](../../.swarm/DECISION-2026-08-10-PRE-SHIP.md).**
> The "start with two, then W5.2/W5.3 before merge" framing below is **no longer binding**.
> Binding scope for this PR: **interning + W5.1 crate split + ARCHITECTURE.md**.
> **W5.2 and W5.3 are now explicit follow-up PRs after this ships** — W5.2 creates a versioned
> on-disk schema (a one-way door) and must not land inside this PR. The rest of this document
> remains accurate as an explanation of what each item *is*.

**Branch:** `static-analysis-architecture-review`  
**Rule:** complete migration before ship to `main`.  
**Out of this PR (do not plan here):** shareable rule packs as a product, Python language, SCIP/LSIF/LSP frontends, framework-models productization.

This note explains **every M5 row in PLAN.md** in plain language, then states what we treat as **pre-merge migration** vs **not this PR**.

---

## The seven M5 items (plain English)

### W5.1 — Crate split
**What:** Today almost all analysis lives in one giant Rust crate. Split it into ~10–12 crates (frontend API, kernel, Go, TS, analysis pieces, etc.) so dependencies can’t form illegal cycles and compile/API boundaries match the architecture.

**Why it matters:** Enforces layering by the compiler, not by hope. Removes the need for awkward `pub(crate)` + out-of-workspace leak-probe contortions.

**Before merge?** **Yes — structural migration.** First of the two we start with.

---

### W5.2 — Persistent store
**What:** Analysis already has types for saving summaries/facts across runs (`SummaryKey`, SQLite-shaped store). Almost nothing is actually written to disk today. Wire real persistence so a second run can reuse work.

**Depends on:** StableKeyId interning (identity must be stable and compact on disk).

**Before merge?** **Yes — after interning.** Second wave, not the first two.

---

### W5.3 — Demand-driven queries
**What:** Instead of always running the whole provider pipeline, answer “only what was asked” (e.g. editor hovering one symbol). Needs the store + digests so we know what is still valid.

**Depends on:** W5.2.

**Before merge?** **Yes — after W5.2.** Same wave as “make incrementality real,” not day-one.

---

### W5.4 — Shareable rule packs
**What:** Version-pinned, lockfile-style distribution of rule packs + subprocess sandbox so teams can publish/consume packs safely.

**Before merge?** **No.** Net-new product surface.

---

### W5.5 — Python language
**What:** Add Python as a frontend to prove the IR/frontend registry.

**Before merge?** **No.** Net-new language.

---

### W5.6 — External-index frontends
**What:** Read SCIP/LSIF/LSP/`gopls`/`tsc` indexes instead of (or in addition to) parsing ourselves.

**Before merge?** **No.** Net-new integration surface.

---

### W5.7 — Framework models as data
**What:** Promote private `.polint/models/*.toml` into a real, documented artifact (routes, DI, ORM, RPC).

**Before merge?** **No.** Net-new product artifact.

---

## Also required for “migration complete” (not numbered M5, but blocks W5.2)

### StableKeyId interning (continue)
See [`INTERNING-CONTINUE.md`](INTERNING-CONTINUE.md). Best identity model; previous RTA-only prototype was the wrong proof. **In scope before ship.**

---

## Start with exactly two (now)

| # | Work | Outcome |
|---|---|---|
| **1** | **Interning replan → implement** (continue note → new W2.3-style specs/steps) | `StableKeyId` at key construction; families migrate; no dual string path |
| **2** | **W5.1 Crate split** | polint broken into the target crate graph; leak gate still holds |

Then we stop and talk before starting W5.2 / W5.3.

**After those two, still before merge (planned, not started yet):**

3. W5.2 Persistent store (needs interning)  
4. W5.3 Demand-driven queries (needs store)

**Not before merge:** W5.4, W5.5, W5.6, W5.7.
