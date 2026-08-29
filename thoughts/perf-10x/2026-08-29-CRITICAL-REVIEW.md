# PR #102 critical correctness review

**Branch:** `perf/scan-10x`  
**Reviewed range:** `v0.3.0..4bb2147e` plus review fix `8d40206a`  
**Review date:** 2026-08-29  
**Final verdict:** **MERGEABLE: YES**

The performance changes preserve report behavior and cached-payload integrity after the fixes in
`8d40206a`. Two cache-identity defects were found and fixed. No memory-safety defect, race,
diagnostic-order change, capability-gating change, or exit-code change was found.

## Findings

### F1 — HIGH — hard-coded parser identities could drift from downstream dependency resolution

**Verdict: FIXED in `8d40206a`.**

The parser labels are compile-time strings (`crates/polint/src/analysis_api/parser_identity.rs:17-28`),
but the workspace previously used ordinary compatible requirements. A downstream rule host was
therefore allowed to resolve another patch release while retaining the old identity in every cache
key. This was not theoretical: the preserved plinty vendor contained `tree-sitter 0.26.13` while
the cache label said `tree-sitter-0.26.8`, and `oxc_resolver 11.24.3` while the implementation's
intended resolver was 11.19.1.

The fix exact-pins every Oxc component that participates in parsing/semantic construction and both
tree-sitter components (`Cargo.toml:49-56`, `Cargo.toml:74-75`). The regression test now checks both
the lockfile version and the exact manifest requirement
(`crates/polint/src/analysis_api/parser_identity.rs:50-133`). Thus a published/downstream build
cannot silently change parser behavior while retaining the same persistent key identity.

### F2 — HIGH — the cached TypeScript module graph omitted its resolver toolchain

**Verdict: FIXED in `8d40206a`.**

`LayerKey::module_graph_layer_key` used an absent toolchain digest even though the TypeScript module
graph executes `oxc_resolver`. A resolver upgrade could therefore reuse topology produced by an old
resolver whenever all source/config inputs were unchanged. Syntax-layer parser identities do not
close this hole: resolver behavior may change while syntax output remains byte-identical.

The module-graph key now carries a digest over `TS_MODULE_RESOLVER`
(`crates/polint/src/analysis_kernel/incremental/keys.rs:39-45`,
`crates/polint/src/analysis_kernel/incremental/keys.rs:248-260`). The resolver is exact-pinned at
`Cargo.toml:54`. The regression at
`crates/polint/src/analysis_kernel/incremental/keys.rs:1654-1672` proves that changing resolver
identity changes the toolchain digest.

### F3 — INFO — old cache entries cannot poison the new key space

**Verdict: PASS; no additional schema bump required.**

The global cache version remains `polint-cache-v1:0.3.0`
(`crates/polint/src/cache/mod.rs:12`), but migration safety does not depend on that string:

- Per-file cache filenames hash `parser_identity` as an independent stable-ID component
  (`crates/polint/src/cache/mod.rs:50-79`). Old filenames therefore cannot be selected by a new
  `CacheKey`.
- Go supplies backend plus grammar to both its syntax-layer and file keys
  (`crates/polint/src/go/adapter.rs:230-275`); TypeScript does the same for its backend
  (`crates/polint/src/ts/adapter.rs:254-276`, `crates/polint/src/ts/adapter.rs:480-488`).
- The cache adapter transfers every syntax-key field without dropping the toolchain digest
  (`crates/polint/src/cache/analysis_cache_adapter.rs:153-168`). Layer manifest paths hash the
  serialized complete `LayerKey`
  (`crates/polint/src/analysis_kernel/incremental/layer_cache.rs:480-485`), so old manifests are in
  a different namespace.
- The SCC summary key includes the complete engine parser identity
  (`crates/polint/src/analysis/summaries/provider.rs:224-233`).

After F2, every affected artifact consumer agrees on the relevant parser/resolver identity. Old
content-addressed blobs may remain on disk, but no new manifest selects them merely by their old
key.

