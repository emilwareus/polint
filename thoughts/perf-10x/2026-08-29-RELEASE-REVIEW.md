# polint v0.3.2 release-readiness review

Date: 2026-08-29  
Branch: `feat/machine-global-rule-host-store`  
Reviewed base: `origin/main` at `9af105f9` (the local `main` ref was stale)  
Reviewed feature commits: `4031a0b7`, `78986ce9`, `8d3fbc1e`, `a962036c`  
Review-fix commit: `c56d69de`
Blocker-fix commit: `fix(cache): refuse to share rule hosts that embed checkout paths`

Line references in the earlier fixed findings describe the reviewed `c56d69de`
snapshot; the C1 follow-up is described by symbol and behavior below.

## Decision

**The machine-global rule-host store is ready to merge for v0.3.2.** The final
blocker is closed by conservatively opting any package whose Rust sources may
embed Cargo-provided checkout paths out of restore and publication. The gate
preserves the store's rule that a surface which cannot be proven is never
shared, and the complete requested gate suite passes.

## Findings by severity

### CRITICAL — fixed release blocker

#### C1. Checkout-specific Cargo compile-time paths could escape the key

Status: **FIXED.** A conservative byte-token gate now makes affected packages
local-only in both directions. False positives are intentional: they cost a
local build but cannot wrong-share a host.

Evidence:

- The build and run paths set `CARGO_TARGET_DIR` to a checkout-specific cache
  location (`crates/polint/src/cli/mod.rs:4492-4504`), while the fingerprint
  deliberately excludes `CARGO_TARGET_DIR`
  (`crates/polint/src/cache/rules_store.rs:658-667`).
- The key uses a repository-relative package label and package-relative input
  names (`crates/polint/src/cache/rules_store.rs:839-853` and
  `crates/polint/src/cache/rules_store.rs:859-896`), so two checkouts with the
  same bytes intentionally receive the same key.
- Cargo exposes checkout-specific values such as `CARGO_MANIFEST_DIR`,
  `OUT_DIR`, `CARGO`, and `RUSTC` during compilation. The shareability decision
  now scans every `*.rs` under `src`, `benches`, `examples`, and `tests`, plus a
  root `build.rs`, for the minimal byte-token family that can name those values.
- The scan is intentionally lexical rather than a claim of Rust semantic
  coverage. A match in a comment or inert string disables sharing; this follows
  the store's fail-closed rule. `CARGO_PKG_*` is deliberately excluded because
  Cargo derives those values from checkout-independent package metadata.
- The gate is evaluated once before consulting the store and the same result
  controls publication after a build. A matched package therefore neither
  restores another checkout's host nor publishes its own.
- The fingerprint records `embeds_checkout_paths=true|false`, so changing the
  gate state is also an explicit component of build identity.
- Unit coverage includes `env!("CARGO_MANIFEST_DIR")`, an `OUT_DIR` token in a
  doc comment, a token-free package, and a root `build.rs` that expands
  `include!(concat!(env!("OUT_DIR"), ...))`. Additional coverage pins the
  `CARGO`/`RUSTC` executable names and the `CARGO_PKG_*` exclusion.

Impact after the fix: packages that might compile a checkout path run through
the original local Cargo path. Packages without those tokens retain
cross-checkout reuse, with the existing fingerprint, restore-integrity, and
same-user trust guarantees.

### HIGH — fixed in `c56d69de`

#### H1. The fingerprint omitted compiler/environment/config/package inputs

Status: **FIXED.** `RUSTC`, `RUSTC_BOOTSTRAP`, Cargo profile/target overrides,
the Cargo-home location, ancestor/Cargo-home configs, ancestor workspace
manifests and lockfiles, legacy `rust-toolchain`, non-Rust package inputs, and
non-UTF-8 file bytes are now covered or fail closed
(`crates/polint/src/cache/rules_store.rs:97-125`,
`crates/polint/src/cache/rules_store.rs:626-677`,
`crates/polint/src/cache/rules_store.rs:755-856`, and
`crates/polint/src/cache/rules_store.rs:869-956`). The `RUSTC` program is also
the compiler whose identity is probed (`crates/polint/src/cli/mod.rs:4427-4442`).

