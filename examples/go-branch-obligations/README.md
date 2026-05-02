# Go Branch Obligations Example

`examples/go-branch-obligations` is a small Go fixture for the example
`examples/go-branch-obligations` rule in `examples/rules`.

Run it from this directory:

```bash
cargo run --manifest-path ../rules/Cargo.toml -- check --profile fast --format json --fail-on none
```

The example intentionally has error branches without companion tests. The rule is
heuristic: it looks for branch obligations and nearby Go test evidence, then
reports diagnostics such as `No nearby test evidence found` when the local test
signal is missing.

This does not prove exact branch coverage. It is a syntax/fact-level policy
example for teams that want an executable reminder to add tests around important
error paths.
