# Phase 21: Provenance, Precision, and Validation Metadata - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 21-Provenance, Precision, and Validation Metadata
**Mode:** `--auto`
**Areas discussed:** Metadata storage, metadata coverage, truth labels, stable keys and merge validation, debug and inspection, compatibility and tests

---

## Metadata Storage

| Option | Description | Selected |
|--------|-------------|----------|
| Sidecar metadata store | Store `FactRef -> FactMeta` internally beside existing facts, preserving public fact structs. | yes |
| Inline fields on every fact | Add provenance/confidence/validation fields directly to all existing fact structs. | |
| Public SDK metadata views | Expose metadata to rule authors now. | |

**Auto choice:** Sidecar metadata store.
**Reason:** Matches the analysis-kernel research, avoids public API churn, and keeps rule ergonomics stable.

---

## Metadata Coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Cover current kernel-produced facts, with debug proof for files/imports/symbols/references | Add metadata broadly where practical, while pinning roadmap-required debug coverage. | yes |
| Only cover files/imports/symbols/references | Minimal success-criteria subset. | |
| Wait for the evaluation harness | Defer metadata coverage until Phase 22. | |

**Auto choice:** Cover current kernel-produced facts, with debug proof for files/imports/symbols/references.
**Reason:** SAE-FND-02 says existing fact families carry metadata; the roadmap specifically calls out debug JSON for files, imports, symbols, and references.

---

## Truth Labels

| Option | Description | Selected |
|--------|-------------|----------|
| Small shared internal vocabulary mapped from existing family-specific statuses | Add common provenance/precision/confidence/validation labels while preserving `ResolutionPrecision`, `SymbolPrecision`, and setup/unresolved statuses. | yes |
| Replace family-specific precision/status enums | Migrate every family to one public/common enum. | |
| Store opaque strings | Avoid typed metadata vocabulary for now. | |

**Auto choice:** Small shared internal vocabulary mapped from existing family-specific statuses.
**Reason:** This preserves truthfulness and forward compatibility without forcing a large public fact migration.

---

## Stable Keys And Merge Validation

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministic stable keys plus fail-closed conflict validation | Keep dense run IDs, add metadata stable keys, collapse identical duplicates, and reject or diagnose conflicting duplicates deterministically. | yes |
| Keep current per-family behavior only | Leave non-symbol families without shared stable-key validation. | |
| Panic on conflict | Treat all conflicts as unrecoverable process failures. | |

**Auto choice:** Deterministic stable keys plus fail-closed conflict validation.
**Reason:** Future cache, evidence, and extension behavior depends on stable identities and trustworthy merges; conflicts must not be silent.

---

## Debug And Inspection

| Option | Description | Selected |
|--------|-------------|----------|
| Crate-private/test-facing deterministic debug JSON | Add internal metadata/provenance reports for tests and future harness input. | yes |
| Public CLI metadata output | Add a documented user-facing command now. | |
| No debug output | Only assert metadata through Rust tests. | |

**Auto choice:** Crate-private/test-facing deterministic debug JSON.
**Reason:** The phase needs debug JSON proof but metadata should remain internal unless deliberately promoted later.

---

## Compatibility And Tests

| Option | Description | Selected |
|--------|-------------|----------|
| Preserve existing rule-facing behavior with focused internal tests | Add metadata tests, deterministic debug snapshots, and conflict validation without changing SDK behavior. | yes |
| Broaden SDK tests and docs for metadata | Treat metadata as a rule-author feature now. | |
| Fold cache-key/layer-cache work into this phase | Also implement Phase 23/24 concerns. | |

**Auto choice:** Preserve existing rule-facing behavior with focused internal tests.
**Reason:** Phase 21 is foundation work; cache vocabulary, persistent cache, and public promotion are explicitly later phases.

---

## the agent's Discretion

- Exact module/type names.
- Whether metadata is owned directly by `AnalysisDb` or by kernel-owned sidecar structures that integrate with `AnalysisDb`.
- Exact enum names for shared precision/confidence/validation labels.
- Exact split between metadata attachment, validation, merge checks, and debug JSON across Phase 21 plans.

## Deferred Ideas

- Evaluation harness fixtures and expected/observed JSON.
- Typed cache-key vocabulary and persistent layer cache behavior.
- Public SDK/CLI metadata promotion.
- Extension sink and extension merge activation.
