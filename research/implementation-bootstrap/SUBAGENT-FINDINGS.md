# Subagent Findings

No subagents were used for this track.

Reason: the current user request asked for deep research and Rust skill usage,
but did not explicitly ask for subagents in this turn. Current tool policy
allows spawning subagents only when the user explicitly asks for subagents,
delegation, or parallel agent work.

## Main-Agent Angles Covered

| Angle | Status | Notes |
| --- | --- | --- |
| Local Rust architecture review | Complete | Reviewed public/private API, `AnalysisDb`, SDK views, macro capabilities, adapters, graph builders, cache keys, and tests. |
| Rust best-practices review | Complete | Read the local Rust skill and relevant chapters on borrowing, linting, performance, errors, testing, dispatch, typestate, docs, and pointers. |
| Prior research synthesis | Complete | Cross-checked bootstrap against analysis kernel, call graph, data-flow, summaries, type/alias, CFG, extension, and abstract-interpretation tracks. |
| Skeptical validation | Complete | Identified rejected paths and remaining high-risk open questions in `RESEARCH-ANALYSIS.md` and `VALIDATION.md`. |

## Main Critiques Preserved

1. `AnalysisDb` should own semantic stores but not implement every semantic
   family directly.
2. `FunctionFact.calls` is not a viable call graph seed beyond legacy syntactic
   hints.
3. Public SDK views must wait until the internal fact family is documented,
   validated, cached, and tested through external-rule fixtures.
4. Dynamic dispatch belongs at extension boundaries, not in the native hot path.
5. Stable keys are required for interprocedural facts; dense IDs alone are
   insufficient.
