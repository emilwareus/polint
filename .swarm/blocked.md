# Hold queue

Append-only. Orchestrator writes one entry per HOLD. RESOLVED entries stay for audit.

## W0.3 — 2026-08-07T17:58:00Z — RESOLVED 2026-08-08

- **task:** W0.3
- **resolution:** **MERGED-NOOP** per human binding decision. Do not invent a Rust frontend. Layering dogfood deferred until a Rust language adapter exists (W2.6). Rationale written in HANDOFF.md §5 / PLAN.md M0.B.
- **prior reason:** no Rust frontend for `module_graph` + `resolved_imports` dogfood.

## W0.2 — 2026-08-08T09:06:00+02:00 — RESOLVED 2026-08-08

- **task:** W0.2
- **resolution:** Human binding — accuracy F1 gate **and** cost-column asserts both stay; reconcile into one coherent test path + baseline JSON on integration. Land reconciled commit from `swarm/W0.2` @ `11fd5187` + landed W0.1.
- **prior reason:** semantic conflict with MERGED W0.1 on external/mod.rs and baseline JSON.

## W0.A2 — 2026-08-08T09:38:30+02:00 — RESOLVED 2026-08-08

- **task:** W0.A2
- **resolution:** Human binding — example self-pairs × `json` only; eval-fixtures out of golden cartesian; scale optional loud-skip. Land `swarm/W0.A2` @ `5cc602c5` onto integration after rebase.
- **prior reason:** `(target × pack × format)` pairing unspecified.

## W0.4 — 2026-08-08T09:45:30+02:00 — RESOLVED 2026-08-08

- **task:** W0.4
- **resolution:** Human binding — no Grafana `full_pipeline` requirement for M0; publish what fits; OOM recorded loudly in artifact with LOC attempted; stop after first OOM / use lightest honest surface. Finish WIP from `swarm/W0.4` within budget.
- **prior reason:** full_pipeline SIGKILL on excalidraw; no defined fallback.

## W1.5 — 2026-08-08T13:08:00+02:00 — RESOLVED / MERGED-NOOP 2026-08-08

- **task:** W1.5
- **resolution:** **MERGED-NOOP** per ORCHESTRATION §7 expected outcome. Measurement-only evidence landed; parse-cache instrumentation abandoned (branch kept, not merged). Dependents: none blocked.
- **escalate reason (verbatim):** `not-worth-doing` — measured parse cost is a small fraction of run wall-clock on the binding M0.A corpus.
- **measurement:** aggregate parse ~6.23% of run (max ~10.51%); ~1 parse/file. See `docs/architecture-review/W1.5-STEP1-MEASUREMENT.md`.
- **worker / branch:** `8fb55ac2-c9b8-4f23-96b4-477552909de9` / `swarm/W1.5` @ `87f905ce`
- **hold_count:** unchanged (not an open hold; do not HALT).

## W2.3 — 2026-08-08T16:20:00+02:00 — OPEN

- **task:** W2.3
- **state:** HELD / ESCALATE
- **escalate reason (verbatim):** `no-measurable-win` — dense SemanticNodeId + bitset Go RTA worklists regress wall-clock ~1.8× on chain (8k) and wide (4k) synthetics; worklist-local retained-bytes savings (~832KB→~33KB) do not move end-to-end / W0.A4 retained bytes because `GoRtaInputs` still owns the qualified Strings.
- **measurement:** `docs/architecture-review/W2.3-STEP1-MEASUREMENT.md` landed FF @ `7f988fc5`.
- **product code:** go_rta unchanged (prototype measured and discarded).
- **branch:** `swarm/W2.3` @ `7f988fc5`
- **dependents:** hold whole of W2.3 (StableKeyId / family migration). **Do not let W2.5 proceed on the assumption interning is coming.**
- **locks:** `fact_family` none held.

## W2.3 — 2026-08-08T16:28:00+02:00 — RESOLVED / MERGED-NOOP 2026-08-08

- **task:** W2.3
- **resolution:** **MERGED-NOOP** per ORCHESTRATION §7 expected outcome. Dense SemanticNodeId + bitset Go RTA worklists regress wall-clock ~1.8×; measurement-only evidence landed. Do not start StableKeyId family migration. **Do not assume interning for W2.5.**
- **escalate reason (verbatim):** no measurable win — wall-clock regresses on binding synthetics; worklist-local memory savings do not move end-to-end retained bytes / W0.A4.
- **measurement:** `docs/architecture-review/W2.3-STEP1-MEASUREMENT.md` (chain 8k ~2479→~4500 µs/iter; wide 4k ~1094→~1939 µs/iter).
- **worker / branch:** swarm/W2.3 @ `fae8e614` (measurement tip refresh) / escalate doc `7f988fc5`
- **hold_count:** unchanged (not an open hold; do not HALT). Dependents: W2.5 blocked pending human replan of interning model.
