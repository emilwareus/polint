# Phase 41: Public SDK Query Views and Agent Ergonomics - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-26
**Phase:** 41-public-sdk-query-views-and-agent-ergonomics
**Areas discussed:** Promotion thresholds, Public SDK query shape, Stable CLI JSON contracts, Agent authoring workflow, Docs/tests/compatibility gates
**Mode:** `$gsd-discuss-phase 41 --auto`

---

## Promotion Thresholds

| Option | Description | Selected |
|--------|-------------|----------|
| Evidence-gated promotion | Promote only surfaces with docs, fixtures, temp-repo tests, cache/input behavior, no-leak proof, and precision/status semantics. | ✓ |
| Roadmap-gated promotion | Promote all candidate views named by Phase 41 because the roadmap reached promotion. | |
| Keep everything internal | Defer every advanced view even if a narrow public contract is ready. | |

**User's choice:** Auto-selected evidence-gated promotion.
**Notes:** `[auto] Promotion Thresholds — Q: "How strict should public promotion be?" → Selected: "Evidence-gated promotion" (recommended default)`.

---

## Public SDK Query Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Typed bounded query builders | Public rules use typed fact views and bounded domain query methods with status/precision evidence. | ✓ |
| Raw graph/store access | Public rules can inspect internal stores, graph rows, or provider IDs directly. | |
| Command-only ergonomics | Keep the SDK minimal and expose analysis only through CLI JSON. | |

**User's choice:** Auto-selected typed bounded query builders.
**Notes:** `[auto] Public SDK Query Shape — Q: "What public query shape should agents and humans author against?" → Selected: "Typed bounded query builders" (recommended default)`.

---

## Stable CLI JSON Contracts

| Option | Description | Selected |
|--------|-------------|----------|
| Stabilize narrow useful commands | Stabilize only real bounded commands such as inspect/test and possibly facts/unknowns/explain with versioned JSON. | ✓ |
| Stabilize all internal eval/debug output | Treat eval/debug schemas as public command contracts immediately. | |
| Avoid new command contracts | Keep all agent feedback in docs and SDK examples only. | |

**User's choice:** Auto-selected narrow useful commands.
**Notes:** `[auto] Stable CLI JSON Contracts — Q: "Which CLI JSON should become public product surface?" → Selected: "Stabilize narrow useful commands" (recommended default)`.

---

## Agent Authoring Workflow

| Option | Description | Selected |
|--------|-------------|----------|
| Preserve artifact boundaries | Rules, model packs, summaries, provider extensions, and fixtures each keep a distinct role. | ✓ |
| Let rules do everything | Encourage large rules to rediscover framework, data-flow, or call semantics directly. | |
| Provider-first workflow | Send agents to provider extensions before simpler rules or models. | |

**User's choice:** Auto-selected preserve artifact boundaries.
**Notes:** `[auto] Agent Authoring Workflow — Q: "How should agent-authored artifacts be guided?" → Selected: "Preserve artifact boundaries" (recommended default)`.

---

## Docs, Tests, and Compatibility Gates

| Option | Description | Selected |
|--------|-------------|----------|
| Docs/tests with every promotion | Every new public surface ships with docs, temp-repo tests, stable JSON checks, and no-leak proof. | ✓ |
| Docs later | Implement SDK methods first and document after adoption. | |
| Tests only | Rely on tests without updating docs/facts or examples. | |

**User's choice:** Auto-selected docs/tests with every promotion.
**Notes:** `[auto] Docs, Tests, and Compatibility Gates — Q: "What must ship with a promoted public surface?" → Selected: "Docs/tests with every promotion" (recommended default)`.

---

## The Agent's Discretion

- The planner may decide the exact first set of promoted query builders after inspecting code and tests.
- The planner may choose exact stability labels and schema module names.
- The planner may keep advanced views unsupported if they cannot meet the promotion threshold.

## Deferred Ideas

- Broad raw graph/database public API.
- Unbounded whole-program call/data-flow/path/slice queries by default.
- Stable public eval JSON if the contract cannot be narrowed and documented.
- Stable model-pack/provider-extension public SDK if examples and validation are not ready.
