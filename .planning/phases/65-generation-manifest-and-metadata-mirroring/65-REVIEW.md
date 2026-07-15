---
phase: 65-generation-manifest-and-metadata-mirroring
reviewed: 2026-07-15T20:28:43Z
depth: deep
files_reviewed: 15
files_reviewed_list:
  - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-17-PLAN.md
  - crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go
  - crates/polint/src/analysis/extensions/provider.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/store/commit_plan.rs
  - crates/polint/src/analysis_kernel/store/generation.rs
  - crates/polint/src/analysis_kernel/store/migrations.rs
  - crates/polint/src/analysis_kernel/validation.rs
  - crates/polint/src/analysis_plan.rs
  - crates/polint/src/go/semantic/client.rs
  - crates/polint/src/go/semantic/process.rs
  - crates/polint/src/go/semantic/protocol.rs
  - crates/polint/src/policy_queries.rs
  - crates/polint/src/repo_fs.rs
  - crates/polint/src/runner/mod.rs
findings:
  critical: 0
  warning: 6
  info: 0
  total: 6
status: issues_found
---

# Phase 65 Deep Code Re-review Report

**Reviewed:** 2026-07-15T20:28:43Z
**Depth:** deep, three fresh independent post-fix reviewers
**Fix range:** `4bb2be13..ba099af7`
**Full diff:** `origin/main...ba099af7`
**Status:** issues found

## Summary

The third fix pass resolved all nine prior findings and a full-gate cache
publication race, then passed `make check` end to end. A fresh fourth review
found six novel issues: three incomplete Go execution boundaries, one
post-execution capability-trust gap, and two publication/ordinal integrity gaps
in the semantic store.

## Go Runtime and Toolchain Boundaries

### SEC-05 (P1): Sealed Go identity excludes the toolchain closure it executes

**File:** `go/semantic/process.rs:294,1511-1547,1719,2030`

Preparation hashes and seals only `GOROOT/bin/go`, while retaining and passing
the original mutable `GOROOT`. The launcher subsequently executes compiler,
linker, and other tools under `pkg/tool` and consumes standard-library data, but
identity records only the launcher digest and GOROOT pathname. Mutating a helper
under the same root can therefore change cold-run behavior without rotating the
active-generation identity.

Certify and pin the complete execution closure. At minimum, bind and revalidate
all selected `GOTOOLDIR` executables and relevant GOROOT content; preferably
execute from a sealed content-addressed closure. Add two GOROOT fixtures with
identical launchers/version output but different delegated tool bytes, then
prove a post-prepare swap is pinned, rejected, or identity-rotating.

### WR-25 (P1): Ambient Go module-resolution state remains unauthenticated

**Files:** `go/semantic/process.rs:25,294,1706`,
`go/semantic/client.rs:90`,
`go-sidecar/polint-go-frontend/internal/semantic/emit.go:105,1079`

The normalized environment leaves module/proxy/cache/VCS inputs such as
`GOPROXY`, `GOPRIVATE`, `GONOPROXY`, `GONOSUMDB`, `GOSUMDB`, `GOPATH`,
`GOMODCACHE`, `GOCACHE`, `GOVCS`, and proxy credentials inherited. The
frontend passes a fresh `os.Environ()` to `packages.Load`, yet these values are
absent from tool identity. The same source snapshot can therefore reuse facts
produced with different dependency resolution or package-loading results.

Use one explicit allowlisted environment for every Go subprocess and pass that
certified environment into `packages.Load`. Normalize and bind every retained
resolution/cache/proxy/VCS input and selected external executable, or force a
deterministic value. Add controlled proxy/module-cache fixtures that produce
error versus resolved facts and require identity rotation or identical sealed
behavior.

### REL-02 (P1): Go subprocess deadlines and allocation bounds are incomplete

**Files:** `go/semantic/process.rs:1377,1547,1990`,
`go/semantic/client.rs:135-177`, `go/semantic/protocol.rs:139`

