# Two-crate consolidation and complete PR remediation plan

**Branch:** `static-analysis-architecture-review`  
**Target PR:** #96  
**Execution mode:** local implementation with intermediate commits; do not push until the complete branch is reviewed, fixed, and ready  
**Implementation agent:** native GPT-5.6 Luna at maximum reasoning effort  
**Coordinator rule:** every implementation tranche is inspected, tested, and independently reviewed before the next tranche begins

## 1. Objective

Replace the recently landed eight-package product graph with the product and stability model that polint actually intends to support:

- publish only `polint` and the required proc-macro companion `polint-macros`;
- keep `polint-bench` and `polint-eval` as unpublished workspace tooling;
- move core, IR, analysis contracts, frontend contracts, neutral analyses, and concrete Go/TypeScript frontends into private modules of `polint`;
- preserve the supported public rule-author contract at `polint::sdk`, `polint::runner`, and `polint::rule`;
- make internal contracts genuinely private and intentionally breakable rather than crates.io packages with externally reachable Rust APIs;
- preserve default behavior while adding optional per-language compilation through Cargo features;
- finish every verified whole-branch ultrareview finding and every newly discovered regression;
- finish with complete local release evidence and a branch that is ready for one final push.

This plan supersedes the eight-published-package release direction. Historical architecture/swarm records remain historical evidence and must not be rewritten to imply that the eight-crate split never occurred.

## 2. Non-negotiable invariants

### Product and API

- [ ] `polint::sdk`, `polint::sdk::prelude`, `polint::runner`, and `polint::rule` keep their supported paths and behavior.
- [ ] All example rule packs continue to behave as external consumers and import public SDK/runner surfaces only.
- [ ] Default builds retain both currently supported languages and byte-identical diagnostics unless a separately verified bug fix deliberately changes internal diagnostics.
- [ ] Internal engine/frontend/database/store APIs are private or `pub(crate)`; they are not compatibility promises.
- [ ] No compatibility shim or duplicate implementation is retained merely to imitate the removed package boundaries.

### Packaging

- [ ] Only `polint` and `polint-macros` are publishable crates.io packages.
- [ ] `polint-bench` and `polint-eval` remain `publish = false`.
- [ ] No publishable manifest depends on `polint-core`, `polint-ir`, `polint-analysis-api`, `polint-frontend-api`, `polint-analysis`, `polint-go`, or `polint-ts`.
- [ ] The release script publishes/dry-runs only `polint-macros` and `polint`, in resumable dependency order.
- [ ] The migration must not attempt to overwrite already published `0.1.17` artifacts; final release/version handling is recorded honestly.

### Architecture

- [ ] Core cannot depend on analysis, frontend implementations, or language implementations.
- [ ] IR can depend on core but not concrete languages.
- [ ] Neutral analysis cannot import concrete Go or TypeScript implementation modules.
- [ ] Go and TypeScript implementation modules cannot import one another.
- [ ] Concrete frontend registration remains at the composition root.
- [ ] Typed SDK fact views remain the only normal rule-author path to facts.
- [ ] Provider replacement commits facts and metadata through one coherent lifecycle.

### Process

- [ ] Stabilize the current uncommitted correctness work before structural movement.
- [ ] Create an intermediate commit after every coherent, green tranche.
- [ ] Do not push intermediate commits.
- [ ] Preserve the 44 pre-existing untracked artifacts unless separately classified and intentionally handled.
- [ ] Do not create, switch, remove, or manage worktrees.
- [ ] Never run parallel writers in this checkout.
- [ ] Every agent handoff includes exact scope, invariants, expected tests, and a request to stop rather than paper over a contradiction.

## 3. Agent/coordinator execution protocol

For every implementation tranche:

1. **Coordinator preflight**
   - confirm clean-or-understood tracked state;
   - record current HEAD and local diff;
   - provide the implementation agent the relevant plan section, current failures, architecture rules, Rust skill rules, and exact allowed scope;
   - instruct the agent to edit in the current checkout only, never push, never manage worktrees, and never commit unless explicitly assigned that step.