### F4 — INFO — manifest output-digest reuse does not bypass blob integrity

**Verdict: PASS.**

The read path still reads the blob under the size limit, recomputes its payload digest, compares it
with the manifest, and evicts both manifest and blob on mismatch
(`crates/polint/src/analysis_kernel/incremental/layer_cache.rs:344-359`). Only after that check does
the typed validator run (`layer_cache.rs:360-363`). A corrupt or truncated blob therefore cannot be
accepted with a valid old manifest digest, absent a hash collision.

The Go validator parses the typed payload, requires the exact sorted `(relative_path,
content_hash)` inventory, and derives the expected output digest from the already-verified payload
digest (`crates/polint/src/go/adapter.rs:300-332`). TypeScript enforces the same contract
(`crates/polint/src/ts/adapter.rs:308-340`). The adapter passes both manifest digests intact
(`crates/polint/src/cache/analysis_cache_adapter.rs:73-99`). Returning the manifest output digest at
`layer_cache.rs:365-370` is consequently safe.

### F5 — INFO — raw-JSON handoff validates syntax, then concrete shape and schema

**Verdict: PASS; semantic cache authentication remains out of scope and unchanged.**

The phrase “well-formed JSON” is literal. `read_json_bytes_with_status` parses into
`serde::de::IgnoredAny`; invalid JSON is evicted, while the original bytes are returned for valid
JSON (`crates/polint/src/cache/mod.rs:248-275`). The Go and TypeScript callers then deserialize those
bytes as `CachedFileAnalysis` and require the current cache schema before restoring facts
(`crates/polint/src/go/adapter.rs:467-486`, `crates/polint/src/ts/adapter.rs:491-510`). Wrong JSON
shape or schema causes recomputation.

A locally modified entry that is both concretely parseable and semantically plausible can still
inject altered facts. The cache is not an authenticated trust boundary, and the prior
`Value -> to_vec -> CachedFileAnalysis` path had exactly the same property. The optimization removes
an intermediate parse/serialization pass; it does not weaken semantic validation relative to
v0.3.0.

### F6 — INFO — interner read-lock fast path is race- and lifetime-safe

**Verdict: PASS.**

The known-key path copies a `StableKeyId` while the temporary read guard is alive
(`crates/polint/src/internal_core/stable_key.rs:62-65`); no reference escapes the guard. On a miss,
the read guard is gone before the write lock is taken, and the map is checked again under the write
lock (`stable_key.rs:67-72`), closing the concurrent-insert race. `intern_and_resolve` returns a
cloned `Arc<str>` (`stable_key.rs:80-87`), and `resolve` likewise clones while locked
(`stable_key.rs:90-97`). There is no aliasing or lifetime escape.

The borrowed metadata builder still uses the canonical sort, length prefixes, and path separator
normalization (`crates/polint/src/analysis_api/metadata.rs:481-521`). Its temporary text is interned
before the thread-local buffer is reused (`crates/polint/src/core/metadata.rs:259-288`). Existing
owned-vs-borrowed and exact-text tests plus the byte gate cover serialization stability.

### F7 — INFO — intentionally forgetting `AnalysisDb` skips no required destructor

**Verdict: PASS.**

`AnalysisDb` contains source vectors, the in-memory interner, metadata/fact stores, path contexts,
and an optional review changeset (`crates/polint/src/core/db.rs:141-155`). It contains no cache
writer, SQLite connection, file writer, temporary-directory guard, or transaction. The explicit
`Drop` implementations in the crate are provider-session guards, a TypeScript thread-local guard,
and a test-only held SQLite writer; none is reachable from the database. The only connection
destructor with an observable rollback is test-only
(`crates/polint/src/analysis_kernel/store/connection.rs:344-367`).

