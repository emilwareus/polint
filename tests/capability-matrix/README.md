# Capability matrix

Pins **capability presence** for every SDK prelude fact view, per language that
claims support. Distinct from the golden corpus: goldens lock *output*; this
matrix locks that a view still returns non-empty, well-formed data.

| Kind | Location |
|------|----------|
| Cell inventory | [`matrix.toml`](matrix.toml) |
| Language fixtures | [`fixtures/`](fixtures/) |
| Harness | `crates/polint/tests/capability_matrix.rs` |

Each `supported` cell maps to a language fixture under `fixtures/<language>/`
that is known to produce that fact family today. The harness runs probe rules
requesting typed views and asserts a `capability present` diagnostic with `view`
evidence. One language fixture covers all supported cells for that language.

`reserved` cells document prelude views that no language claims yet
(`Cfg`, `CallGraph`, `CoverageFacts`, `TestSuiteMetrics`). `review_only` covers
`ChangedFiles`, which is populated only under `polint review`. The Go matrix
sets `languages.go.include_tests = false` so symbol/reference facts load; GoTests
still come from syntax on `_test.go` files in the fixture.

Do not merge this into the golden harness. Do not update cells to hide a
capability loss — a red matrix cell means the refactor dropped support.
