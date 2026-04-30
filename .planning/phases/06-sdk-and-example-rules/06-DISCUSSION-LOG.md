# Phase 6: SDK and Example Rules - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-30T07:31:00Z
**Phase:** 06-sdk-and-example-rules
**Areas discussed:** SDK public surface, Rule query helpers, Example rule strategy, Config and diagnostics, Testing proof
**Mode:** `--auto`

---

## SDK public surface

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal additive public SDK | Keep the current core traits and `polint-sdk::prelude::*`, then add docs/tests/helpers where rule authors need them. Recommended because Phase 3 already stabilized the core rule contract. | x |
| Core rewrite | Replace the current trait/context structure with a new SDK architecture. Higher churn and not needed for Phase 6. | |
| Dynamic plugin-first SDK | Design around Wasm or dynamic repo-local loading now. Out of scope until plugin phases. | |

**User's choice:** Auto-selected minimal additive public SDK.
**Notes:** Preserve `Rule`, `RuleMeta`, `Capabilities`, `RuleCtx`, and `RuleOptions` as the baseline.

---

## Rule query helpers

| Option | Description | Selected |
|--------|-------------|----------|
| Add high-level RuleCtx helpers as needed | Expose existing facts through pleasant SDK helpers while preserving deterministic `AnalysisDb` ordering. Recommended because `SDK-02` asks for high-level queries without requiring a full query engine. | x |
| Expose raw db only | Force rule authors through `ctx.db()` and core internals. Simpler but less pleasant and weaker as SDK proof. | |
| Introduce full query engine | Build a generalized query layer now. Too broad for v1 Phase 6. | |

**User's choice:** Auto-selected high-level `RuleCtx` helpers as needed.
**Notes:** Borrowed slices/iterators are preferred over cloning large facts.

---

## Example rule strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Built-in SDK dogfood examples | Keep the eight requested `examples/...` rule IDs and make their implementations use SDK-facing APIs. Recommended because existing CLI/config/tests already know these IDs. | x |
| Repo-local generated examples | Move proof primarily into generated `.polint/rules` examples. Useful documentation, but not enough for built-in rule verification. | |
| Wasm/plugin examples now | Prove rules through the future plugin host. Out of scope for Phase 6. | |

**User's choice:** Auto-selected built-in SDK dogfood examples.
**Notes:** Do not add dynamic plugin loading or repo-local rule compilation in this phase.

---

## Config and diagnostics

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse current RuleOptions with narrow additive fields | Keep existing TOML/profile shape and add fields only when a requested example cannot be configured honestly. Recommended to avoid config churn before Phase 8. | x |
| Per-rule bespoke config types | Stronger typing per rule, but heavier and inconsistent with current config flow. | |
| Config schema overhaul | Redesign config around examples. Too broad and risky for this phase. | |

**User's choice:** Auto-selected reuse current `RuleOptions` with narrow additive fields.
**Notes:** Diagnostics should include stable IDs, ranges, evidence/help where useful, and honest heuristic wording.

---

## Testing proof

| Option | Description | Selected |
|--------|-------------|----------|
| Unit + CLI integration + representative snapshots | Combine SDK/rule unit tests, temp-repo CLI integration, and human/JSON snapshots for representative diagnostics. Recommended because Phase 6 carries `TEST-01` and `TEST-03`. | x |
| Unit only | Fast but does not prove CLI/profile behavior or user-facing diagnostics. | |
| End-to-end only | Good user proof but weak for SDK helper contracts and individual rule logic. | |

**User's choice:** Auto-selected unit plus CLI integration plus representative snapshots.
**Notes:** Full production SARIF hardening remains Phase 8 unless a narrow snapshot is useful.

---

## the agent's Discretion

- Exact plan split across SDK surface, helper APIs, example rule families, diagnostics, fixtures, and snapshots.
- Exact snapshot file layout.
- Exact helper method names when they follow existing Rust/API style.

## Deferred Ideas

None.