2. **Luna implementation**
   - inspect existing implementation and tests before editing;
   - make the smallest coherent change that completes the tranche;
   - run focused tests and formatting;
   - report changed files, design choices, tests, residual failures, and uncertainties.
3. **Coordinator verification**
   - inspect the complete diff, not only the agent summary;
   - verify visibility and ownership, stable identities, metadata lifecycle, error behavior, determinism, and platform assumptions;
   - rerun focused tests through Cargo;
   - run broader checks appropriate to the tranche;
   - reject placeholders, broad visibility, stale compatibility paths, or tests that only exercise mocks when a real provider path is required.
4. **Independent review pass**
   - use a fresh Luna Max review context for the completed tranche;
   - request correctness/security/API/build/release findings with exact path and evidence;
   - reproduce material findings and fix all verified issues before accepting the tranche.
5. **Checkpoint**
   - update this checklist and `REMEDIATION-LOOP.md` with exact evidence;
   - commit the coherent tranche with a durable commit message;
   - confirm no unintended generated or untracked files are staged.

A tranche is not complete because it compiles. It is complete only when its focused behavior, architecture boundary, tests, and review findings are resolved.

## 4. Baseline and stabilization

**Purpose:** distinguish existing correctness failures from migration failures and produce a trustworthy green checkpoint before moving modules.

### 4.1 Inventory and baseline

- [ ] Record HEAD, origin/main relationship, PR check state, tracked diff, and preserved untracked manifest.
- [ ] Record the current package/dependency graph and publish script assumptions.
- [ ] Record public-surface allowlist count and example package count.
- [ ] Record baseline default build/check time, binary size, and dependency families when practical; these are measurements, not reasons to preserve the wrong package graph.

### 4.2 Finish H-3 metadata lifecycle work already in progress

Current local work explicitly implements `AnalysisHost` for the facade database and restores metadata refreshes. It is incomplete until real capability paths are clean.

- [ ] Preserve and review the existing eight-file uncommitted diff.
- [ ] Ensure each scheduled neutral provider replacement writes facts and correct metadata atomically.
- [ ] Remove precision-ceiling violations, especially reachability rows currently labeled `exact` above a `setup_aware` ceiling.
- [ ] Ensure metadata families use stable semantic references rather than run-local dense IDs where required.
- [ ] Ensure stale family metadata is removed on replacement.
- [ ] Ensure local Go/TS test databases retain only the minimal generic lifecycle they need.
- [ ] Add/retain a real scheduled-kernel regression asserting no missing or stale metadata across moved families.
- [ ] Make both Go and TypeScript capability-matrix supported-view tests pass.
- [ ] Run metadata validation with `POLINT_VALIDATE_FACTS=1` on representative Go, TS, and polyglot paths.

### 4.3 Fix known pushed-tip CI regressions

- [ ] Replace the semantic-store future-schema test’s stale hard-coded `found: 2` with fixture-derived/current future-schema behavior without weakening the assertion.
- [ ] Reproduce and resolve semantic-store parity on macOS/local; ensure the assertion remains meaningful cross-platform.
- [ ] Confirm the publish dry-run failure is solely the known internal-package graph issue; defer its structural resolution to consolidation rather than publishing temporary internal crates.

