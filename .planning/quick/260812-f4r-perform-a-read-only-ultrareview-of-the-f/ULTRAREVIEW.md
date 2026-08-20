# Whole-branch architecture ultrareview

**Review target:** `static-analysis-architecture-review`  
**Range:** `1263208a11d66b2b31bd4fac11f4cb46fdccec13...40f7ec123c0796810b2ed0a497d68b84ca2f70c7`  
**Scale:** 196 commits, 1,116 files, +161,224/−83,321 lines  
**Mode:** read-only product review; no product fixes, pushes, PRs, merges, worktrees, or remote CI  
**Verdict:** **NOT READY TO SHIP**

## Executive verdict

The branch demonstrates a large amount of successful migration work: the intended eight product crates exist with the documented dependency direction, the supported facade paths and 116-item prelude remain present, all 17 example packages remain wired, the recorded final branch-tip suite is green, and focused Go and TS crate tests pass. The split is real rather than a Cargo-only shell.

The branch nevertheless must not be pushed for review as “ready to ship” yet. This review verified correctness defects in the call graph and provider lifecycle, a repository-write escape in `new-rule`, stable-payload violations, and Go subprocess/cache integrity gaps. It also found incomplete physical ownership in the nominally completed split. Separately, current `main` has advanced by three commits and a read-only merge simulation produces 13 conflicts, including two modify/delete conflicts in moved analysis/Go provider files. The recorded Q6 evidence proves the pre-integration tip only.

Severity summary after orchestrator verification and deduplication:

| Severity | Count | Summary |
| --- | ---: | --- |
| Blocker | 1 | Current `main` cannot be integrated without conflict resolution and a new full gate run |
| High | 8 | Call-ID collisions; provider digest/store inconsistency; missing metadata lifecycle; `new-rule` symlink escape; Go executable-cache integrity, offline, and timeout failures; CI deny mismatch |
| Medium | 8 | Four stable-identity/digest defects; two residual ownership violations; partial `new-rule` writes; stale Go CI cache paths |

The report intentionally rejects several first-pass candidates: pre-existing `new-rule` keyword handling and `sdk::__private` are not branch regressions; `#[non_exhaustive]` changes are deliberate beta contract hardening already required by the binding migration; ignored tests are transparently recorded and separately exercised in CI; and the golden timing assertion is compatible with an orchestrator-level single retry/waiver even though that policy is not encoded in one test process.

## Review method and evidence

Eight independent tracks reviewed public API, crate architecture, identity/cache/determinism, Go, TS/JS, neutral analyses, CLI/security, and validation/documentation. Candidate findings were then compared against exact HEAD source and, where relevant, the merge base and current `main`. Temporary external consumer/security probes lived under `/tmp`; no product files were edited.

Validation evidence available to this review:

- committed Q6 log for tested SHA `4e563aabc7d01bc605c39d676938603ad96766ea`;
- workspace result: 2,424 passed, 4 ignored;
- public surface 8/8; golden 8/8 byte-identical; determinism 12/12; polyglot 2/2;
- clippy, rustdoc, all examples, dependency checks, and supported cargo-deny invocations green;
- focused rechecks: `cargo test -p polint-go --lib --locked` (129 passed), `cargo test -p polint-ts --lib --locked` (162 passed), and public-surface 8/8;
- `cargo metadata --no-deps --format-version 1` and source forbidden-import checks;
- read-only `git merge-tree --write-tree main HEAD` (exit 1, conflicts reported).

The Q6 run is strong regression evidence, but it cannot cover unforced replacement failures, adversarial symlinks/temp caches, hanging external commands, or an unbuilt merge result.

# Verified findings

## B-1 — Blocker — current `main` produces 13 integration conflicts; all readiness evidence becomes stale after resolution

**Evidence:** `git merge-tree --write-tree main HEAD` exits 1 against current `main` `fafd08d8af78d78313fdd61ff616a887652ac0ab`. Conflicts occur in:

