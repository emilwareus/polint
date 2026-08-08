# M4 — One principled engine (binding overview)

**Human authorized:** 2026-08-08 — `.swarm/HUMAN-BINDING-2026-08-08-M3-M4.md`  
**Precondition:** M3 exit (real MIR + CFG from terminators; no source-text CFG; `ts_value_flows` folded/retired).

## Goal

Replace recognizer-bank / unrealizable BFS reachability with a principled interprocedural engine over the repaired ICFG for **existing** Go/TS security and dataflow surfaces.

## Tasks (specs to be expanded at dispatch; contracts below are binding)

| Task | Contract |
|---|---|
| W4.1 | Promote existing `js_points_to` Andersen solver to primary TS resolver; object sensitivity as required by existing call precision — **migrate**, do not dual-run recognizer bank |
| W4.2 | IFDS/IDE solver over repaired ICFG (call/return matched paths) |
| W4.3 | Taint: existing typed sources → sanitizers → sinks with replayable paths on IFDS |
| W4.4 | Retire recognizer bank + reachability filter that inflates precision |
| W4.5 | Lift existing constant-prop / nullability domains to IDE |

## Exit

Existing shipped security templates measured with honest paths; **no unrealizable path** reported by the production engine. Goldens locked only with evidence. No new languages.

Detailed step specs (`W4.1-*.md` …) are written immediately before each dispatch (same bar as W3.1).
