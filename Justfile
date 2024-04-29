set dotenv-load

# Run tests
test:
	cargo test

# Run clippy
lint:
	cargo clippy

# Run with INFO level logging
run:
	RUST_LOG=info cargo run

# Run with crate-level DEBUG level loggin
debug:
	RUST_LOG=none,dkn_compute=debug cargo run

# Generate & open crate documentation
docs:
	cargo doc --open --no-deps

# Print active environment
env:
  @echo "Wallet Secret: $DKN_WALLET_SECRET_KEY"
  @echo "Admin Public: $DKN_ADMIN_PUBLIC_KEY"