- `.planning/STATE.md`;
- `crates/polint-go/src/adapter.rs` and `crates/polint-go/src/tests.rs`;
- `crates/polint/src/analysis_kernel/incremental/{digest,run_report,stats}.rs`;
- `crates/polint/src/analysis_kernel/{metadata,mod,validation}.rs`;
- `crates/polint/src/core/mod.rs` and `crates/polint/src/metrics.rs`;
- modify/delete conflicts for `crates/polint/src/analysis/cfg/provider.rs` and `crates/polint/src/go/semantic/provider.rs`.

`main` added the Phase 65 generation-manifest/sealed-provider-outcome work after the review merge base. The conflict set is not administrative: it touches the exact provider outcome, digest, metadata, validation, cache, adapter, and moved-file seams this branch changes. Choosing either side mechanically can drop Phase 65 behavior or reintroduce old monolithic ownership.

**Gate blind spot:** `.swarm/READY-TO-SHIP.md:11-19` accurately records Q6 against the pre-merge parent `4e563aab`; no branch-local test proves a future resolved tree.

**Required action:** reconcile with current `main` first, port the three main commits into their new crate owners, resolve the two modify/delete conflicts semantically, and rerun every Q6 gate on the reconciled SHA. Replace the readiness record and log; never treat the current log as evidence for that new tree.

## H-1 — High — call-site IDs collide across files and languages, corrupting joins

**Locations:**

- `crates/polint-go/src/mir/lower.rs:1594-1600`;
- `crates/polint-ts/src/mir/lower.rs:4375-4381`;
- `crates/polint-analysis/src/mir_body_compose.rs:184-251`;
- `crates/polint-analysis/src/calls/extract.rs:79-96`;
- `crates/polint-analysis/src/calls/store.rs:65-92,112-135`.

Go constructs `CallSiteId` from `node.start_byte()` and TS/JS from `(span.start << 32) | span.end`. Both are file-local coordinate schemes. `lower_go_mir` and `lower_ts_mir` each traverse multiple files in one lowering instance, so two files with a call at the same byte span immediately receive the same ID. Language-output composition remaps body/block/place/operation families but leaves `MirOperationKind::Call.site` and `MirValue::CallReturn` unchanged. Call extraction copies that ID directly. `CallStore` builds a set/map but does not reject duplicate site IDs; targets for unrelated sites become grouped and the owner-symbol map can be overwritten.

**Concrete trigger:** two Go files containing a call at the same byte offset, two similarly shaped TS files, or an equal encoded site across composed language outputs. Calls/refined calls/data flow can associate a target or owner with the wrong callsite.

**Why gates miss it:** lowering tests are largely single-file; composition tests assert ordinary MIR ID remapping, not call-site references; store tests reject dangling sites but not duplicate sites.

**Minimal fix:** assign call-site IDs globally after stable `(path/body/span/kind)` ordering, remap every call and call-return reference, and make `CallStore` reject duplicate IDs. Add two-file Go, two-file TS, and polyglot collision tests through the real provider path.

## H-2 — High — failed providers publish a digest for output that was not stored

**Locations:**

- `crates/polint/src/analysis/provider.rs:48-58`;
- `crates/polint-analysis/src/{cfg,calls,identity,entrypoints,refined_calls}/provider.rs` error arms (respectively `49-59`, `72-82`, `68-78`, `50-60`, `130-140`);
- `crates/polint/src/analysis_kernel/mod.rs:182-192,374-393`.

Each listed provider computes an output digest, attempts a validated store replacement, emits a diagnostic when replacement fails, but still returns `Some(output_digest)`. The kernel inserts every `Some` into `upstream_digests` and labels it `native_trusted`. Since replacements build the new store before assignment, failure preserves the prior/empty facts: downstream providers receive a digest for candidate facts that are not in the database.

