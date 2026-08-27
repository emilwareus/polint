CARGO ?= cargo
PYTHON ?= python3
BUILD_COST_LABEL ?= local
BUILD_COST_RUNS ?= 1

.PHONY: install test lint doc install-smoke deny check readme-assets fetch-scale-repos scale-corpus-run build-cost build-cost-baseline

install:
	$(CARGO) install --locked --path crates/polint --force

test:
	$(CARGO) test --workspace --all-features --locked

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets --all-features --locked -- -D warnings

# `rustdoc` same as CI doc job (workspace, -D warnings).
doc:
	RUSTDOCFLAGS='-D warnings' $(CARGO) doc --workspace --all-features --no-deps --locked

# Mirrors CI: release `cargo install` + `polint --version` (slow; uses --ignored test).
install-smoke:
	$(CARGO) test -p polint --test cargo_install_smoke --locked -- --ignored

# Supply-chain / policy (install: `cargo install cargo-deny`).
deny:
	$(CARGO) deny check all

# Full local gate aligned with `.github/workflows/ci.yml` (all jobs on one machine).
check: lint test doc install-smoke deny

# Materialize the three golden-corpus scale repositories at the commit SHAs
# pinned in research/evaluation-harness/suites/*-scale.toml (never floating).
# Checkouts are gitignored under research/evaluation-harness/repos/.
fetch-scale-repos:
	$(PYTHON) scripts/fetch-scale-repos.py

# Fetch pinned scale repos and publish LOC / peak RSS / wall-clock into
# research/evaluation-harness/baselines/scale-corpus-run.json (opt-in; not CI).
scale-corpus-run:
	$(PYTHON) scripts/run-scale-corpus.py

# Measure what a repo-local rule host costs to build today - Cargo invocations,
# compiled units, wall-clock, rule-host peak RSS, and bytes written/retained -
# and print the ratio against the committed baseline (opt-in; not CI).
build-cost:
	$(CARGO) build --release --locked -p polint -p polint-bench
	$(CARGO) run --release --locked -p polint-bench -- build-cost \
		--label $(BUILD_COST_LABEL) \
		--runs $(BUILD_COST_RUNS) \
		--baseline research/evaluation-harness/baselines/build-cost.json

# Same matrix, rewriting research/evaluation-harness/baselines/build-cost.json.
# Run only when the recorded numbers are meant to move, on an otherwise idle
# machine, and set BUILD_COST_LABEL to name it. BUILD_COST_RUNS raises the runs
# per cell so the recorded value is a median rather than one sample; wall-clock
# needs that, the counts do not.
build-cost-baseline:
	$(CARGO) build --release --locked -p polint -p polint-bench
	$(CARGO) run --release --locked -p polint-bench -- build-cost \
		--label $(BUILD_COST_LABEL) \
		--runs $(BUILD_COST_RUNS) \
		--out research/evaluation-harness/baselines/build-cost.json

# Regenerate the colored-output SVGs embedded in README.md from the tracked
# ANSI fixtures under docs/img/. Run after changing diagnostic colors/format.
readme-assets:
	$(PYTHON) scripts/render-ansi-to-svg.py \
		--title "polint check --color always" \
		-i docs/img/example-config-denied-literal.ansi \
		-o docs/img/example-config-denied-literal.svg
	$(PYTHON) scripts/render-ansi-to-svg.py \
		--title "polint check --color always" \
		-i docs/img/example-no-raw-colors.ansi \
		-o docs/img/example-no-raw-colors.svg
