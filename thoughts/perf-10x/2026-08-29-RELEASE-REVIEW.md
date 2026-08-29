# polint v0.3.2 release-readiness review

Date: 2026-08-29  
Branch: `feat/machine-global-rule-host-store`  
Reviewed base: `origin/main` at `9af105f9` (the local `main` ref was stale)  
Reviewed feature commits: `4031a0b7`, `78986ce9`, `8d3fbc1e`, `a962036c`  
Review-fix commit: `c56d69de`

## Decision

**Do not merge or release this as v0.3.2 yet.** The implementation is much safer
after `c56d69de`, and every requested gate passes, but the content key still does
not name checkout-specific compile-time path inputs that Cargo makes available to
Rust code and build scripts. Two byte-identical checkouts can therefore compute
the same key while compiling behaviorally different hosts. That contradicts the
store's core correctness claim, not merely a performance or documentation claim.

## Findings by severity

### CRITICAL — open release blocker

#### C1. Checkout-specific Cargo compile-time paths are absent from the key

Status: **WONTFIX in this review; blocks release.** A safe fix requires an
architectural decision, not a source-text heuristic.

Evidence:

- The build and run paths set `CARGO_TARGET_DIR` to a checkout-specific cache
  location (`crates/polint/src/cli/mod.rs:4492-4504`), while the fingerprint
  deliberately excludes `CARGO_TARGET_DIR`
  (`crates/polint/src/cache/rules_store.rs:658-667`).
- The key uses a repository-relative package label and package-relative input
  names (`crates/polint/src/cache/rules_store.rs:839-853` and
  `crates/polint/src/cache/rules_store.rs:859-896`), so two checkouts with the
  same bytes intentionally receive the same key.
- Cargo exposes checkout-specific absolute values such as
  `CARGO_MANIFEST_DIR` and `OUT_DIR` during compilation. Rule code can compile
  `env!("CARGO_MANIFEST_DIR")` into the host, and a build script can emit or
  embed `OUT_DIR` or other absolute paths. Hashing the source and `build.rs`
  bytes does not hash the values those inputs observe.
- The store explicitly permits different binary bytes for one fingerprint and
  lets concurrent publishers race on the entry
  (`crates/polint/src/cache/rules_store.rs:409-417`). That is safe only if all
  possible outputs are behaviorally interchangeable; a compiled manifest or
  output-directory path disproves that premise.
- The public documentation currently calls the enumerated inputs the
  “complete input surface” (`README.md:240-250`), so this is also a truthfulness
  defect in the shipped contract.

Impact: a host restored in checkout B can contain checkout A's manifest/target
path and behave differently from the host `cargo run` would compile in B. The
binary passes the stored SHA-256 check because it is exactly the binary the store
published; byte verification cannot detect that the key was incomplete.

Why it was not patched here: including these paths (even only as digests) makes
the key checkout-specific and eliminates the feature's cross-checkout reuse.
Scanning source text for `env!` is unsound because macros, proc macros, and build
scripts can construct the same behavior indirectly. A release-quality fix must
choose and enforce one of these designs:

1. compile in a deterministic, machine-global build root and normalize or forbid
   path-observing compile-time inputs;
2. include the actual checkout/build paths in the key and accept that these
   packages are local-only; or
3. define and enforce a narrower, mechanically provable class of shareable rule
   packages.

Before release, add an end-to-end regression with two byte-identical checkouts
whose host reports `env!("CARGO_MANIFEST_DIR")`, plus a build-script/`OUT_DIR`
case. Both must either produce distinct keys or be rejected by the shareability
gate.

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
  This is blocker C1.
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

Answer: **NO**, because C1 remains even though the enumerated static inputs are
now covered.

### B. Gate soundness

Workspace manifests, nested/build-dependency manifests, absolute paths, and
canonicalized symlink escapes are covered. Cargo is invoked with the repository
root as cwd, so the applicable config chain is repo root plus its ancestors and
Cargo home; the rule package config is additionally checked conservatively. An
intermediate `.polint/.cargo` directory is not a Cargo config ancestor of that
cwd and cannot affect this invocation. Includes are rejected rather than chased.

Answer: **YES for declared dependency/config redirects after `c56d69de`; NO for
the broader shareability proof because C1 is outside this gate.**

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

Answer: **YES for process invocation and failure rendering after `c56d69de`, but
overall NO until C1 guarantees the restored program is the program this checkout
would compile.**

### F. Stamp correctness

The key includes polint version and normalized profile; the stamp also records
schema, fingerprint, target-relative path, binary length, and SHA-256
(`crates/polint/src/cache/rules_store.rs:294-322`). Version downgrade/profile
switches miss. Stamp writes atomically replace old stamps. The target lock
serializes writers. A SHA-256 collision remains a standard cryptographic
assumption, not a realistic stale-stamp scenario.

Answer: **YES, subject to fingerprint completeness; C1 can make a perfectly
valid stamp identify the wrong checkout semantics.**

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
and byte-identical fallback behavior. The exact focused suite runs 28 tests.

Test-thin claims:

- no regression for checkout-specific compile-time values (`CARGO_MANIFEST_DIR`
  / `OUT_DIR`) — this is the release blocker;
- no true multiprocess target/store contention test;
- no Windows execution in this environment;
- the warm test proves no Cargo build/run, while deliberately allowing
  `cargo -V`; it does not prove “no Cargo process.”

## Gate results

All commands ran on `c56d69de`; none was pushed.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo test -p polint --lib cache::rules_store` | PASS — 28 passed, 0 failed |
| Extra: `cargo test -p polint --test rule_host_store` | PASS — 1 passed, 0 failed |

The branch is five commits ahead of `origin/main` after the review fix. No push
was performed.

RELEASE-READY: NO — checkout-specific Cargo compile-time paths are not represented in the supposedly complete cross-checkout key
