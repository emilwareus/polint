# Go test facts (`TestFact`)

polint harvests one **`TestFact`** per **top-level** Go test entry (`Test…`, `Benchmark…`,
`Fuzz…`) in `*_test.go` files. Rules read them via **`RuleCtx::go_tests`**, per-file iterators,
and **`RuleCtx::go_tests_for_related_file`**.

## Fields

| Field | Meaning |
|-------|--------|
| `file` | Stable **`FileId`** of the `_test.go` source. |
| `function` | Owning test function id, if any. |
| `name` | Test function name (e.g. `TestFoo` or `pkg.TestFoo` for methods). |
| `span` | Source span of the function declaration. |
| `evidence_terms` | Sorted tokens from identifiers and string literals in the body (heuristic). |
| `assertion_count` | Heuristic count of obvious assertions (`t.Fatal`, `require.`, `assert.`, simple nil checks, etc.). |
| `subtest_count` | Number of `t.Run(` call sites in the body (any first argument). |
| `subtest_names` | Literal first arguments only: `t.Run("x", …)` or `` t.Run(`x`, …) `` — not variables or expressions. |
| `table_rows` | Heuristic count of rows in anonymous struct table literals. |

## `evidence_terms`

Built by walking the function body: identifiers, select string contents, and `nil`
when control flow mentions nil checks. This is **heuristic** — not semantic
type-checking — and may miss or over-include tokens.

## Limits (honest coverage)

- Nested test functions inside closures are not separate `TestFact` rows.
- Build tags, generated files, and non-standard layouts may exclude files from analysis.
- Only syntax visible to the tree-sitter Go parser is considered; invalid parse trees may reduce facts.

## Debugging

Use:

```bash
polint explain go-test --file path/to/file_test.go --test TestName
```

to print one harvested fact as JSON (including `subtest_names`).

In rules, `polint::sdk::prelude::collect_go_tests` wraps the per-file iterator.
