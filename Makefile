# load .env
ifneq (,$(wildcard ./.env))
		include ./.env
		export
endif

###############################################################################
.PHONY: debug #        | Run with DEBUG logs with INFO log-level workflows
debug:
		RUST_LOG=warn,dkn_compute=debug,dkn_workflows=debug,dkn_p2p=debug,ollama_workflows=info \
		cargo run --bin dkn-compute

.PHONY: build #        | Build
build:
		cargo build --workspace

.PHONY: trace #        | Run with TRACE logs
trace:
		RUST_LOG=warn,dkn_compute=trace,libp2p=debug \
		cargo run --bin dkn-compute

# https://stackoverflow.com/a/45843594
.PHONY: help #         | List targets
help:                                                                                                                    
		@grep '^.PHONY: .* #' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1 \2/' | expand -t20