Tests cover compiler/profile overrides, ancestor and Cargo-home config, Cargo
home relocation, all package files, and raw non-UTF-8 bytes
(`crates/polint/src/cache/rules_store.rs:1462-1671`).

#### H2. The shareability gate missed mutable and redirected inputs

Status: **FIXED.** Absolute path dependencies are now local-only because their
contents have no lockfile checksum. Workspace and nested manifests are scanned;
relative targets are canonicalized so symlink escapes fail; unreadable or
non-regular manifests fail closed
(`crates/polint/src/cache/rules_store.rs:1066-1167`).

Cargo configs are considered from the rule package, repository ancestors, and
Cargo home (`crates/polint/src/cache/rules_store.rs:1239-1261`). The gate now
rejects `patch`, `replace`, `paths`, `source` replacement, `include`, `[env]`,
and target runners, and treats unreadable/unparseable/non-file configs as unsafe
(`crates/polint/src/cache/rules_store.rs:118-125` and
`crates/polint/src/cache/rules_store.rs:1278-1312`). `include` is intentionally
not chased: any include disables sharing in both directions.

Regression coverage is at
`crates/polint/src/cache/rules_store.rs:1695-1875`, including workspace path
dependencies, symlink escapes, absolute dependencies, config source
replacement, runtime environment, runner, include, and unreadable inputs.

#### H3. Restore/stamp publication was not Windows-safe and target use raced

Status: **FIXED.** PID-only temporary names and `rename`-over-existing were
replaced with randomized same-directory `NamedTempFile` persistence. Restores
copy to a temporary, re-hash length plus SHA-256, set executable mode on Unix,
then atomically replace the destination
(`crates/polint/src/cache/rules_store.rs:351-406`,
`crates/polint/src/cache/rules_store.rs:486-545`). Stamps use the same atomic
writer (`crates/polint/src/cache/rules_store.rs:325-339`).

An exclusive target-directory lock is held across verification, restore/build,
and host execution (`crates/polint/src/cache/rules_store.rs:133-155` and
`crates/polint/src/cli/mod.rs:4329-4372`). Store publishers remain safe because
blobs are content-addressed and entry replacement is atomic. Crash leftovers are
unreferenced randomized temporary files; no partial entry, stamp, or destination
becomes visible.

The cross-platform replacement regression is
`crates/polint/src/cache/rules_store.rs:1907-1938`; corrupt-byte rejection is
`crates/polint/src/cache/rules_store.rs:1964-1990`.

#### H4. Speculative and direct-exec failures changed public behavior

Status: **FIXED.** A speculative `cargo build` failure is discarded and the
original `cargo run` path executes (`crates/polint/src/cli/mod.rs:4300-4317` and
`crates/polint/src/cli/mod.rs:4506-4548`). A direct spawn failure is a miss, and
a directly started host that exits nonzero also re-enters Cargo so Cargo's own
failure diagnostic is preserved (`crates/polint/src/cli/mod.rs:4591-4610`).

The end-to-end test compares status, stdout, and stderr byte-for-byte for a
nonzero direct host, a direct spawn failure, and a failed speculative build
against the original Cargo-only path
(`crates/polint/tests/rule_host_store.rs:238-330`).

#### H5. Cargo configuration could change direct-run semantics

Status: **FIXED.** `[source] ... replace-with` was not caught by the original
four-key gate. `[env]` and target `runner` also affect `cargo run` but are not
automatically reproduced by `Command::new(binary)`. All three are now
local-only (`crates/polint/src/cache/rules_store.rs:118-125` and
`crates/polint/src/cache/rules_store.rs:1278-1312`), with table-driven tests at
`crates/polint/src/cache/rules_store.rs:1794-1814`.

