# Build the native binary
build *ARGS:
    cargo build --release {{ ARGS }}

# Run unit tests
test *ARGS:
    cargo test {{ ARGS }}

# Run integration tests for run-ephemeral
test-integration:
    cd crates/kit && cargo test --test run_ephemeral

# Run this before committing
fmt:
    cargo fmt

# Run the binary directly
run *ARGS:
    cargo run --release -- {{ ARGS }}

# Install the binary to ~/.local/bin
install: build
    cp target/release/bck ~/.local/bin/

