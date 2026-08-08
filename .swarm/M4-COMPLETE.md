# M4 COMPLETE — principled engine on this branch

**Status:** W4.1–W4.5 MERGED on `static-analysis-architecture-review`  
**Never merge to main from swarm.**

| Task | Notes |
|------|-------|
| W4.1 | Points-to sole TS resolver; token/object banks deleted |
| W4.2 | Matched IFDS/ICFG; unmatched BFS deleted |
| W4.3 | Taint + sanitizer kills in IFDS; post-solve barrier filter deleted |
| W4.4 | Reachability precision theater / scoring filters deleted |
| W4.5 | IdeDomainSolver sole domain path; LocalDomainSolver deleted |

**Human binding:** `.swarm/HUMAN-BINDING-2026-08-08-M3-M4.md`  
**Follow-ups (explicit):** StableKeyId interning; W0.3 Rust dogfood — not blockers.

M3+M4 architecture migration for existing Go/TS analysis is closed on this branch pending human ship review / final tip gates.
