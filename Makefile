# load .env
ifneq (,$(wildcard ./.env))
		include .env
		export
endif

###############################################################################
.PHONY: launch #       | Run with INFO log-level & release mode
launch:
		RUST_LOG=info cargo run --release
		
.PHONY: run #          | Run with INFO log-level
run:
		RUST_LOG=none,dkn_compute=info cargo run

.PHONY: debug #        | Run with DEBUG log-level with INFO log-level workflows
debug:
		RUST_LOG=none,dkn_compute=debug,ollama_workflows=info cargo run

.PHONY: trace #        | Run with crate-level TRACE logging
trace:
		RUST_LOG=none,dkn_compute=trace cargo run

.PHONY: build #        | Build
build:
		cargo build

.PHONY: version #      | Print version
version:
	  @cargo pkgid | cut -d@ -f2

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
