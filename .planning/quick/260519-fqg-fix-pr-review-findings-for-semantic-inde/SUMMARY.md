# Summary

Fixed the semantic-index PR review findings:

- TS stable export identities now use the real exported symbol stable key.
- TS semantic imports, aliases, resolutions, and alias closures now include enough identity to avoid collisions.
- Semantic validation now checks references against real symbol/reference/semantic rows instead of accepting self-seeded keys.
- Go semantic sidecar keys are mapped to the actual symbol/reference stable keys produced by the Rust symbol graph path.
- TS semantic resolution rows are emitted only when reference facts are also requested.
- The clippy issue and planning-log trailing blank line were fixed.

Verification passed:

- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test`
- `go test ./...` from `tools/polint-go-symbols`
- `git diff origin/main --check`
