# Phase 11: Capability-Driven Analysis Plan - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-09
**Phase:** 11-capability-driven-analysis-plan
**Areas discussed:** Plan Visibility, Local Rule Host Ownership, Fact Gating Strictness, Unsupported Capability Behavior, Explain Plan Output

---

## Plan Visibility

| Option | Description | Selected |
|--------|-------------|----------|
| Public Summary | Expose a stable read-only plan summary through SDK/explain, while keeping planner internals private. | |
| Internal Only | Keep `AnalysisPlan` crate-private and make `explain plan` the only supported visibility surface. | x |
| Full Public Type | Make the full plan structure part of the supported SDK contract immediately. | |

**User's choice:** Internal Only.
**Notes:** Reconciled with later answer: the full `AnalysisPlan` stays internal, but a narrow read-only support view can be exposed through `RuleCtx`.

| Option | Description | Selected |
|--------|-------------|----------|
| Capabilities + Support | Include requested capabilities, languages, support status, setup probes, and cache digest inputs. | x |
| Capabilities Only | Keep the first model minimal: merged capability booleans by enabled rules. | |
| Detailed Work Graph | Model explicit adapter tasks and dependency edges now. | |

**User's choice:** Capabilities + Support.
**Notes:** The internal plan should carry enough information to explain support and setup behavior, not only booleans.

| Option | Description | Selected |
|--------|-------------|----------|
| Read-Only Access | Add a narrow `RuleCtx` accessor so rules can inspect support/precision without mutating behavior. | x |
| Explain Only | Rules declare capabilities but cannot inspect the resolved plan during `run`. | |
| No Runtime API | Use the plan purely for host orchestration and cache keys. | |

**User's choice:** Read-Only Access.
**Notes:** This does not make the full `AnalysisPlan` public.

---

## Local Rule Host Ownership

| Option | Description | Selected |
|--------|-------------|----------|
| Child rule host owns it | `polint-local-rules` builds the real plan from registered rules. | x |
| Parent CLI owns it | Parent tries to infer or collect rule metadata before running the child. | |
| Both build separately | Parent builds a coarse plan, child builds final plan. | |

**User's choice:** Child rule host owns it.
**Notes:** The registered repo-local rules live in the child process.

| Option | Description | Selected |
|--------|-------------|----------|
| Delegate to child host | Parent invokes local rule host with an explain-plan command and relays output. | x |
| Parent-only fallback | Parent reports only config/discovery info and says rule capabilities are unavailable. | |
| Require explicit manifest flag | User must call a separate command pointing at `.polint/rules/Cargo.toml`. | |

**User's choice:** Delegate to child host.
**Notes:** The parent should mirror the ownership model already used by `check`.

| Option | Description | Selected |
|--------|-------------|----------|
| Empty valid plan | Explain shows files/languages but zero requested capabilities. | x |
| Error | Explain plan fails because no rules exist. | |
| Skip analysis plan | Explain prints a short no-rules message only. | |

**User's choice:** Empty valid plan.
**Notes:** This keeps plan inspection useful even before rules are registered.

---

## Fact Gating Strictness

| Option | Description | Selected |
|--------|-------------|----------|
| Hybrid gating | Always keep basic parsing/source diagnostics; gate optional or future-expensive families where safe. | x |
| Strict gating now | Only harvest facts explicitly requested by enabled rules. | |
| Plan only for now | Build/cache/explain the plan, but do not change fact harvesting yet. | |

**User's choice:** Hybrid gating.
**Notes:** Avoid a compatibility cliff while still making capabilities affect real analysis where safe.

| Option | Description | Selected |
|--------|-------------|----------|
| Debuggable but compatible | Rules may still see currently harvested facts, but docs/tests push authors to declare capabilities. | x |
| Hard empty facts | If a rule did not declare the capability, return empty accessors for that rule. | |
| Runtime warning | Emit warnings when a rule accesses facts it did not declare. | |

**User's choice:** Debuggable but compatible.
**Notes:** Existing rules should not break solely because facts are now planned.

| Option | Description | Selected |
|--------|-------------|----------|
| Plan digest in cache keys | Adapter cache keys change when resolved plan/support inputs change. | x |
| Rule hash only | Continue relying on rule metadata/options as today. | |
| Separate cache namespace | Create a new plan-specific cache directory/version for all Phase 11 runs. | |

**User's choice:** Plan digest in cache keys.
**Notes:** Capability changes must not reuse stale cached facts.

---

## Unsupported Capability Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Emit diagnostic, continue | Report a clear `polint/capability` warning/error and keep running supported analysis. | |
| Fail fast | Stop before rule execution with a fatal error. | |
| Explain only | Show unsupported status in `explain plan`, but `check` stays silent. | |
| Custom | Do not implement future capabilities in any way here; make them non-requestable. | x |

**User's choice:** Do not implement future capabilities in any way here; make them non-requestable.
**Notes:** The user explicitly does not want Phase 11 to partially implement CFG, call graph, coverage, or other later capabilities.

| Option | Description | Selected |
|--------|-------------|----------|
| Warning | Visible and CI-actionable if `--fail-on warn`, but not a hard default failure. | |
| Error | Default `polint check` fails when a requested capability is unsupported. | |
| Info | Advisory only. | |
| Not applicable | Does not matter if unsupported capabilities are not selectable; fail clearly if requested anyway. | x |

**User's choice:** Not applicable if unsupported capabilities are not selectable; fail clearly if requested anyway.
**Notes:** Planning should prevent unsupported capabilities from looking supported.

| Option | Description | Selected |
|--------|-------------|----------|
| Capability diagnostic with setup hint | Deterministic warning/error explaining exact missing setup and docs path. | x |
| Adapter-specific parser diagnostic | Reuse `parser/go` or `parser/ts` style diagnostics. | |
| No diagnostic unless rule emits one | Rule authors decide how to report missing support. | |

**User's choice:** Capability diagnostic with setup hint.
**Notes:** Supported-but-missing-setup is different from parser failure.

---

## Explain Plan Output

| Option | Description | Selected |
|--------|-------------|----------|
| Human + JSON | Human default, deterministic `--format json` for agents/CI. | x |
| JSON only | Simpler and agent-first. | |
| Human only | Useful locally, less stable for automation. | |

**User's choice:** Human + JSON.
**Notes:** Human output is default; JSON is needed for agents and CI.

| Option | Description | Selected |
|--------|-------------|----------|
| Rules, capabilities, support, setup, digest | Show enabled rules, requested capabilities, support/requestability status, setup probes, and plan digest. | x |
| Rules + capabilities only | Smaller and easier to stabilize. | |
| Full internal details | Include adapter work units and cache internals. | |

**User's choice:** Rules, capabilities, support, setup, digest.
**Notes:** Do not expose full internals, but include enough to debug planning and cache behavior.

| Option | Description | Selected |
|--------|-------------|----------|
| No parsing | Load config/rules, build plan, run setup probes only. | x |
| Optional parsing flag | Default no parsing, `--with-analysis` validates against discovered files. | |
| Always parse | Prove the plan end to end, but slower and less explain-only. | |

**User's choice:** No parsing.
**Notes:** `explain plan` should be cheap and should not need to parse source files.

---

## the agent's Discretion

- Exact internal type names and module placement.
- Exact deterministic JSON field names for `explain plan`.
- Whether plan digest is folded into existing rule/cache hash or passed as a distinct cache-key component.

## Deferred Ideas

- Actual CFG fact construction - Phase 12.
- Coverage report import - Phase 13.
- Resolved imports/module graph - Phase 14.
- Direct call graph facts - Phase 15.
- Symbol/reference facts - Phase 16.
