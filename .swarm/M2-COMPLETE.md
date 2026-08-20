# M2 COMPLETE — structural tip ready; M3 authorized

**Status:** M2 dispatchable tasks closed on tip. **Human authorized M3+M4** (2026-08-08).  
**Integration branch:** `static-analysis-architecture-review`  
**HEAD:** `cad6c8fa` (verify at land time)  
**Never merge to main from swarm.**

## Task set

| Task | State | Notes |
|------|-------|-------|
| W2.1 | MERGED | polint-eval eviction |
| W2.2 | MERGED | core split |
| W2.3 | MERGED-NOOP | Interning follow-up only (human) |
| W2.4 | MERGED | Provider + capability-closure scheduler (reconcile state if CLAIMED stale) |
| W2.5 | MERGED | FactStore; AnalysisDb ~5 fields; no StableKeyId |
| W2.6 | MERGED | LanguageFrontend registry |

## Human binding (overrides older halt text)

See `.swarm/HUMAN-BINDING-2026-08-08-M3-M4.md`:

- Do **M3 + M4** on this branch
- Migrate existing Go/TS only; no new features/languages; no W0.3 dogfood
- Interning = follow-up; LoC metrics ignored

## Next

Dispatch **W3.1** from `docs/architecture-review/specs/W3.1-mir-blocks-terminators.md`.
