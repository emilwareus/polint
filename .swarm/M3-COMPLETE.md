# M3 COMPLETE — real IR landed; M4 authorized

**Status:** W3.1–W3.5 MERGED on `static-analysis-architecture-review`  
**Never merge to main from swarm.**

| Task | Commit | Notes |
|------|--------|-------|
| W3.1 | 5173934e | MirBlock + core terminators; CFG terminator-driven |
| W3.2 | 0d8e03f1 | Throw / Call unwind / Suspend |
| W3.3 | 835859bf | BinOp / Aggregate / Closure+captures / place types |
| W3.4 | 5b037543 | Single `lower_cfg`; lang CFG lowerers deleted |
| W3.5 | 3e25db23 | Deleted `ts_value_flows` + `calls/js_points_to` Oxc pipeline |

Next: W4.1 (points-to primary). Binding: `.swarm/HUMAN-BINDING-2026-08-08-M3-M4.md`.