**Concrete trigger:** malformed MIR or any invalid/dangling provider output rejected by `from_output`. The run can continue using stale facts under a new trusted digest, invalidating cache correctness and downstream joins.

**Why gates miss it:** successful provider and store-rejection tests are separate; no test forces the provider replacement error and asserts outcome/digest/dependency blocking. Data-flow, evidence, reachability, semantic-graph, and solver already use the correct `None` discipline, which makes the inconsistency clear.

**Minimal fix:** make every failed replacement return `None`, generalize the kernel's `provider_failed` outcome rather than special-casing only two providers, block dependents on missing required output, and test each forced-invalid path. Port the fix onto current `main`'s sealed `ProviderOutcome` model during reconciliation rather than rebuilding the old optional-digest protocol.

## H-3 — High — moved neutral provider replacements omit required fact metadata lifecycle

**Locations:**

- missing refresh after replacements in `crates/polint-analysis/src/host.rs:297-318,500-600,627-638`;
- complete facade lifecycle retained but bypassed at `crates/polint/src/core/db.rs:955-960,1043-1066,1770-1849,2472-2517`;
- missing metadata is a declared validation error at `crates/polint/src/analysis_kernel/validation.rs:95-123,3378-3392`;
- release validation is opt-in at `validation.rs:41-65`.

The generic `AnalysisHost` path correctly refreshes summaries, CFG, calls, refined calls, and abstract domains after commit `28797238`, but it does not refresh MIR, entrypoints, data flow, evidence, reachability, solver, type/value/access-path/points-to/alias, semantic graph, or identity metadata. Moved providers are generic over `AnalysisHost`, so method resolution uses these default lifecycle methods instead of the facade's inherent replacements, where several refresh implementations still exist. This creates facts without corresponding `FactMeta` and can retain stale family metadata on subsequent replacement.

**Concrete trigger:** execute a non-empty scheduled provider in debug/`POLINT_VALIDATE_FACTS=1`, or consume metadata in a normal release run. Validation reports missing rows; without opt-in, metadata-dependent debug, evidence, validation, and ownership behavior sees incomplete state.

**Why gates miss it:** the acceptance repair covered a subset of families, inherent facade tests exercise a different path, and release skips whole-DB validation. Existing goldens need not request every affected provider/family.

**Minimal fix:** make each neutral replacement an atomic fact-plus-metadata lifecycle or add a neutral post-commit metadata hook implemented by the host. Delete/bypass no duplicate inherent lifecycle until parity is tested. Add a non-empty scheduled-kernel test that asserts zero missing/stale metadata across every moved family.

## H-4 — High — `new-rule` follows symlinked output parents and writes outside the repository

**Locations:** `crates/polint/src/cli/mod.rs:767-783` and fixture writes at `1056-1072`.

`new_rule` uses direct `create_dir_all`/`write` below `.polint/rules/src` and `.polint/tests/rules`, unlike repository filesystem helpers and the hardened `add-skill` path. Existing symlink parents are followed.

**Verified trigger:** in a temporary repo, point `.polint/rules/src` at an outside directory and run `polint new-rule`; it succeeds and writes the generated module and `main.rs` outside the repo. A symlinked fixture parent has the same property.

**Why gates miss it:** rule-name traversal and `add-skill` symlinks are tested; `new-rule` parent symlinks are not.

**Minimal fix:** preflight every path component through the no-symlink repository filesystem layer and use atomic/no-follow creation. Test both rules and fixture parent symlinks and assert outside targets stay untouched.

## H-5 — High — predictable embedded Go sidecar cache authorizes executable content with only `.complete`

**Locations:** `crates/polint-go/src/semantic/process.rs:99-172,325-363` and `crates/polint-go/src/symbol_graph.rs:450-459` (same materialization pattern).

The cache path is predictable under `temp_dir()/polint-go-{frontend,symbols}/<version>/<embedded-hash>`. If `.complete` exists, reuse skips embedded source verification; semantic execution also accepts an already present cached binary. The competing-publisher rename path accepts any destination with the marker.

