CARGO ?= cargo

.PHONY: install test

install:
	$(CARGO) install --locked --path crates/polint-cli --force

test:
	$(CARGO) test --workspace
