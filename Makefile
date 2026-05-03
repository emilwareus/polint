CARGO ?= cargo

.PHONY: install

install:
	$(CARGO) install --locked --path crates/polint-cli --force
