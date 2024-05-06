# load env
ifneq (,$(wildcard ./.env))
		include .env
		export
endif

.PHONY: test #         | Run tests
test:
		cargo test

.PHONY: test-ollama #  | Run Ollama integration tests only
test-ollama:
		cargo test ollama_test --features=ollama_test

.PHONY: test-waku #    | Run Waku integration tests only
test-waku:
		cargo test waku_test --features=waku_test

.PHONY: lint #         | Run clippy
lint:
		cargo clippy

.PHONY: format #       | Run formatter
format:
		cargo fmt -v

.PHONY: run #          | Run with INFO level logging
run:
		RUST_LOG=info cargo run

.PHONY: run-all #      | Run all workers
run-all:
		RUST_LOG=info cargo run --features=synthesis

.PHONY: run-all-dbg #  | Run all workers in debug mode
run-all-dbg:
		RUST_LOG=none,dkn_compute=debug cargo run --features=synthesis

.PHONY: debug #        | Run with crate-level DEBUG level logging
debug:
		RUST_LOG=none,dkn_compute=debug cargo run

.PHONY: docs #         | Generate & open crate documentation
docs:
		cargo doc --open --no-deps

.PHONY: env #          | Print active environment
env:
		@echo "Wallet Secret: ${DKN_WALLET_SECRET_KEY}"
		@echo "Admin Public: ${DKN_ADMIN_PUBLIC_KEY}"

# https://stackoverflow.com/a/45843594
.PHONY: help #         | List targets
help:                                                                                                                    
		@grep '^.PHONY: .* #' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1 \2/' | expand -t20
