# T-VERIFY — tip gate baseline (ae5c7faf)

**Status:** RED — fix in flight before T-INTERN-A  
**Log:** `.swarm/gate-logs/T-VERIFY-ae5c7faf.log`  
**Binding:** `.swarm/DECISION-2026-08-10-PRE-SHIP.md`

| Gate | Exit | Notes |
|---|---|---|
| G1 fmt | 0 | |
| G2 clippy | 0 | |
| G3 workspace tests | 101 | 25 failures — budget default; jelly value-flow expectations; abstract_domains_core |
| G4 leak | 0 | |
| G7 golden | 101 | timing-only `go-sensitive-writes` (522 vs 358); retry policy applies |
| G5 determinism | 0 | |
| G6 polyglot | 0 | |
| G8 doc | 101 | broken intra-doc links in `capability.rs` (fixing) |
| G9 deny | 2 | CLI: use `cargo deny check` not `--all-features` (Makefile) |

**Dispatch rule:** NOTHING else until G3/G7/G8/G9 green on tip.
