# Build the native binary
build:
   make

# Run unit tests (excludes integration tests)
unit *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v cargo-nextest &> /dev/null; then
        cargo nextest run {{ ARGS }}
    else
        cargo test {{ ARGS }}
    fi

pull-test-images:
    podman pull -q quay.io/fedora/fedora-bootc:42 quay.io/centos-bootc/centos-bootc:stream9 quay.io/centos-bootc/centos-bootc:stream10 >/dev/null

# Run integration tests (auto-detects nextest, with cleanup)
test-integration *ARGS: build pull-test-images
    #!/usr/bin/env bash
    set -euo pipefail
    export BCVK_PATH=$(pwd)/target/release/bcvk
    
    # Clean up any leftover containers before starting
    cargo run --release --bin test-cleanup -p integration-tests 2>/dev/null || true
    
    # Run the tests
    if command -v cargo-nextest &> /dev/null; then
        cargo nextest run --release -P integration -p integration-tests {{ ARGS }}
        TEST_EXIT_CODE=$?
    else
        cargo test --release -p integration-tests -- {{ ARGS }}
        TEST_EXIT_CODE=$?
    fi
    
    # Clean up containers after tests complete
    cargo run --release --bin test-cleanup -p integration-tests 2>/dev/null || true
    
    exit $TEST_EXIT_CODE

# Clean up integration test containers
test-cleanup:
    cargo run --release --bin test-cleanup -p integration-tests

# Install cargo-nextest if not already installed
install-nextest:
    @which cargo-nextest > /dev/null 2>&1 || cargo install cargo-nextest --locked

# Run this before committing
fmt:
    cargo fmt

# Run the binary directly
run *ARGS:
    cargo run --release -- {{ ARGS }}

# Create archive with binary, tarball, and checksums
archive: build
    #!/usr/bin/env bash
    set -euo pipefail
    ARCH=$(arch)
    BINARY_PATH="target/release/bcvk"
    TARGET_NAME="bcvk-${ARCH}-unknown-linux-gnu"
    
    # Strip the binary
    strip "${BINARY_PATH}" || true
    
    # Copy binary with target-specific name
    cp "${BINARY_PATH}" "${TARGET_NAME}"
    
    # Create tarball
    tar -czf "${TARGET_NAME}.tar.gz" "${TARGET_NAME}"
    
    # Generate checksums
    sha256sum "${TARGET_NAME}.tar.gz" > "${TARGET_NAME}.tar.gz.sha256"
    
    echo "Archive created: ${TARGET_NAME}.tar.gz"
    echo "Checksum: ${TARGET_NAME}.tar.gz.sha256"

# Install the binary to ~/.local/bin
install: build
    cp target/release/bck ~/.local/bin/

