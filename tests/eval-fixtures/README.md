# Native eval fixtures

Native evaluation fixtures live under:

```text
tests/eval-fixtures/<area>/<case>/repo/
tests/eval-fixtures/<area>/<case>/expected.polint-eval.toml
```

The manifest describes the expected rows for the tiny repo owned by the fixture.
Paths in manifests must be relative fixture-owned paths; absolute paths and
parent-directory traversal are rejected before the fixture is run.

external benchmark content must not be committed here. Benchmark suites, copied
third-party cases, and larger benchmark corpora belong behind future adapter
manifests with explicit license review.