**Concrete trigger:** another process able to write that temporary hierarchy (typically same-user malware/process on macOS, potentially another user on a permissive shared temp setup) pre-seeds `.complete` and an executable. The next polint run executes it with repository access.

**Why gates miss it:** normal materialization and source drift are tested; preseeded markers, binary hashes, ownership, and competing publishers are not.

**Minimal fix:** use a private per-user cache with restrictive permissions, exclusive/atomic publication, and verification of the full embedded source and executable digest/ownership on reuse. A marker must never be an authorization primitive.

## H-6 — High — Go `offline = true` does not cover cold embedded semantic-sidecar compilation

**Locations:** `crates/polint-go/src/semantic/client.rs:68-76` and `crates/polint-go/src/semantic/process.rs:325-363`.

Offline variables are applied to the analysis request only after `command_for_frontend` returns. In source fallback, that function first runs an unbounded `go build` with `GOWORK=off` and `GOTOOLCHAIN=local`, but without `GOPROXY=off`/checksum-network controls.

**Concrete trigger:** no adjacent prebuilt sidecar, cold Go module cache, and `[languages.go] offline = true`. The first run can access or wait on the network before the offline request begins.

**Why gates miss it:** command-selection/protocol unit tests do not build the embedded frontend with an empty module cache under offline mode.

**Minimal fix:** thread lifecycle policy into build/materialization and apply offline variables to every Go command, or ship/vendor a complete prebuilt source path. Add an empty-cache test with network access made fatal.

## H-7 — High — Go module and symbol subprocesses can hang indefinitely

**Locations:** `crates/polint-go/src/module_graph/mod.rs:910-935` and `crates/polint-go/src/symbol_graph.rs:348-402`.

Both use `Command::output()` without a deadline or process-tree cleanup. Symbol source fallback additionally invokes `go run .`. The semantic client has a bounded runner, but these siblings do not.

**Concrete trigger:** unavailable proxy/VCS prompt, pathological `go list`/`packages.Load`, stuck toolchain, or child process. A check requesting module/symbol capability wedges indefinitely rather than emitting controlled setup diagnostics.

**Why gates miss it:** fake runners cover output parsing but not a sleeping process and descendants.

**Minimal fix:** route all Go builds/list/sidecars through one bounded process-group runner with concurrent output draining, timeout diagnostics, and descendant cleanup. Add sleeping-child tests.

## H-8 — High — checked-in CI passes cargo-deny flags rejected by the recorded tool interface

**Locations:** `.github/workflows/ci.yml:45-54`, `deny.toml:6-8`, and final log lines 2907-2935.

CI configures `cargo-deny-action@v2` with `command: check` and `arguments: --all-features --locked`. The recorded cargo-deny 0.19.4 rejects `--all-features`; direct usage shows `--locked` is also not a `check` option. The local Q6 log transparently records the rejected binding invocation, then proves `cargo deny check` and `cargo deny check all` green. That is valid local policy evidence, but not proof that the action's distinct configured argument contract succeeds.

**Concrete trigger:** the action forwards these arguments to a compatible recorded 0.19.x CLI, producing an argument error before policy checks. At minimum the job depends on undocumented action-side rewriting.

**Why gates miss it:** no local command exercises the action wrapper.

**Minimal fix:** use one documented supported invocation everywhere—`cargo deny check all`, with `[graph] all-features = true` in `deny.toml`—or pin and verify a tool/action pair supporting the intended flags. Execute the CI-equivalent command after reconciliation.

## M-1 — Medium — identity and reachability stable keys embed run-local `FileId`

**Locations:** `crates/polint-analysis/src/identity/facts.rs:164-181` and `reachability/facts.rs:155-179`.

Both recipes render `file_id.0` while comments and `ARCHITECTURE.md:260-280` state numeric allocation is not external identity. File IDs are assigned by insertion order. Adding/reordering a file changes stable identity and downstream digests for unchanged functions/roots.

