# Capability probes

This suite tests conclusions, not merely fact-family presence. Each probe is a tiny Go or
TypeScript program whose positive case requires a named capability level, paired with one or
more near-identical twins that must not report.

The suite is defined by `probes.toml`. Programs live below `repo/`, which is copied to a fresh
temporary repository and analyzed once per test run. The harness uses polint's normal analysis
kernel and the same expected/observed matcher used by eval fixtures. Policy-query probes run as
real `#[polint::rule]` rules over typed SDK views. Domain probes inspect the kernel's test-only
fact surface because the raw abstract-domain store is intentionally not a public rule API.

## Levels

- **L1 — syntactic:** exact constructs, imports, literals, call names, and literal argument
  positions. Comment/string lookalikes and safe near-names are twins.
- **L2 — semantic-resolved:** cross-file symbol identity, aliases, re-export/module reachability,
  and explicit unresolved-import status. Same-named symbols from safe modules are twins.
- **L3 — intraprocedural flow-sensitive:** branch refinement, reachability, initialization,
  dominance/post-dominance for guards and cleanup, and same-function source-to-sink flow.
  Guarded, initialized, all-exit-cleanup, reachable, and sanitized variants are twins.
- **L4 — seed:** two-function taint, entrypoint-to-danger reachability, and direct-versus-refined
  call edges. The seed contains exactly 60 cases across Go and TypeScript. It records the current
  pass rate but does not certify L4 or fail CI.

## Certification

Results roll up independently for every level and language.

- L1–L3 positives must pass at least 95%.
- L1–L3 twins must pass 100%.
- L4 rates are printed as a non-gating seed baseline.

The failing assertion lists every probe and case that missed its expected outcome. A positive
failure is not converted into a twin or weakened to preserve a claim: fix an incorrect probe, or
fix and test the engine behavior it exposed.

Run the suite with Go available on `PATH`:

```sh
export PATH="/opt/data/home/.local/bin:$PATH"
cargo test -p polint --lib --all-features --locked \
  eval::capability_probes:: -- --nocapture
```

The certification-only CI command is:

```sh
cargo test -p polint --lib --all-features --locked \
  eval::capability_probes::capability_probe_certification_rollup \
  -- --exact --nocapture
```

## Adding a probe

1. Choose the lowest level whose machinery is necessary for the conclusion.
2. Add a uniquely named `[[probe]]` entry to `probes.toml`.
3. Add `positive/` and at least one `twin-*` directory under
   `repo/<level>/<language>/<probe-name>/`.
4. Keep the programs self-contained and valid for their language. Use globally unique function
   and variable names when adding L4 cases so facts cannot cross-match another seed.
5. Select an existing detector from the harness. If new machinery is required, add the smallest
   detector that queries real kernel facts or a public typed view; do not parse comments or use a
   source-text shortcut for a semantic or flow conclusion.
6. Run all three suite tests. Check the printed per-level/per-language rates and confirm every new
   twin remains quiet.

`capability_probe_manifest_has_unique_ids_and_cases` rejects duplicate IDs or case directories
and preserves the 60-case L4 seed size. `capability_probe_suite_is_deterministic` runs the corpus
twice and compares byte-serialized results.
