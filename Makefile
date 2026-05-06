CARGO ?= cargo
PYTHON ?= python3

.PHONY: install test lint doc install-smoke deny check readme-assets

install:
	$(CARGO) install --locked --path crates/polint --force

test:
	$(CARGO) test --workspace --all-features --locked

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets --all-features --locked -- -D warnings

# `rustdoc` same as CI doc job (workspace, -D warnings).
doc:
	RUSTDOCFLAGS=-D warnings $(CARGO) doc --workspace --all-features --no-deps --locked

# Mirrors CI: release `cargo install` + `polint --version` (slow; uses --ignored test).
install-smoke:
	$(CARGO) test -p polint --test cargo_install_smoke --locked -- --ignored

# Supply-chain / policy (install: `cargo install cargo-deny`). Older binaries may only support `cargo deny check`.
deny:
	$(CARGO) deny check

# Full local gate aligned with `.github/workflows/ci.yml` (all jobs on one machine).
check: lint test doc install-smoke deny

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