**Minimal fix:** use canonical normalized repository-relative path in key construction and retain `FileId` only as an in-memory relation. Add allocation-order invariance tests.

## M-2 — Medium — reachability “stable” output payload serializes dense foreign IDs

**Location:** `crates/polint-analysis/src/reachability/provider.rs:200-220`.

Although the row's own `id` is excluded, `target_function`, `target_symbol`, `originating_entrypoint`, and `file` are serialized numerically. Equivalent semantic roots with remapped run IDs get different provider digests, contrary to the explicit D-19 comments and architecture contract.

**Minimal fix:** resolve all referenced entities to stable text (or omit fields redundant with the stable key) and test digest equality under complete ID remapping.

## M-3 — Medium — type/value/alias output digests serialize dense row and relation IDs

**Location:** `crates/polint-analysis/src/types/provider.rs:276-404` and digest structs starting at `407`.

Payloads include row IDs and dense `FunctionId`, `PlaceId`, `TypeSetId`, CFG/body/operation IDs, allocation IDs, and points-to/alias relations. Normalization makes ordinary producer runs deterministic, but the digest is not invariant to an equivalent stable graph with different allocation and violates the stated stable-text boundary.

**Minimal fix:** create explicit payload projections resolving row and referenced entity keys, omitting post-normalization handles. Test equivalent outputs under dense-ID remapping.

## M-4 — Medium — refined-call output digest includes dense edge and relation IDs

**Location:** `crates/polint-analysis/src/refined_calls/provider.rs:645-690`.

`RefinedCallEdgeDigest` includes `edge.id` plus dense site/target/function/symbol relations even though a resolved stable key is available. Finalization assigns `edge.id` before digesting, masking the defect in ordinary order tests but not satisfying persistence/cache invariance.

**Minimal fix:** digest resolved stable identities and semantic fields only; test remapped IDs.

## M-5 — Medium — TS-specific binding extraction/projection remains in the facade

**Location:** `crates/polint/src/analysis/semantic_graph/build.rs:38-58,218-338,417-710` (the module continues through line 2326).

The facade directly imports TS binding, inventory, object-model, scope, token-flow, and parsing implementation types, performs TS-specific collection, and projects them to the semantic graph. `ARCHITECTURE.md:286-290,321-340` assigns language enrichment/adaptation to `polint-ts` and reusable graph assembly to `polint-analysis`, leaving `polint` as composition root.

**Concrete cost:** a second host cannot reuse the pipeline without facade-private TS machinery, and TS semantic ownership remains split across three crates.

**Minimal fix:** move TS collection/projection into `polint-ts` behind neutral drafts; keep graph assembly in `polint-analysis` and only scheduling/downcast adapters in the facade. Add a structural check prohibiting facade semantic-graph imports of `crate::ts` implementation modules.

## M-6 — Medium — Go RTA algorithm remains facade-owned

**Locations:** `crates/polint/src/analysis/solver/go_rta/{mod,fixpoint,dispatch,inputs}.rs` (approximately 3,900 lines) and `analysis/solver/policy.rs`.

The facade owns the RTA worklist, dispatch, and input mapping, tied to facade Go facts and `AnalysisDb`, even though `polint-analysis` owns solver algorithms and `polint-go` owns Go adaptation. Compiler dependency direction passes, but physical ownership remains incomplete.

**Minimal fix:** put neutral RTA engine/input-output snapshot in `polint-analysis`, Go fact projection in `polint-go`, and leave only registration/composition in `polint`. Add a host-independent solver test and a source ownership check.

## M-7 — Medium — failed `new-rule` scaffolding leaves a broken partial rule pack

**Location:** `crates/polint/src/cli/mod.rs:767-798`.

The command updates Cargo/main/module before creating clean/violating fixtures. A later filesystem error returns failure but leaves those earlier mutations.