The database is forgotten only after human/JSON/SARIF/AI-friendly output has been written and
immediately before computing the already-determined exit code
(`crates/polint/src/runner/mod.rs:280-299`). Cache handles and report writers remain separately
owned. The commit's side-effect claim is confirmed. This deliberately retains memory until process
exit; `run_cli` is the terminal rule-host entrypoint used as `main`'s return expression.

### F8 — INFO — serialized facts and diagnostics remain deterministic

**Verdict: PASS.**

- Memoized Go line counts are derived once from immutable per-file source and are still looked up by
  `FileId`; canonical row construction/order is unchanged.
- Borrowed glob matching holds an immutable matcher guard and uses thread-local scratch only for the
  `./` candidate; the reference-oracle and property tests retain the old short-circuit semantics.
- Metrics function digests are sorted before folding (`crates/polint/src/metrics.rs:173-187`), and
  the `dependency-edges=v2` parameter retires old manifest shape without changing fact payloads
  (`crates/polint/src/analysis_neutral/metrics.rs:149-165`).
- Go and TypeScript syntax results are sorted by relative path before restoration; Rayon completion
  order cannot leak into facts or diagnostics.
- F1/F2 change cache keys only. They do not change report bytes.

The plinty byte gate rebuilt the rule host against this worktree and compared its direct
`check --format json --fail-on none --ignore-comments true --kind check` output with the preserved
v0.3.0 release output. Both are 15,772 bytes with MD5
`8507f4fae91f6608c6dce4912be0926f`; `cmp` returned 0.

An attempted outer-vs-inner comparison differed only in report metadata name (`polint` versus
`polint-local-rules`), as expected for two different surfaces. The apples-to-apples inner JSON is
byte-identical.

### F9 — INFO — scheduling, gating, ordering, exit status, and AI-friendly behavior are preserved

**Verdict: PASS.**

No change in the reviewed commits alters the analysis plan, provider closure, rule scheduler,
capability support lattice, diagnostic sort, renderer, or fail-threshold calculation. The runner
change occurs after rendering. The full 172-test CLI suite, capability matrix, deterministic eval
fixtures, golden tests, consumer API tests, and public-surface tests passed. This covers human,
JSON, SARIF, GitHub, AI-friendly persistence/stdout, rule filtering, review diff gates, cache
cold/warm parity, and fail-on exit codes.

### F10 — LOW — the 64 MiB manifest limit remains a performance ceiling

**Verdict: WONTFIX in this PR; bounded miss, not incorrect reuse.**

The manifest ceiling now equals the payload ceiling, and function dependencies are folded, which
fixes the observed permanent miss. A pathological repository can still produce more than 64 MiB of
source dependency edges. Such a manifest is evicted and recomputed rather than reused. This can
degrade performance but cannot return stale or corrupted facts, so it does not block this
correctness-focused PR.

## Quality gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo test --workspace` | PASS with Go 1.25 on `PATH`: zero failures |
| plinty v0.3.0 JSON byte comparison | PASS: 15,772 bytes, identical MD5, `cmp=0` |

The base environment had no `go`, which initially caused 22 cascading Go semantic/eval failures.
Go 1.27 was also incompatible with the repository's pinned `x/tools` sidecar (`reflect` package
without types). The authoritative run used Go 1.25, matching the sidecar's `go 1.25.0` declaration.
It passed all tests, including the three capability-matrix tests that were permitted to fail. No
test exception was needed.

The CLI tests generate many temporary rule hosts. An unconstrained run exhausted disk and caused
linker bus errors/`ENOSPC`; those were infrastructure failures. After cleaning only the dedicated,
reproducible Cargo target directories and using four test threads, the complete suite passed.

## Commit produced by this review

- `8d40206a fix(cache): bind persistent keys to analysis toolchains`

No push was performed.

## Final verdict

The two confirmed cache-key soundness defects are fixed and regression-tested. Blob integrity,
raw-JSON concrete validation, concurrency, teardown, deterministic bytes, scheduling, capability
gating, renderers, and exit behavior all passed review and execution gates.

**MERGEABLE: YES**