### MEDIUM — accepted for the next design revision

#### M1. “Warm path skips Cargo entirely” is not literally true

Status: **WONTFIX for this patch; non-blocking performance/wording issue.** The
stamp path still starts `rustc -vV` and `cargo -V` to resolve floating toolchain
identity before validating the stamp (`crates/polint/src/cli/mod.rs:4344-4372`
and `crates/polint/src/cli/mod.rs:4398-4442`). It skips Cargo **build/run**, not
all Cargo processes. The implementation comments and tests now say that
accurately (`crates/polint/src/cache/rules_store.rs:282-288` and
`crates/polint/tests/rule_host_store.rs:111-127,223-235`). Removing the probe
without a replacement identity guard would make a floating toolchain upgrade
reuse a stale binary.

#### M2. Concurrency and Windows execution coverage is thinner than the claims

Status: **WONTFIX in this review; test gap, not a known code defect.** Atomic
replacement is covered by a platform-neutral unit test and the Windows root
resolution regression from `a962036c` is retained
(`crates/polint/src/cache/rules_store.rs:1416-1423` and
`crates/polint/src/cache/rules_store.rs:1907-1938`). There is no real
two-process contention test, and no Windows target/runner was installed in this
environment (`rustup target list --installed` reported only
`x86_64-unknown-linux-gnu`). Windows CI must run before any eventual release.

### LOW / release mechanics

#### L1. The workspace still says 0.3.1

Status: **WONTFIX here; expected release workflow behavior.** `Cargo.toml:37`
is still `0.3.1`. The release workflow accepts a patch bump and performs the
workspace version commit before tagging (`.github/workflows/release.yml:8-17`
and `.github/workflows/release.yml:54-98`). If the correctness blocker is fixed,
merge first and invoke that workflow with `patch` to produce 0.3.2.

## Checklist A–H

### A. Fingerprint completeness

- **Features:** no feature flags are passed by either build or run; Cargo's
  default-feature selection is determined by the hashed manifests
  (`crates/polint/src/cli/mod.rs:4525-4533,4625-4630`).
- **Target directory:** the ambient value is correctly ignored because polint
  overwrites it on both paths, but the overwritten value itself is
  checkout-specific and can enter the binary through Cargo compile-time values.
  The C1 token gate now makes packages that name those values local-only.
- **RUSTC/toolchains/flags:** fixed and covered as described in H1.
- **Cargo home/config resolution:** location and applicable config contents are
  hashed; unsafe configs fail the gate.
- **Bins:** explicit `[[bin]]` names and the autodiscovered package-name fallback
  are derived at `crates/polint/src/cache/rules_store.rs:959-985`; the manifest
  and every possible source path are also hashed.
- **Profile case:** `dev`/`debug` and case-insensitive `release` normalize to the
  exact Cargo invocation, while custom profile spelling is preserved
  (`crates/polint/src/cli/mod.rs:4750-4802`).
- **Non-UTF-8:** file contents are streamed as bytes; non-UTF-8 input bytes are
  tested. A non-UTF-8 relative path or relevant environment value disables the
  store rather than colliding (`crates/polint/src/cache/rules_store.rs:649-677`
  and `crates/polint/src/cache/rules_store.rs:880-896`).

Answer: **YES.** The enumerated static inputs are covered, and packages that
name checkout-specific Cargo values are refused cross-checkout sharing.

### B. Gate soundness

Workspace manifests, nested/build-dependency manifests, absolute paths, and
canonicalized symlink escapes are covered. Cargo is invoked with the repository
root as cwd, so the applicable config chain is repo root plus its ancestors and
Cargo home; the rule package config is additionally checked conservatively. An
intermediate `.polint/.cargo` directory is not a Cargo config ancestor of that
cwd and cannot affect this invocation. Includes are rejected rather than chased.

