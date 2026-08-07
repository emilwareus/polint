# Implementation Specs

Binding contracts for work items that need more than a table row. **Read [`../HANDOFF.md`](../HANDOFF.md)
first** — it carries the errata, the hard rules, and the task tiering.

## Precedence

**Spec > HANDOFF > PLAN > review documents (`01`–`10`).**

The reviews are analysis produced by parallel agents; several of their numbers and framings are
imprecise. The specs were written afterwards, verified directly against the tree, and correct them.
Where they disagree, the spec is right.

## The specs

| Spec | Goal | Precondition | The fact that de-risks it |
|---|---|---|---|
| [W1.1](W1.1-parse-error-honesty.md) | Stop silently analysing partial ASTs | M0.A | `ts/adapter.rs` already does it right — copy that pattern |
| [W1.2](W1.2-ship-evidence.md) | Ship provenance to users | M0.A | 4,335 LOC already written; three lines delete it |
| [W1.3](W1.3-rule-telemetry.md) | Make silent rules diagnosable | M0.A | Additive field on an existing summary struct |
| [W1.5](W1.5-parse-cache.md) | Stop re-parsing | M0.A **+ W0.A4** | **Measure-first checkpoint — may conclude "not worth doing"** |
| [W2.3](W2.3-interning.md) | `StableKeyId` | W2.2, W0.A4 | `stable_key_from_parts` is the single construction site |
| [W2.4](W2.4-provider-trait-and-scheduler.md) | Execute the manifest DAG | W2.2, M0.A | Manifest order **is** execution order — it's a verification, not a redesign |
| [W2.5](W2.5-fact-store.md) | Decompose `AnalysisDb` | W2.4 | 15 provider-owned stores already exist |
| [W2.6](W2.6-language-frontend.md) | Open language registry | W2.4 | The two adapters have byte-identical signatures |

## Spec structure

Every spec has: **Goal** · **Why** (with evidence) · **Preconditions** · **Design decisions** (the
part that cannot be improvised) · **Ordered PR-sized steps** · **Acceptance** (exact commands) ·
**Anti-goals** · **Escalate if**.

Design decisions are labelled `D1`, `D2`, … and are binding. **If you disagree with one, escalate —
do not deviate.** A design decision you route around silently is how the built-not-wired pattern
(HANDOFF §3) reappears.

## Three specs contain a mandatory stop

These exist because proceeding past a failed checkpoint produces confident, plausible, wrong work:

- **W1.5 step 1** — measure parse cost before implementing. If parse time is a small fraction of the
  run, **report and stop**; the task is not worth doing.
- **W2.3 step 1** — the Go RTA fixpoint conversion must show a measurable win before the migration is
  committed to. If it does not, the memory model is wrong and the task needs re-planning.
- **W2.4 step 4** — the topological sort must reproduce the declared manifest order exactly. If it
  does not, **escalate**. Do not edit the expected value and do not special-case the sort.

## Not specified here, deliberately

**M3 (real IR) and M4 (IFDS/taint) have no specs and must not be implemented from PLAN.md.** They are
genuine design problems and one-way doors — a half-right IR is worse than none, because analyses fork
around it exactly as `ts_value_flows.rs` (11,898 LOC) already did. They are blocked on M0–M2 anyway.
When M2 completes, they get the same treatment these did: a spec written by someone with full context,
reviewed, *then* implemented.
