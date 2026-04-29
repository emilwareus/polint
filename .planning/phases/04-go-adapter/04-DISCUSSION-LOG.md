# Phase 4: Go Adapter - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-29T04:50:14Z
**Phase:** 04-Go Adapter
**Areas discussed:** Parser diagnostics and extraction source, Go fact breadth, Branch obligations and error-path heuristics, Test evidence and fixture strategy
**Mode:** `--auto`

---

## Parser diagnostics and extraction source

| Option | Description | Selected |
|--------|-------------|----------|
| tree-sitter source of truth | Use tree-sitter nodes as the primary extraction contract and report parser errors as diagnostics while continuing best-effort extraction when safe. Recommended because Phase 4 explicitly requires tree-sitter-go parsing and diagnostics. | ✓ |
| keep line-oriented parsing | Preserve the current string/line scanning implementation and only add tests around it. Faster but brittle for methods, grouped imports, and spans. | |
| defer parser diagnostics | Extract facts but avoid diagnostic behavior until later. Conflicts with GO-01. | |

**Auto choice:** tree-sitter source of truth
**Notes:** Phase 4 should harden the existing parser invocation rather than introduce a Go sidecar or full semantic toolchain.

---

## Go fact breadth

| Option | Description | Selected |
|--------|-------------|----------|
| full syntax-level Phase 4 set | Extract package names where representable, imports, functions, methods, tests, subtests, table-test evidence, calls, complexity, and practical import graph facts. Recommended because it maps directly to GO-02 and GO-04 without claiming semantic precision. | ✓ |
| minimal parser diagnostics only | Close GO-01 first and leave most fact extraction unchanged. Too narrow for the roadmap phase. | |
| full semantic Go analysis | Add type checking and semantic analysis now. Out of scope for v1 Phase 4 and conflicts with project non-goals. | |

**Auto choice:** full syntax-level Phase 4 set
**Notes:** Exact type information remains future semantic work.

---

## Branch obligations and error-path heuristics

| Option | Description | Selected |
|--------|-------------|----------|
| conservative syntax heuristics | Extract obligations for `if`, `switch`, `case`, `default`, `for`, and `range`; mark error paths through clear syntax cues. Recommended because it satisfies GO-03 honestly. | ✓ |
| branch extraction without error-path flags | Create branch facts but skip `is_error_path`. Safer but weakens Go branch-obligation rules. | |
| exact semantic/runtime coverage | Model exact execution paths or dynamic coverage. Out of scope and explicitly deferred by project constraints. | |

**Auto choice:** conservative syntax heuristics
**Notes:** Diagnostics/rule messages must stay honest that branch evidence is heuristic.

---

## Test evidence and fixture strategy

| Option | Description | Selected |
|--------|-------------|----------|
| adapter unit tests plus CLI fixtures | Add focused Go adapter tests and clean/failing CLI fixture coverage. Recommended because it matches TEST-01/TEST-02 Phase 4 scope and existing test patterns. | ✓ |
| adapter unit tests only | Good for extraction internals but misses the user-visible `polint check` behavior required by TEST-02. | |
| broad end-to-end rule suite | Useful later, but risks pulling Phase 6 rule completeness into Phase 4. | |

**Auto choice:** adapter unit tests plus CLI fixtures
**Notes:** Built-in Go rules should be verified only enough to prove Phase 4 facts are usable.

---

## the agent's Discretion

- Exact tree-sitter traversal helper structure.
- Plan split across parser diagnostics, fact extraction, branch/test evidence, and fixture integration.
- Minimal additive core adjustments if needed to represent Phase 4 facts honestly.

## Deferred Ideas

- Full Go type checking through `go/packages` or `go/analysis`.
- Exact dynamic branch coverage.
- Comprehensive Go rule authoring and documentation.
- Production graph command and DOT output hardening.
