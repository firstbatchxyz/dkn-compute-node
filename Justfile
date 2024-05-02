set dotenv-load

# Run tests
test:
	cargo test

# Run Ollama integration tests only
test-ollama:
	cargo test ollama_test --features=ollama_test

# Run Waku integration tests only
test-waku:
	cargo test waku_test --features=waku_test

# Run clippy
lint:
	cargo clippy

# Run formatter
format:
	cargo fmt -v

# Run with INFO level logging
run:
	RUST_LOG=info cargo run

# Run all workers
run-all:
	RUST_LOG=info cargo run --features=synthesis

# Run all workers in debug mode
run-all-dbg:
	RUST_LOG=none,dkn_compute=debug cargo run --features=synthesis

# Run with crate-level DEBUG level logging
debug:
	RUST_LOG=none,dkn_compute=debug cargo run

# Generate & open crate documentation
docs:
	cargo doc --open --no-deps

# Print active environment
env:
  @echo "Wallet Secret: ${DKN_WALLET_SECRET_KEY}"
  @echo "Admin Public: ${DKN_ADMIN_PUBLIC_KEY}"
