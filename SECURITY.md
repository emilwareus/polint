# Security policy

## Supported versions

polint is in early development. Security fixes are applied to the latest released
version.

## Report a vulnerability

Do not open a public issue for a suspected vulnerability.

Request a private reporting channel through the [OAIZ contact page](https://oaiz.io/).
Do not include vulnerability details or secrets in the first message. After a maintainer
gives you a private channel, include:

- the affected version or commit;
- the impact;
- steps to reproduce the problem; and
- a suggested fix, if you have one.

The maintainers will confirm receipt, assess the report, and coordinate a fix and
disclosure with you. Do not include real API keys, customer data, or other secrets in
the report.

For normal bugs and support questions, use the public issue tracker.

## Scope

polint compiles and runs the rule pack in `.polint/rules/` from your repository, with
the privileges of the user who invokes it. Executing repository-owned rule code is the
documented model, not a flaw.

The machine-global rule-host store keeps compiled binaries keyed by their complete build
input, and polint re-hashes a restored binary against the recorded digest before running
it. See the store section in [docs/CACHE.md](docs/CACHE.md).

Reports worth sending:

- reading or writing outside the declared cache, output, and rule-pack directories;
- restoring or running a rule-host binary built from different inputs than the key
  claims;
- a token, credential, or file content leaking into `.polint/output/`, the cache, or a
  SARIF upload;
- a crafted repository causing polint to execute code from a source the documentation
  does not name.
