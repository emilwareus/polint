# Fix PR Review Findings For Semantic Index Keys

## Goal

Fix the semantic-index review findings before final PR review.

## Scope

- Replace synthetic TS stable export symbol keys with the real exported `SymbolFact.stable_key`.
- Make semantic import, alias, and resolution stable keys collision-resistant for side-effect imports, star reexports, and alias closures.
- Remove validation self-seeding so missing semantic references are reported instead of accepted by their own rows.
- Align TS and Go semantic references with actual symbol/reference stable keys where those facts are emitted.
- Fix strict clippy and whitespace failures.

## Verification

- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test`
- `go test ./...` in `tools/polint-go-symbols`
- `git diff origin/main --check`
