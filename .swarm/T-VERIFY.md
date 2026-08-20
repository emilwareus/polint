# T-VERIFY — tip gate baseline

**Status:** GREEN  
**Tip at clear:** `8427dcc6` (+ doc fix `1c6aafe6`)  
**Binding:** `.swarm/DECISION-2026-08-10-PRE-SHIP.md`  
**Fix note:** `.swarm/T-VERIFY-FIX.md`  
**Logs:** `.swarm/gate-logs/T-VERIFY-ae5c7faf.log`, `.swarm/gate-logs/T-VERIFY-G3-recheck.log`

G3 initially red (post-M4 eval expectations). Fixed without restoring deleted pipelines.
G7 timing: retry PASS (deltas recorded in T-VERIFY-FIX). G9: `cargo deny check`.