`go version` and `go build` use unbounded `Command::output()`. Runtime pipe
readers use unbounded `read_to_end`, protocol decoding accumulates unbounded
rows and fields, and the timeout path joins reader threads after only the direct
child is handled. A descendant retaining a pipe can block forever; large output
can exhaust memory; source preparation can hang before the timed client starts.

Route probes, builds, and semantic execution through a single bounded runner
with a deadline that remains active until process exit and pipe EOF, total and
per-stream byte ceilings, NDJSON line/row/field limits, and complete process-tree
cleanup. Test sleeping/spamming version and build commands, oversized semantic
streams, and a direct child that exits while a descendant retains both pipes;
all must return within a short deadline with bounded allocation and no survivor.

## Provider Trust and Rule Dispatch

### WR-26 (P1): Failed global fact validation does not revoke capabilities

**Files:** `analysis_kernel/mod.rs:1193-1203,1359-1389`,
`analysis_kernel/validation.rs:325-379`, `runner/mod.rs:424-431`

Effective capability support is finalized before global fact validation. A
provider can execute successfully, install malformed CFG, call, refined-call,
or data-flow facts, then fail validation while its metadata remains
`NativeTrusted` and dependent rules still execute against invalid facts. This
is distinct from an execution failure: the provider returned success and only
the authoritative defense-in-depth validation rejected its output.

Validate before capability finalization, fold failed validation kinds into
effective availability, and downgrade the corresponding provider metadata.
CFG/call/refined-call failures must revoke calls and control flow; data-flow
failure must revoke dataflow. Add a full kernel/runner injection regression that
asserts the internal validation diagnostic, capability diagnostics, zero rule
invocations, non-trusted metadata, and cold/warm parity.

## Semantic Store Integrity

### WR-27 (P2): Publication can activate scalar data it never authenticated

**Files:** `analysis_kernel/store/generation.rs:675-877,3417-3523`,
`.planning/phases/65-generation-manifest-and-metadata-mirroring/65-17-PLAN.md:64`

After writing, the in-memory plan is released. `validate_written_generation`
checks storage shape, counts, declared child counts, copied statistics, and
validation events, but never decodes the persisted projection or recomputes its
canonical identities before marking it complete and rotating the active
pointer. A binding regression or transaction-local scalar tamper can therefore
activate an invalid candidate and displace the previous valid generation; only
the next reader detects the corruption and requests a rebuild.

Before completion and activation, run the pending candidate through the typed
projection decoder plus canonical identity/reference validation inside the same
transaction, or compare equivalent streaming family digests to the reservation
identities. Add a post-write scalar-tamper seam and prove commit fails, the
candidate never activates, and the previous complete generation remains ready.

### WR-28 (P2): Ordinal validation accepts value-preserving swaps

**File:** `analysis_kernel/store/generation.rs:3247-3307,3722-3747,4579-4623,4797-4876`

The new validator orders only by ordinal and proves that every partition
contains the set `{0..n}`. It does not authenticate which canonical row owns
each ordinal. Swapping `0` and `1` between two rows preserves the contiguous
set, while downstream readers sort or canonicalize content again, so identities
still match and the tampered store is accepted.

Authenticate ordinal-to-content association: order each partition by canonical
semantic columns and require the stored ordinal to equal the enumerated index,
or preserve raw ordinal order and require it already matches the canonical row
sequence. Extend the tamper matrix with two-row swaps in a global family and
partitioned child families; active reads and identical-generation validation
must fail closed.

## Clean Areas

The fourth review found no additional defect in transient syntax-cache warning
handling, exact extension dependency scoping, execution-failure capability
revocation, active-pointer revalidation, all-table storage preflight, row/byte
budget symmetry, cache create-or-validate security, private frontend cache
ownership, content framing, supported SDK/CLI visibility, or Cargo/MSRV
compatibility.

---

_Reviewed: 2026-07-15T20:28:43Z_
_Review mode: fresh three-domain post-fix deep review_
