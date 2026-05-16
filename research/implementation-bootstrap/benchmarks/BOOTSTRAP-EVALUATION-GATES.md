# Bootstrap Evaluation Gates

The first semantic implementation does not need full external benchmark
coverage. It does need gates that prevent architectural drift.

## Required Native Fixtures

Create small fixtures for Go and TS/JS covering:

- direct assignment;
- branch narrowing;
- constant propagation;
- nullish/nil checks;
- direct function call;
- method/member call that remains unresolved;
- unsupported construct preserved as an unsupported fact.

Expected outputs:

- MIR snapshot;
- place snapshot;
- direct call snapshot;
- P0 domain snapshot;
- direct summary snapshot.

## Determinism Gates

Run each fixture multiple times and assert:

- same stable keys;
- same dense ID order where input order is fixed;
- same sorted fact output;
- same diagnostics;
- same cache keys.

## Extension Gates

Use a test-only fake extension emitter. Assert:

- additive model facts improve result;
- invalid references are rejected;
- conflicting exact facts are diagnosed;
- suppressions are not silent;
- provenance labels distinguish native and extension facts.

## Performance Baseline

Record initial rough metrics:

- files analyzed per second;
- MIR operations per second;
- direct call facts per second;
- summary cache hit rate;
- peak memory on a medium fixture.

No optimization should be accepted without a baseline and a measured delta.
