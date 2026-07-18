CARGO ?= cargo
PYTHON ?= python3
UNAME_S := $(shell uname -s 2>/dev/null)

.PHONY: install test eval-runtime eval-semantic-store eval-semantic-store-scale lint doc install-smoke deny check readme-assets

install:
	$(CARGO) install --locked --path crates/polint --force

test:
	$(CARGO) test --workspace --all-features --locked

# Wall-clock budgets run without contention from the parallel workspace suite.
eval-runtime:
	$(CARGO) test -p polint --lib --all-features --locked --profile performance-gate \
		eval::fixtures::eval_native_fixture_runner_tests::eval_input_snapshot_fixture_meets_five_second_budget \
		-- --exact --ignored --test-threads=1 --nocapture

eval-semantic-store:
	# Cross-platform supported-boundary authenticity smoke with same-host paired controls.
	$(CARGO) test -p polint --lib --all-features --locked --profile performance-gate \
		eval::bench::gate::tests::semantic_store_boundary::supported_boundary_authenticity_smoke_passes_paired_budget \
		-- --exact --ignored --test-threads=1 --nocapture

eval-semantic-store-scale:
ifeq ($(UNAME_S),Linux)
	# Large generated scale gate; CI intentionally runs this only on Linux.
	$(CARGO) test -p polint --lib --all-features --locked --profile performance-gate \
		eval::bench::gate::tests::semantic_store_boundary::generated_scale_store_enabled_measurement_passes_paired_budget \
		-- --exact --ignored --test-threads=1 --nocapture
else
	@echo "Skipping semantic-store scale gate: Linux-only performance policy (host: $(UNAME_S))."
endif

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
	$(CARGO) deny --all-features --locked check

# Core local gate. CI additionally covers MSRV, SARIF shape, and platform matrices.
# Keep the wall-clock budget isolated even when the caller enables parallel make.
check:
	$(MAKE) lint
	$(MAKE) test
	$(MAKE) eval-runtime
	$(MAKE) eval-semantic-store
	$(MAKE) eval-semantic-store-scale
	$(MAKE) doc
	$(MAKE) install-smoke
	$(MAKE) deny

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
