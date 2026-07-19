# Semantic Store Identity Readiness Audit

Date: 2026-07-19
Audited base: `origin/main` at `afa32d8f`

## Result

The Phase 64 SQLite facade is ready for a minimal generation-lifecycle PR. The broader kernel metadata vocabulary is not ready to be persisted or reused as one atomic contract.

This is the restart’s R0 gate. R1 may implement only the generation state machine. R2 and later slices must obey the per-contract decisions below.

## Status labels

- **Ready:** usable in the next owning slice without redesign.
- **Conditional:** usable only through the narrow projection and restrictions listed.
- **Not ready:** requires an independent prerequisite or must be omitted from durable reuse.

## Audit table

| Contract on `origin/main` | Status | Evidence | Allowed restart use |
|---|---|---|---|
| Private store facade and configuration | Ready | `analysis_kernel/store/mod.rs` keeps paths, SQL, and `rusqlite` private; disabled mode returns before opening the store. | R1 may extend the private facade without public behavior. |
| SQLite connection policy | Ready | Writer lease uses `BEGIN IMMEDIATE`; WAL, foreign keys, bounded busy timeout, and read-only connections are already tested. | Reuse directly in R1. |
| Schema migration bootstrap | Ready for R1 | Schema v1 contains only the migration marker and rejects future/malformed versions. | R1 may add one small exact migration for generation state. |
| Generation lifecycle | Not ready by design | Main has no generation, pending, complete, or active-selection tables. | This is R1’s entire production scope. |
| `Digest` and `DigestKind` codec | Conditional | The types are ordered and serializable with explicit kind labels, but `DigestBuilder::debug_part` permits unstable formatting and values are 64-bit FNV fingerprints. | Store as supporting evidence, never as the sole proof of row equality; prohibit `debug_part` in persisted projections. |
| Workspace identity | Not ready | Main has no closed workspace identity type; store ownership currently derives from the cache path. | R1 uses store ownership only. Define a minimal explicit workspace projection in R2. |
| Complete configuration identity | Conditional | `config_hash` is deterministic and extensively mutation-tested, but it is a string hash without a purpose-typed run identity wrapper. | R2 may retain it as one manifest field alongside schema/version and canonical source fields; do not pass it into every provider key. |
| `InputSnapshot` v1 | Not ready | It embeds `plan_digest` inside rule components, records a placeholder absent model input, includes mtime hints in its serialized shape, and cannot prove that tool-invocation rows describe tools actually run. | Do not persist the full snapshot in R2. Select a minimal audited run-manifest projection instead. |
| `InputComponentStatus` | Conditional | Present/absent/unsupported/setup-missing is a closed serializable enum. It has no failed/succeeded provider semantics. | Reuse only for input availability, not provider outcomes. |
| `ProviderManifest` | Conditional | Provider ID, kind, inputs, outputs, language, cache policy, schemas, and precision are explicit static fields with deterministic label helpers. | A later provider slice may persist an explicit projection after exact codec tests; do not serialize Rust debug text. |
| `ProviderOutputMeta` | Not ready | `validation` is an open string, cache telemetry lives beside semantic fields, dependencies are only digests, and construction labels every manifest-derived row `native_trusted`. | Complete the provider-outcome prerequisite before persisting trusted provider rows. |
| Provider execution outcome | Not ready | No single typed outcome distinguishes succeeded, failed, blocked, skipped, and absent providers across the kernel. | R3 owns this contract independently of SQLite. |
| Effective capability state | Not ready | Main finalizes support before all late execution and global-validation failures are reflected. | R3 must fix capability revocation before store metadata can claim trusted availability. |
| `LayerKey` | Not ready | The generic key has a broad `config_digest`; the compatibility bridge imports full config, rule, and plan hashes; producer-specific consumed-input boundaries are not enforced. | Audit and migrate one provider in its own prerequisite/R4 PR. Do not perform a repository-wide key migration. |
| `SummaryKey` | Conditional, deferred | Fields are explicit and ordered, but production completeness and all behavior-affecting budgets/settings have not passed the readiness gate. | Defer until summary persistence; no Phase 65 restart dependency. |
| `QueryKey` | Not ready | Query kind/version are open strings and the key does not declare the complete typed input/status vocabulary needed for durable invalidation. | Defer until query persistence; do not redesign in R1–R4. |
| `DependencyIndex` v1 | Not ready | It serializes independently materialized forward and reverse maps, uses stringly `Input`/`Extension`/`ToolInvocation` nodes, and producer coverage is incomplete. | Do not persist it wholesale. R4 derives one canonical edge set for one provider. |
| Syntax provider dependency edges | Not ready | Existing syntax manifests can omit the exact key inputs required to authenticate reuse. | Fix and prove one syntax provider before selecting syntax as the first mirrored provider. |
| `FactMeta` | Not ready | Stable-key ownership is partly canonical, but durable identity still depends on transient `FactRef::run_id`; cross-file semantic-ID collision and legacy cache concerns remain. | Defer fact metadata. Fix semantic IDs independently before fact persistence. |
| Validation result | Not ready | `validate_fact_metadata` returns rendered diagnostics rather than a closed structured event/result that can revoke provider trust and be persisted. | R3 or a later focused validation PR must introduce the minimal structured result. |
| Cache statistics and demand trace | Telemetry only | Cache counters and demand cache statuses describe execution, not semantic truth. | May be reported privately, but never enter run/provider/generation identity. |

## R1 approved scope

R1 may touch only the private store facade, migration, connection/test helpers, and a new generation-lifecycle module/test file if needed.

Approved behavior:

1. reserve a pending generation handle;
2. mark the same handle complete inside a transaction;
3. rotate one active pointer only to a complete handle;
4. read only the selected complete handle;
5. preserve the prior active handle after injected failure;
6. reject malformed state and future schema without public output changes.

R1 must not contain:

- `InputSnapshot` changes;
- provider or capability changes;
- layer/query/summary key changes;
- fact metadata;
- Go runtime/toolchain/process changes;
- broad repository filesystem hardening;
- performance harness redesign;
- public CLI/config/SDK changes.

## R2 decision required after R1

R2 must choose a minimal run-manifest projection instead of persisting `InputSnapshot` v1 wholesale. Candidate fields are:

- manifest schema version;
- explicit workspace/cache ownership identity;
- complete config digest plus its purpose label;
- deterministic source-file membership/content projection if required by the slice;
- creation-independent run identity derived from those canonical fields.

Rule, plan, provider, tool, model, extension, query, and fact metadata remain out until their own readiness gates pass.

## Go/no-go

- **R1:** GO.
- **R2 full `InputSnapshot` mirror:** NO-GO.
- **Trusted provider metadata:** NO-GO until R3.
- **Whole dependency-index persistence:** NO-GO.
- **Fact metadata persistence:** NO-GO.