**Verified trigger:** make `.polint/tests/rules` a regular file. The command fails while `Cargo.toml`, `src/main.rs`, and the new module remain, referencing a scaffold with missing fixtures.

**Minimal fix:** preflight all paths and then install an entirely staged scaffold atomically, or implement complete rollback. Test failures at every write boundary.

## M-8 — Medium — Go CI cache keys still point at pre-split sidecar paths

**Location:** `.github/workflows/ci.yml:75-81,111-117,138-144,167-173`.

Every `setup-go` `cache-dependency-path` still names `crates/polint/go-sidecar/...`, while the files moved to `crates/polint-go/go-sidecar/...`. Sidecar dependency changes therefore do not participate in the intended Go cache key and missing-path behavior depends on the action version.

**Minimal fix:** update all four path lists and add a static workflow test that every declared dependency path exists.

# Architecture-area assessment

## Public SDK and macro surface

The supported `polint::sdk`, `sdk::prelude`, `runner`, and rule macro paths remain. The parsed prelude is exactly 116 unique entries and public-surface tests pass 8/8. No verified branch-created raw store leak or macro signature regression was found. `sdk::__private` is callable by downstream crates, but it already existed at the merge base as proc-macro plumbing; hardening it is worthwhile but not a defect introduced by this migration.

The addition of `#[non_exhaustive]` to public fact/report types is source-breaking for exhaustive matches and struct literals, but it is an explicit beta API decision required by this branch's public-contract repair. It should be called out prominently in release notes and ideally paired with constructors/builders; it is not categorized here as an accidental regression against the binding decision.

## Crate graph and physical ownership

Cargo metadata matches the eight binding product crates. `polint-core` is foundational; `polint-analysis-api` depends only on core/IR; `polint-frontend-api` names no concrete frontend; `polint-analysis` imports no frontend/facade; Go and TS do not import each other or the facade. Forbidden-import scans pass. The split is therefore compiler-enforced in dependency direction.

The two medium ownership findings show why that is necessary but not sufficient: facade-local TS semantic projection and Go RTA still violate the documented owner-by-input/invariant rule. These should be completed before calling W5.1 physically done.

## Identity, cache, and determinism

Interning structurally completed: production `stable_key: String` is zero, the prelude and golden output remain stable, and the seeded determinism suite passes. The four medium findings are narrower but important persistence-readiness gaps: stable recipes/payloads still serialize run-local identity. They may manifest today mainly as false cache misses because W5.2 persistence is out of scope, but they directly contradict the architecture and must be fixed before persistent artifacts make the format harder to change.

## Go frontend and lifecycle

The Go crate boundary, monorepo root selection, temp workspace lifecycle, path normalization, protocol framing, and semantic request timeout path are strong. Focused unit tests pass 129/129. The verified weaknesses are at external-process boundaries: executable cache authenticity, offline build coverage, and missing deadlines for module/symbol siblings. These are release-significant for a tool designed to run on untrusted repositories and in CI/agents.

## TypeScript/JavaScript frontend

No TS/JS-specific behavioral defect was verified in the bounded review. `cargo test -p polint-ts --lib --locked` passes 162/162. Parser recovery exposes conservative states, resolver/module graph code handles unresolved/dynamic/setup-missing outcomes explicitly, and the crate/facade adapter wiring is coherent. Residual risk remains in malformed recovery propagation, package exports/conditions/workspaces, symlink/path adversarial cases, cache mutation matrices, and complex CommonJS/re-export/declaration-merging fixtures. The facade ownership finding is about where TS projection lives, not a proven TS algorithm error.

## Neutral analyses and provider lifecycle

This is the highest-risk product area. The callsite collision is a direct semantic corruption. Digest-on-failed-store and missing metadata are lifecycle-contract failures created or exposed by the split. Ordinary MIR family offsets, source path ordering, solver normalization, budget diagnostics, and successful provider paths are well tested, but the error/commit boundary needs a single atomic abstraction rather than per-family conventions.

