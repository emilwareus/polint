---
status: complete
quick_id: 260527-auc
slug: document-public-resolvedimports-and-modu
date: 2026-05-27
---

# Summary

Improved the public API documentation:

- Added copyable `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` rule examples to `docs/facts/resolved-imports.md`.
- Updated the custom Go rule README to state that branch/test evidence is heuristic and to show heuristic help text.

Verification:

- `rg -n "no-unresolved-imports|no-ui-to-domain|heuristic" docs/facts/resolved-imports.md examples/custom-rule-go/README.md`
- `git diff --check`
- `cargo test -p polint --test cli phase41_relationship_query_helpers_external_rule --locked -- --exact`
