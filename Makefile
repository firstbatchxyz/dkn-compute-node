.PHONY: docs
docs:
	cargo doc --open --no-deps

.PHONY: test
test:
	cargo test

.PHONY: lint
lint:
	cargo clippy

.PHONY: run
run:
	RUST_LOG=info cargo run
