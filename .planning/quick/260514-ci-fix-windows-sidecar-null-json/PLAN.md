# Plan

Fix the Windows CI failure in the Phase 13 symbols/references PR.

## Tasks

1. Make the Go symbol sidecar emit arrays, not `null`, for sequence fields in
   its JSON contract.
2. Make Rust sidecar deserialization tolerate `null` sequence fields from older
   or externally supplied sidecar binaries.
3. Add regression tests for the JSON shape/reader behavior.
4. Run targeted tests, formatting, clippy, and the relevant full workspace check.
5. Commit and push the fix to the PR branch.
