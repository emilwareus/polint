# Phase 60-01 Summary: Template Selector and Rule Bodies

## Completed

- Added `polint new-rule <lang> <name> --template <id>` with a closed enum of ten flagship template IDs.
- Preserved the existing default `polint new-rule <lang> <name>` skeleton behavior.
- Generated template rule modules for:
  - `request-to-shell`
  - `secret-to-log`
  - `pii-to-analytics`
  - `sensitive-write-guard`
  - `transaction-cleanup`
  - `raw-reachable-api`
  - `ssrf`
  - `dangerous-html`
  - `unsafe-deserialization`
  - `user-file-path`
- Kept every generated policy template on the same SDK shape: prelude import, one typed preview policy view, one query object, and `violation.diagnostic(ctx.rule_id(), ...)`.

## Notes

- Broader categories such as PII, SSRF, HTML, deserialization, analytics, and file paths are scaffolds over currently backed primitives. The generated target names are placeholders for repo-local APIs.

