# Build the native binary
build:
   make

# Run unit tests
test *ARGS:
    cargo test {{ ARGS }}

pull-test-images:
    podman pull -q quay.io/fedora/fedora-bootc:42 quay.io/centos-bootc/centos-bootc:stream9 quay.io/centos-bootc/centos-bootc:stream10 >/dev/null

# Run integration tests
test-integration *ARGS: build pull-test-images
    env BCVK_PATH=$(pwd)/target/release/bcvk cargo run --release -p integration-tests -- {{ ARGS }}

# Run specific integration test
test-integration-single TEST: build pull-test-images
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

