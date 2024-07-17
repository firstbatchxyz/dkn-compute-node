# load .env
ifneq (,$(wildcard ./.env))
		include .env
		export
endif

###############################################################################
.PHONY: run #          | Run with INFO level logging
run:
		RUST_LOG=info cargo run

.PHONY: debug #        | Run with crate-level DEBUG level logging
debug:
		RUST_LOG=none,dkn_compute=debug cargo run

.PHONY: build #        | Build
build:
		cargo build

.PHONY: build-all #    | Build with all features
build-all:
		cargo build --all-features

###############################################################################
.PHONY: test #         | Run tests
test:
		cargo test

############################################################################### 
.PHONY: prompt #       | Run a single prompt on a model
prompt:
		cargo run --example prompt

###############################################################################
.PHONY: lint #         | Run clippy
lint:
		cargo clippy

.PHONY: format #       | Run formatter
format:
		cargo fmt -v

###############################################################################
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