Answer: **YES.** Declared dependency/config redirects and checkout-path source
tokens all fail closed before restore or publication.

### C. Restore safety

Length plus SHA-256 are checked on a private temporary before atomic persistence;
bad blobs and entries are deleted. Per-target locking closes the normal
verify/replace/execute race between polint processes. Unix restore sets `0755`;
non-Unix needs no executable bit. Atomic files prevent partial visible state on
crash. A same-user actor can still mutate files after verification, but the store
explicitly has same-user Cargo-registry trust (`crates/polint/src/cache/rules_store.rs:30-44`).

Answer: **YES under the documented same-user trust model.**

### D. Failure degradation

Store/fingerprint/lock/restore operations use `Option`, ignored best-effort
results, or errors swallowed by the caller. Speculative build and direct-exec
failures re-enter the original path. No production `unwrap`/`expect` was added in
the store module; occurrences after `#[cfg(test)]` are test-only.

Answer: **YES.**

### E. Behavior identity

Both direct and Cargo paths use `apply_local_rule_host_env`, which supplies
`POLINT_CACHE_DIR`, the pinned `CARGO_TARGET_DIR`, and the
`POLINT_RULES_TOOLCHAIN` to `RUSTUP_TOOLCHAIN` mapping
(`crates/polint/src/cli/mod.rs:4492-4504,4599-4602,4625-4630`). Ambient
`POLINT_RULES_PROFILE`, `RUSTUP_TOOLCHAIN` when not overridden, `POLINT_CARGO`,
and other environment variables are inherited by both. Cargo-config `[env]` and
runner cases are now gated out. Nonzero and spawn/build failure byte identity is
tested.

Answer: **YES.** Process invocation and failure rendering are identical, and C1
packages now stay on the local Cargo path.

### F. Stamp correctness

The key includes polint version and normalized profile; the stamp also records
schema, fingerprint, target-relative path, binary length, and SHA-256
(`crates/polint/src/cache/rules_store.rs:294-322`). Version downgrade/profile
switches miss. Stamp writes atomically replace old stamps. The target lock
serializes writers. A SHA-256 collision remains a standard cryptographic
assumption, not a realistic stale-stamp scenario.

Answer: **YES.** The fingerprint records the checkout-path gate state, and a C1
package never consults or publishes to the machine-global store.

### G. Windows

Store roots use `%LOCALAPPDATA%`; recorded paths normalize `\\` to `/` and are
validated as relative components. Randomized temp files avoid PID collisions;
`NamedTempFile::persist` can replace existing destinations on Windows, unlike
the old plain rename path. Executable permission changes are Unix-only. The
`a962036c` absolute override test fix is present.

Answer: **source review passes, local runtime validation unavailable; require
Windows CI.**

### H. Tests

Strong coverage now pins key-field changes, raw bytes, all package inputs,
ancestor/Cargo-home config, path/workspace/symlink gates, corrupt restore,
atomic replacement, stamp tampering, cross-checkout publish/restore, warm stamp,
and byte-identical fallback behavior. The focused suite also covers the C1 token
gate in normal sources, comments, build scripts, and its minimal variable set.

Test-thin claims:

- no true multiprocess target/store contention test;
- no Windows execution in this environment;
- the warm test proves no Cargo build/run, while deliberately allowing
  `cargo -V`; it does not prove “no Cargo process.”

## Gate results

All commands ran after the C1 blocker fix; none was pushed.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo test -p polint --lib cache::rules_store` | PASS — 32 passed, 0 failed |
| Extra: `cargo test -p polint --test rule_host_store` | PASS — 1 passed, 0 failed |

The branch contains the review hardening and the C1 blocker fix. No push was
performed.

RELEASE-READY: YES — checkout-path-embedding rule hosts are local-only, and shareable hosts retain a complete cross-checkout key
