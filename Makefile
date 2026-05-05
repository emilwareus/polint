CARGO ?= cargo

.PHONY: install test

install:
	$(CARGO) install --locked --path crates/polint --force

test:
	$(CARGO) test --workspace
