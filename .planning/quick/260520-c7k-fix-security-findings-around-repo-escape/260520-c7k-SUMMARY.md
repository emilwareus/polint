# Quick Task 260520-c7k: Summary

**Completed:** 2026-05-20
**Status:** Complete

## Changes

- Added centralized repo-contained file helpers in `crates/polint/src/module_graph/paths.rs` that reject absolute paths, ancestor escapes, and symlink escapes by canonicalizing the repository root and target before reading.
- Switched TS/JS topology, module graph cache key inputs, input snapshots, and Go topology lifecycle reads to safe bounded reads for `package.json`, lockfiles, `pnpm-workspace.yaml`, `tsconfig` / `jsconfig`, `go.mod`, `go.work`, and `go.sum`.
- Rejected absolute and escaping workspace glob bases before `read_dir`, and rejected absolute / escaping `tsconfig.extends` resolution.
- Added topology input size limits: 1 MiB for manifests/config files and 16 MiB for lockfile-style inputs. Oversized topology files now become controlled unsupported facts/components instead of being parsed.
- Rejected Go package patterns starting with `-`, added `-mod=readonly` to `go list`, and replaced predictable synthetic `go.work` paths with exclusive `tempfile` creation held alive for the command.
- Added regression tests for symlink escapes, absolute tsconfig extends, escaping workspace globs, oversized manifests/lockfiles, and rejected Go package-pattern flags.

## Verification

- `cargo test -p polint module_graph::ts::topology -- --nocapture`
- `cargo test -p polint analysis_kernel::incremental::keys::tests::module_graph_layer_key -- --nocapture`
- `cargo test -p polint analysis_kernel::incremental::input_snapshot::lifecycle -- --nocapture`
- `cargo test -p polint go::lifecycle -- --nocapture`
- `cargo test -p polint module_graph::go::dependency_topology -- --nocapture`
- `cargo test -p polint`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
