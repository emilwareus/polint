# Decision Log: Evaluation Harness

## D1. Use external-benchmark-first strategy

Decision: external benchmarks should be the primary evidence for scanner-level quality.

Reasoning:

- independent ground truth is more credible than self-authored fixtures;
- existing suites cover years of edge cases;
- external metrics make results comparable;
- agent-authored extensions need objective feedback.

Consequence: benchmark adapters become first-class infrastructure.

## D2. Do not rely exclusively on external benchmarks

Decision: keep a small native polint fixture layer.

Reasoning:

- external suites do not test provenance, merge policy, cache invalidation, deterministic scheduling, or typed SDK stability;
- polint's engine behavior is part of the product;
- extension safety cannot be outsourced to scanner benchmarks.

Consequence: native fixtures should be narrow and engine-focused, not a replacement for external suites.

## D3. Preserve suite-native metrics and add unified metrics

Decision: every adapter should report both.

Reasoning:

- OWASP scorecards, RealVuln F3, and other suite-native metrics are useful for comparison;
- polint needs cross-suite precision/recall/F-score, unknowns, path quality, provider stats, and extension deltas.

Consequence: report format must support arbitrary suite-native metric maps.

## D4. Treat dynamic graph truth as partial

Decision: dynamic observed call/data-flow edges are required evidence, but extra static edges are not automatically false positives.

Reasoning:

- dynamic execution only covers paths that ran;
- conservative static analysis may include real but unexecuted edges;
- naive precision scores would punish valid conservative edges.

Consequence: graph results need `EXTRA_UNCONFIRMED` and `PartialGroundTruth` result classes.

## D5. Implement hidden/internal first

Decision: build `polint eval` as internal/hidden until schemas stabilize.

Reasoning:

- evaluation schemas will change as adapters are added;
- public CLI/API promises would slow iteration;
- internal access to kernel/provider/cache stats is required.

Consequence: keep modules `pub(crate)` and avoid public documentation until stable.

## D6. Build the harness before serious call graph/data-flow implementation

Decision: evaluation should precede large analysis-family implementation.

Reasoning:

- without metrics, algorithm sophistication is easy to confuse with product progress;
- call graph/data-flow design choices need recall/precision/cost feedback;
- agent-extension value must be measured as a delta from default mode.

Consequence: next implementation work after kernel should include the harness vertical slice.