## CLI and security

Repository FS helpers, cache clean boundaries, `add-skill` symlink rejection, extension argument construction, bounded output, and timeout cleanup reviewed cleanly. `new-rule` bypasses those helpers, leading to the symlink escape and partial writes. Fix by consolidating all repo mutation behind the same transaction/no-follow layer.

## Validation, documentation, and release evidence

The final log is internally consistent and its checksum/tested-parent claims are honest. The architecture document accurately describes the target graph and most runtime invariants. The decisive gap is integration: current main's Phase 65 work conflicts exactly where the branch has moved provider contracts.

Four ignored tests in the workspace log do not by themselves invalidate Q6: they are reported rather than hidden, and CI has separate install/store jobs. Before an eventual push, however, execute and record the install smoke and serialized semantic-store check on the reconciled tip. Similarly, golden cost failures are subject to the binding one-retry timing policy; do not regenerate baselines or confuse a timing-only retry with diagnostic parity.

# Rejected or downgraded candidates

- **`new-rule` accepts Rust keywords/leading digits:** reproduced, but identical validation exists at the merge base and current main. Track separately, not as a branch regression.
- **`sdk::__private` permits manual raw rules/DB reads:** true and semver-visible despite `doc(hidden)`, but pre-existing and required by the current macro design. Treat as hardening/design debt.
- **`#[non_exhaustive]` breaks exhaustive external matches/literals:** true source compatibility impact, but this branch intentionally chose beta contract hardening and restored constructors for fact schemas. Report/match migration should be documented; not an accidental ship blocker under the binding decision.
- **Publishable internal crates expose broad modules:** a semver-maintenance risk. Cross-crate implementation currently requires much of this visibility, and supported rule-author surface remains the facade. Curate before independent crate stabilization, but do not mislabel it as current runtime breakage.
- **Golden cost code hard-fails before orchestrator waiver:** a single test process fails, but Q6 explicitly defines the retry/waive policy outside it. Automating the policy would reduce human error; current green run did not violate it.
- **Four ignored workspace tests:** transparent and covered by separate workflows; run them explicitly after reconciliation but do not call the recorded default test count false.

# Prioritized remediation plan

1. **Do not push. Reconcile current `main` locally** while preserving the eight-crate ownership model. Port Phase 65 sealed outcomes/generation metadata into new owners; never choose modify/delete sides mechanically.
2. **Fix provider atomicity as one design:** failed store means failed outcome/no digest/no dependent run; successful replacement means facts plus complete metadata committed together. Cover all families with a conformance test.
3. **Fix call-site identity globally** and add two-file/two-language collision fixtures plus store uniqueness validation.
4. **Close write/execute security boundaries:** make `new-rule` no-follow and transactional; move/verify embedded sidecar cache; apply offline policy to builds; use one bounded Go subprocess runner.
5. **Remove dense IDs from stable keys/digests** before persistence work begins. Add generalized ID-remapping property tests rather than one fixture per family only.
6. **Finish physical ownership:** TS-specific projection to `polint-ts`, RTA engine to `polint-analysis` with Go adaptation in `polint-go`.
7. **Repair CI configuration:** cargo-deny arguments and all moved Go cache dependency paths.
8. **Run complete local release evidence on the reconciled final SHA:** Q6, CI-equivalent cargo deny, install smoke, semantic-store ignored check, all examples, structural checks, and byte-identical golden diagnostics. Write a new READY record/log only then.

# Final release judgment

The branch is a substantial and largely successful architecture migration, but **READY-TO-SHIP must be withdrawn as a current release judgment**. The recorded evidence remains valuable for the isolated pre-integration tree. It does not cover the current-main merge result, and the verified call/provider/security defects are material. After the prioritized corrections and a clean full gate run on the reconciled tip, the branch should receive a shorter focused re-review of exactly these seams before any human opens a PR.