### 4.4 Stabilization gates

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets --all-features --locked`
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- [ ] focused provider outcome and metadata lifecycle tests
- [ ] `cargo test -p polint --test capability_matrix --locked`
- [ ] `cargo test -p polint --lib semantic_store_check_parity --locked`
- [ ] `cargo test -p polint --test public_surface_leak --locked`
- [ ] `cargo test -p polint --test golden --locked`
- [ ] nearest determinism and polyglot gates

### 4.5 Stabilization review and commit

- [ ] Fresh Luna Max review of the entire stabilization diff.
- [ ] Fix every verified review finding.
- [ ] Update remediation ledger for H-3 and known CI regressions.
- [ ] Commit: metadata lifecycle and pushed-tip CI stabilization.

## 5. Source consolidation strategy

Move leaf packages inward before their dependencies. Keep each movement behavior-preserving. Do not add feature gates during file movement.

### 5.1 Consolidate `polint-go`

- [ ] Move `crates/polint-go/src` into a private `polint` language module.
- [ ] Move embedded Go sidecar sources/assets with paths that continue to package correctly.
- [ ] Replace `polint_go::` references with internal module paths.
- [ ] Remove `pub(crate) use polint_go as go` once imports are migrated.
- [ ] Move unit tests with their implementation and preserve platform conditionals.
- [ ] Reconcile any facade-local Go code by owner rather than creating duplicate `go` trees.
- [ ] Keep Go-specific parser/lifecycle/lowering code out of neutral analysis modules.
- [ ] Remove the `polint-go` package only after all workspace consumers are migrated.
- [ ] Verify Go sidecar builds, Go unit tests, real provider paths, capability matrix, golden output, determinism, and packaging asset inclusion.
- [ ] Fresh review, fixes, checklist update, and commit.

### 5.2 Consolidate `polint-ts`

- [ ] Move `crates/polint-ts/src` into a private `polint` language module.
- [ ] Replace `polint_ts::` references with internal module paths.
- [ ] Remove `pub(crate) use polint_ts as ts` after migration.
- [ ] Reconcile facade-local TS semantic projection so TS-specific extraction/projection has one clear owner.
- [ ] Keep TS/Oxc implementation details out of neutral analysis modules.
- [ ] Remove the `polint-ts` package only after all consumers are migrated.
- [ ] Verify TS unit tests, malformed/recovery paths, real provider paths, capability matrix, golden output, determinism, and package contents.
- [ ] Fresh review, fixes, checklist update, and commit.

### 5.3 Consolidate frontend contracts

- [ ] Move `polint-frontend-api` into a private frontend contracts module.
- [ ] Tighten `LanguageFrontend`, profiles, units, and source contracts to the narrowest visibility.
- [ ] Update both language implementations and composition-root registration.
- [ ] Remove the `polint-frontend-api` workspace package and dependency.
- [ ] Add/retain architecture checks proving only composition code registers concrete frontends.
- [ ] Fresh review, fixes, checklist update, and commit.

### 5.4 Consolidate neutral analysis

- [ ] Merge `crates/polint-analysis/src` into the existing `polint::analysis` hierarchy.
- [ ] Reconcile names and owners; do not preserve “external crate adapter” layers that no longer serve a real boundary.
- [ ] Evaluate `AnalysisHost`: keep it only if multiple real host implementations still justify it; otherwise use direct private database lifecycle APIs.
- [ ] Unify fact/store replacement and metadata refresh paths.
- [ ] Keep provider contracts and stores private.
- [ ] Ensure Go RTA and TS semantic projection have deliberate logical owners while avoiding facade monolith leakage.
- [ ] Remove the `polint-analysis` package after all consumers move.
- [ ] Run all neutral analysis tests, provider scheduling/outcome tests, solver/data-flow/evidence tests, golden/determinism/polyglot gates.
- [ ] Fresh review, fixes, checklist update, and commit.

### 5.5 Consolidate analysis API, IR, and core

- [ ] Move `polint-analysis-api` into private `analysis::contracts` or the narrowest appropriate modules.
- [ ] Move `polint-ir` into private `ir`, shared by language lowerers and neutral analyses.
- [ ] Merge `polint-core` into the facade’s existing private `core`; reconcile IDs, spans, language identity, stable-key interning, diagnostics, and database vocabulary rather than layering duplicates.
- [ ] Preserve exactly one `AnalysisDb`-scoped interner and no dual string/ID identity paths.
- [ ] Remove all seven internal product packages from workspace members and `[workspace.dependencies]`.
- [ ] Ensure tooling/examples consume only `polint`, with `polint-eval` and `polint-bench` still unpublished.
- [ ] Run complete workspace check/test, public surface, golden, determinism, polyglot, rustdoc, and package-content checks.
- [ ] Fresh review, fixes, checklist update, and commit.

## 6. Visibility and internal architecture enforcement

The consolidation is not complete until internal APIs stop looking like independently supported package APIs.

### 6.1 Visibility cleanup

- [ ] Default all internal items to private.
- [ ] Use `pub(super)` or restricted visibility where possible.
- [ ] Use `pub(crate)` only for intentional cross-subsystem contracts.
- [ ] Keep bare `pub` only in supported SDK/runner/macro surfaces and the documented hidden bench bridge.
- [ ] Remove broad barrels and compatibility re-exports left by package migration.
- [ ] Keep `unreachable_pub = "deny"` and fix violations rather than weakening it.
- [ ] Verify rustdoc exposes no accidental engine/frontend/language extension API.

### 6.2 Structural architecture gates

Add a deterministic source/module dependency gate enforcing:

- [ ] core cannot import analysis/frontend/languages;
- [ ] IR cannot import analysis or languages;
- [ ] neutral analysis cannot import concrete languages;
- [ ] Go and TypeScript cannot import one another;
- [ ] concrete frontends are registered only at the composition root;
- [ ] examples/rule packs import SDK and runner surfaces only;
- [ ] the gate handles aliases and canonical module paths without relying on brittle substring matches where avoidable.

- [ ] Fresh review of visibility and gate blind spots.
- [ ] Fix findings and commit.

## 7. Language feature isolation

Feature gating is a separate behavior change and begins only after behavior-preserving consolidation is green.

### 7.1 Stable feature contract

Target features:

```toml
[features]
default = ["lang-go", "lang-typescript"]
all-languages = ["lang-go", "lang-typescript"]
lang-go = [/* Go-only optional dependencies */]
lang-typescript = [/* TS/JS-only optional dependencies */]
bench = ["lang-go", "lang-typescript"]
```

- [ ] Choose and document stable feature names.
- [ ] Preserve current default behavior with both languages enabled.
- [ ] Define `all-languages` as the forward-compatible aggregate.
- [ ] Keep public `Language`/report/schema shapes feature-independent where practical so SDK source shape does not vary with compilation features.

### 7.2 Dependency and module gating

- [ ] Mark Go-only dependencies optional and connect them only to `lang-go`.
- [ ] Mark Oxc/TS-only dependencies optional and connect them only to `lang-typescript`.
- [ ] Gate language implementation modules and provider registration.
- [ ] Gate tests and tooling deliberately; do not silently stop testing full-language behavior.
- [ ] Ensure bench tooling requests all languages explicitly.
- [ ] Ensure generated skills/docs state compiled-language limitations honestly.

### 7.3 Unavailable-language behavior

- [ ] Configuration/rules requesting an uncompiled language produce controlled `polint/capability` diagnostics.
- [ ] No placeholders are supplied for unsupported hard capabilities.
- [ ] No panics or “success with empty facts.”
- [ ] Compiled provider/language set participates in generation/store/cache identity where behavior can differ.
- [ ] A cache/store created with one feature set cannot be trusted under another without compatible identity validation.

### 7.4 Feature matrix gates

- [ ] `cargo check -p polint --no-default-features`
- [ ] `cargo check -p polint --no-default-features --features lang-go`
- [ ] `cargo check -p polint --no-default-features --features lang-typescript`
- [ ] `cargo check -p polint --features all-languages`
- [ ] no-default dependency tree includes neither Go parser/sidecar nor Oxc families
- [ ] Go-only dependency tree excludes Oxc
- [ ] TS-only dependency tree excludes tree-sitter Go
- [ ] focused unavailable-language diagnostic tests in each reduced configuration
- [ ] default/all-languages goldens and capability matrices remain correct
- [ ] record clean build time, dependency count, and binary size for default, Go-only, TS-only, and no-language builds

- [ ] Fresh feature-isolation review, fixes, and commit.

## 8. Two-package release pipeline

### 8.1 Manifests

- [ ] Only `polint` and `polint-macros` are publishable.
- [ ] Internal tooling remains `publish = false`.
- [ ] `cargo metadata` shows no removed internal package dependencies.
- [ ] Package include/exclude behavior contains all required embedded assets, schemas, docs, and generated skill material without target/cache artifacts.

### 8.2 Release tooling

- [ ] Update `scripts/publish-crates.sh` to the two-package graph.
- [ ] Keep publication resumable if `polint-macros` already exists.
- [ ] Avoid crates.io race assumptions; verify package before dependent package.
- [ ] Clean/isolate `target/package` to avoid stale rust-cache restoration artifacts.
- [ ] Validate versions and the already-published-version scenario explicitly.

### 8.3 Release gates

- [ ] `cargo package -p polint-macros --locked`
- [ ] `cargo package -p polint --locked`
- [ ] verify packaged `polint` from its extracted package directory
- [ ] `DRY_RUN=1 ./scripts/publish-crates.sh`
- [ ] `cargo install --path crates/polint --locked` smoke
- [ ] fresh release/packaging review, fixes, and commit.

## 9. Complete remaining ultrareview remediation

The architectural consolidation does not waive independent correctness/security findings. Reverify every original finding against the new tree and close it with tests or explicit disproof.

### Correctness and integration

- [x] B-1 current-main integration performed at `5803ca31`; final gates still required on final tip.
- [x] H-1 global call-site IDs implemented at `860a5be1`; reverify after movement.
- [x] H-2 typed provider failures implemented at `c8afd7da`; reverify after movement.
- [ ] H-3 complete metadata lifecycle (stabilization tranche).

### CLI/security

- [ ] H-4 `new-rule` must reject symlinked output parents and never write outside the repository.
- [ ] M-7 `new-rule` must be transactional or fully preflighted so failures leave no partial scaffold.
- [ ] Test rules parent, fixture parent, destination collisions, and failure at every write boundary.

### Go process/lifecycle security

- [ ] H-5 replace marker-only predictable sidecar cache trust with private ownership/permission/content verification and atomic publication.
- [ ] H-6 apply offline policy to cold embedded sidecar compilation and every relevant Go subprocess.
- [ ] H-7 route module/symbol/sidecar processes through bounded execution with output draining, timeout diagnostics, and descendant cleanup.
- [ ] Add adversarial preseed, competing publisher, empty-cache offline, sleeping child, and cleanup tests across supported platforms.

### CI/release configuration

- [ ] H-8 use a documented supported cargo-deny invocation consistently.
- [ ] M-8 becomes obsolete with removed internal Go package path, but workflow dependency/cache paths must still all exist and be statically checked.

### Stable identity and payloads

- [ ] M-1 remove run-local `FileId` from identity/reachability stable keys.
- [ ] M-2 resolve reachability digest references to stable semantic text.
- [ ] M-3 remove dense type/value/alias row and relation IDs from stable payloads.
- [ ] M-4 remove dense refined-call edge and relation IDs from stable payloads.
- [ ] Add allocation-order and complete ID-remapping invariance tests, preferably generalized rather than fixture-only.

### Logical ownership after consolidation

- [ ] M-5 verify TS-specific projection has one intentional language owner and neutral graph assembly remains language-neutral.
- [ ] M-6 verify Go RTA is separated into neutral algorithm plus Go adaptation where that separation is real; do not create public crates or generic ceremony solely to satisfy an old path-based finding.
- [ ] Add module-level architecture tests for both boundaries.

For each finding:

- [ ] reproduce against current code;
- [ ] implement minimal complete fix;
- [ ] add adversarial/real-path regression;
- [ ] run focused and neighboring tests;
- [ ] fresh Luna Max review;
- [ ] fix all verified follow-ups;
- [ ] update `REMEDIATION-LOOP.md`;
- [ ] commit cohesive finding group.

## 10. Documentation and PR reconciliation

- [ ] Replace the root eight-crate product graph in `ARCHITECTURE.md` with the implemented private module graph and two-package distribution model.
- [ ] Update `AGENTS.md` architecture summary only through its managed/source-appropriate mechanism.
- [ ] Update API visibility plan to state that internal contracts are private and intentionally unstable.
- [ ] Document default, Go-only, TS-only, no-language, and all-language installation/build choices honestly.
- [ ] Update README/version/release guidance and generated skill text where feature availability is user-visible.
- [ ] Add a superseding architecture decision; preserve historical swarm records unchanged.
- [ ] Update PR #96 body/status evidence locally; do not push or mark ready until final review passes.
- [ ] Reconcile `REMEDIATION-LOOP.md` ledger statuses with actual commits and evidence.
- [ ] Fresh documentation/API-contract review, fixes, and commit.

## 11. Exhaustive final review/fix loops

Three explicit review/fix iterations are required by the active remediation workflow. Earlier focused reviews count only as tranche reviews; final iterations examine the integrated branch.

### Iteration 1 — architecture, API, and correctness

- [ ] Review full `origin/main...HEAD` plus uncommitted final delta.
- [ ] Check module dependency direction, visibility, SDK/macro/runner compatibility, provider lifecycle, fact/store ownership, identity, cache, determinism, and language behavior.
- [ ] Reproduce and fix every verified blocker/high/medium issue.
- [ ] Rerun affected focused and neighboring suites.
- [ ] Commit fixes and record review evidence.

### Iteration 2 — security, process boundaries, packaging, and platforms

- [ ] Review repository writes, symlinks, subprocesses, caches, offline behavior, timeouts, temp paths, package contents, feature combinations, CI workflows, and Windows/macOS/Linux assumptions.
- [ ] Reproduce and fix every verified issue.
- [ ] Commit fixes and record review evidence.

### Iteration 3 — adversarial final-tip review

- [ ] Fresh whole-branch Luna Max review with no reliance on earlier conclusions.
- [ ] Recheck all original findings, newly changed code, public surface, release artifacts, and tests for blind spots.
- [ ] Fix everything verified; no known accepted defects remain unless explicitly escalated to the user.
- [ ] Commit final fixes and record final verdict.

## 12. Final release-readiness gates

Run sequentially on one final tracked SHA. Do not claim earlier evidence for the final tree.

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- [ ] `cargo test --workspace --all-features --locked`
- [ ] reduced feature matrix checks and tests
- [ ] `cargo test -p polint --test public_surface_leak --locked`
- [ ] `cargo test -p polint --test capability_matrix --locked`
- [ ] `cargo test -p polint --test golden --locked`
- [ ] determinism suite
- [ ] polyglot suite
- [ ] explicit semantic-store parity/check
- [ ] explicit cargo-install smoke
- [ ] all examples/rule packs
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked`
- [ ] `cargo deny check all`
- [ ] architecture/import/visibility gates
- [ ] feature dependency-isolation gates
- [ ] package and publish dry-run gates
- [ ] verify no generated output or unintended untracked file is included
- [ ] record full output, duration, tested SHA, and checksum in a new final-tip gate log

## 13. Definition of done

Work is complete only when all statements below are true:

- [ ] The stabilization checkpoint and every migration/remediation tranche have intermediate commits.
- [ ] The workspace contains one publishable product package plus its proc-macro companion.
- [ ] Internal APIs are private and intentionally changeable.
- [ ] Default Go+TS behavior is preserved.
- [ ] Reduced language builds materially exclude unused dependency families.
- [ ] Every original ultrareview finding is fixed or disproven with current-tip evidence.
- [ ] Three recorded integrated review/fix iterations are complete.
- [ ] All verified review findings are fixed.
- [ ] Complete final gates pass on the final tracked SHA.
- [ ] The PR documentation and architecture docs describe the actual final design.
- [ ] Local branch matches the intended ready-to-push state with no accidental tracked/untracked residue.
- [ ] Nothing has been pushed during implementation; the user receives a final ready-to-push report and controls publication.
