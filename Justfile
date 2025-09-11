# Build the native binary
build:
   make

# Run unit tests
test *ARGS:
    cargo test {{ ARGS }}

# Run integration tests
test-integration *ARGS: build
    env BCVK_PATH=$(pwd)/target/release/bcvk cargo run --release -p integration-tests -- {{ ARGS }}

# Run specific integration test
test-integration-single TEST: build
    env BCVK_PATH=$(pwd)/target/release/bcvk cargo run --release -p integration-tests -- {{ TEST }} --exact --nocapture

# Run this before committing
fmt:
    cargo fmt

# Run the binary directly
run *ARGS:
    cargo run --release -- {{ ARGS }}

# Install the binary to ~/.local/bin
install: build
    cp target/release/bck ~/.local/bin/

