---
phase: 09
slug: plugin-skeleton
status: verified
threats_open: 0
asvs_level: 1
created: 2026-05-01
verified: 2026-05-01T13:17:31Z
---

# Phase 09 — Security

Per-phase security contract: threat register, accepted risks, and audit trail.

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| host facts -> plugin | Future plugins can request narrow host-owned facts. | Stable IDs for files, functions, and branches. |
| plugin -> diagnostics | Future plugins report findings through host-owned payloads. | Typed diagnostic records. |
| manifest file -> host | Local JSON manifests identify plugin metadata and component paths. | Manifest fields and filesystem paths. |
| wasm bytes -> Wasmtime | Optional feature validates component bytes without execution. | Local component bytes. |
| docs -> user expectations | User-facing docs describe what the skeleton does and does not do. | Experimental status and out-of-scope runtime behavior. |

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-09-01-01 | Information Disclosure | host fact API | mitigate | WIT host API uses stable IDs and tests reject full AST/source payload names. | closed |
| T-09-01-02 | Tampering | diagnostic report API | mitigate | `report` accepts a typed diagnostic record. | closed |
| T-09-01-03 | Repudiation | plugin metadata | mitigate | WIT exports typed metadata and capabilities. | closed |
| T-09-01-04 | Elevation of Privilege | plugin runtime | mitigate | No execution or scheduling API was added. | closed |
| T-09-02-01 | Tampering | plugin manifest | mitigate | Required manifest fields are validated with typed errors. | closed |
| T-09-02-02 | Spoofing | component path | mitigate | Relative component paths resolve against the manifest directory and missing files fail closed. | closed |
| T-09-02-03 | Denial of Service | invalid component bytes | mitigate | Feature-gated Wasmtime validation rejects invalid component bytes without executing them. | closed |
| T-09-02-04 | Elevation of Privilege | plugin execution | mitigate | The host skeleton remains validate-only and is not wired into `polint check`. | closed |
| T-09-03-01 | Spoofing | user docs | mitigate | Docs explicitly say Wasm plugins are experimental and not run by `polint check` in v1. | closed |
| T-09-03-02 | Information Disclosure | plugin API docs | mitigate | Docs describe stable-ID host queries and no full AST/source transfer. | closed |
| T-09-03-03 | Repudiation | phase evidence | mitigate | `09-03-SUMMARY.md` and this security report record exact pass status. | closed |
| T-09-03-04 | Tampering | future expectations | mitigate | Docs and summaries list automatic compilation, caching, and execution as out of scope. | closed |

Status legend: `closed` threats have implemented mitigation evidence or an explicit accepted-risk entry.

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| None | - | No accepted security risks remain for Phase 09. | - | - |

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-05-01 | 12 | 12 | 0 | Codex |

## Verification Evidence

- `cargo test -p polint-plugin --lib` passed.
- `cargo test -p polint-plugin --features wasmtime-host --lib invalid_component_bytes_are_rejected` passed.
- `cargo fmt -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed.

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-05-01
