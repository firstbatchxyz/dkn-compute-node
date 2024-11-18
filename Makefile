# load .env
ifneq (,$(wildcard ./.env))
		include .env
		export
endif

###############################################################################
.PHONY: launch #       | Run with INFO logs in release mode
launch:
		RUST_LOG=none,dkn_compute=info,dkn_workflows=info,dkn_p2p=info,libp2p_gossipsub=info cargo run --release

.PHONY: run #          | Run with INFO logs
run:
		RUST_LOG=none,dkn_compute=info,dkn_workflows=info,dkn_p2p=info,libp2p=info cargo run

.PHONY: debug #        | Run with DEBUG logs with INFO log-level workflows
debug:
		RUST_LOG=warn,dkn_compute=debug,dkn_workflows=debug,dkn_p2p=debug,ollama_workflows=info,libp2p_gossipsub=info cargo run

.PHONY: trace #        | Run with TRACE logs
trace:
		RUST_LOG=warn,dkn_compute=trace,libp2p=debug cargo run

.PHONY: build #        | Build
build:
		cargo build --workspace

.PHONY: profile-cpu #  | Profile CPU usage with flamegraph
profile-cpu:
	  DKN_EXIT_TIMEOUT=120 cargo flamegraph --root --profile=profiling

.PHONY: profile-mem #  | Profile memory usage with instruments
profile-mem:
	  DKN_EXIT_TIMEOUT=120 cargo instruments --profile=profiling -t Allocations

.PHONY: ollama-versions
ollama-versions:
	  @cat Cargo.lock | grep "https://github.com/andthattoo/ollama-workflows"
		@cat Cargo.lock | grep "https://github.com/andthattoo/ollama-rs"

.PHONY: test #         | Run tests
test:
		cargo test --workspace

###############################################################################
.PHONY: lint #         | Run linter (clippy)
lint:
		cargo clippy --workspace

.PHONY: format #       | Run formatter (cargo fmt)
format:
		cargo fmt -v

.PHONY: version #      | Print version
version:
	  @cargo pkgid | cut -d@ -f2

.PHONY: docs #         | Generate & open documentation
docs:
		cargo doc --open --no-deps --document-private-items

.PHONY: env #          | Print active environment
env:
		@echo "Wallet Secret: ${DKN_WALLET_SECRET_KEY}"
		@echo "Admin Public: ${DKN_ADMIN_PUBLIC_KEY}"

# https://stackoverflow.com/a/45843594
.PHONY: help #         | List targets
help:                                                                                                                    
		@grep '^.PHONY: .* #' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1 \2/' | expand -t20
