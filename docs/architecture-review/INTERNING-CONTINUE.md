# Continue StableKeyId interning — research note

> **DECIDED — see [`.swarm/DECISION-2026-08-10-PRE-SHIP.md`](../../.swarm/DECISION-2026-08-10-PRE-SHIP.md) §Q2.**
> Interning is **required before ship**, at **slice C** (interner + construction site + all fact
> families + `FactMeta` + owner maps). **Solver densification (Phase C below) is OUT of scope.**
> Pass/fail is **structural, not performance**: `stable_key: String` count must reach 0, determinism
> and goldens must stay green; memory is **measured and recorded, not gated**; the only performance
> block is a sustained >10% wall-clock regression. Phases A and B below are correct — follow them.

**Status:** CONTINUE (replan; do not treat W2.3 MERGED-NOOP as “interning is wrong”)  
**Date:** 2026-08-09  
**Branch:** `static-analysis-architecture-review`  
**Context:** Human binding — best identity model before ship; prior attempt was an incorrect proof shape, not proof that interning cannot win.

---

## What we are trying to fix

Facts still carry identity as owned `String` (`stable_key: String` and related maps). The same logical identity is often retained in multiple places (fact row, fact metadata, ownership maps). That costs memory and makes hot loops compare/hash long text instead of small integers.

The intended end state (from `specs/W2.3-interning.md`) remains right:

- One `AnalysisDb`-scoped interner (not process-global)
- `StableKeyId(u32)` handles everywhere facts currently own key text
- Sort/determinism by **resolved text**, never by allocation-order id
- Migrate family-by-family; measure with W0.A4 cost records

That design matches how compilers and analyzers normally work: intern once, pass `Copy` ids, resolve to `&str` only for display, digests, and deterministic ordering.

---

## What we actually measured (and why it misled)

W2.3 step 1 only rewrote the **Go RTA worklist** from `BTreeSet<String>` to dense ids + bitsets (`W2.3-STEP1-MEASUREMENT.md`):

| Result | Meaning |
|---|---|
| ~1.8× **slower** wall-clock on synthetic chain/wide graphs | Proof-of-concept **implementation** lost |
| Worklist-local memory ~25× better | Bitsets help *that* set only |
| End-to-end RSS / W0.A4 **unchanged** | Inputs still owned full qualified `String` keys for the whole solve |

### Why that attempt was the wrong proof

1. **Interning was not installed at the choke point.** Keys are built in `stable_key_from_parts` (~115 call sites). The experiment never put an interner there; it densified one solver’s scratch set while the snapshot still held strings.
2. **Index rebuild dominated.** Dense adjacency was rebuilt at the start of every `solve_go_rta` call. That tax can erase bitset wins on medium graphs.
3. **Wrong success metric for step 1.** Spec D5 treated “RTA wall-clock must improve” as the gate for *all* of StableKeyId. Retained-bytes / duplicate-key removal is the real product thesis; a local fixpoint microbench cannot prove or disprove it while strings remain the store keys.
4. **BTreeSet&lt;String&gt; was not the main tax.** Once indexes exist, membership in a string set is often not the bottleneck; rebuilding structures and keeping N string copies is.

**Conclusion:** escalate correctly said “this prototype is not the migration.” It did **not** say “never intern.”

---

## Better approach (continue from here)

### Phase A — Install the interner where keys are born

1. Add `StableKeyInterner` on `AnalysisDb` / provider context (per D1).
2. Change `stable_key_from_parts` to return `StableKeyId` (intern on construct).
3. Keep displaying/sorting via `interner.resolve(id)` (per D2).
4. Measure W0.A4 retained bytes / RSS **before** chasing solver microbenches.

### Phase B — Stop re-owning strings

Replace `stable_key: String` and `to_string()` propagation with `Copy` ids, family by family (symbols → MIR → calls → CFG → rest). Delete dual `String` fields in the same PR as each family migrates (no compat shims).

### Phase C — Only then densify solvers

Once inputs are id-keyed (or hold ids alongside a single intern table):

- Persist dense adjacency on the input snapshot (build once, reuse).
- Use bitsets / dense frontiers over **stable dense node ids**, not over freshly ranked text each call.
- Re-measure Go RTA (and points-to hot loops) — expect wall-clock wins only after Phase A/B.

### Phase D — FactMeta / owner maps

After fact rows use ids, intern `FactMeta` and `stable_key_owners` keys (spec D4). Conflict reporting still resolves to text for users.

### Anti-patterns (do not repeat)

- Densify one solver while `GoRtaInputs` (or equivalent) still owns `String` maps
- Global/process interner
- Sorting by raw `StableKeyId`
- Dual `String` + `StableKeyId` fields left on the tip
- Claiming victory from worklist-local KB estimates without W0.A4 movement

---

## Success criteria (honest)

| Gate | Pass when |
|---|---|
| Memory | W0.A4 (or successor) retained-bytes / peak RSS moves in the expected direction on the binding corpus after Phase A/B |
| Correctness | Determinism gate + goldens (diagnostic sets) stay green; sorts use text order |
| Speed | Solver microbenches may improve in Phase C; **not required** to greenlight Phase A/B if memory/correctness land |
| Architecture | No parallel string identity path on tip |

---

## Binding

**CONTINUE interning on this branch before ship** as part of finishing the identity-model migration. Prior W2.3 MERGED-NOOP means “discarded the bad prototype,” not “abandon StableKeyId.”

Related unlock: persistent store (M5 W5.2) depends on a stable interned identity model; do not build disk persistence on duplicated strings.
